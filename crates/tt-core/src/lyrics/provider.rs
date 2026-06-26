/// Online lyrics provider — search and fetch lyrics via OpenAPI 52VMY.
use crate::lyrics::parser::LrcFile;
use serde::{Deserialize, Serialize};

/// OpenAPI 52VMY lyrics endpoint (酷小狗歌词).
///
/// API docs: https://openapi.52vmy.cn/docs/music/kg/lrc.html
/// Endpoint: GET/POST /api/music/kg/lrc
/// Params:   token (required), word (song name), n (result index)
/// Returns:  raw LRC text with `[mm:ss.xx]` timestamps.
pub const OPENAPI_BASE_URL: &str = "http://openapi.52vmy.cn";

/// Search result from an online lyrics provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricSearchResult {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u64>,
    pub source: String,
}

/// Online lyrics provider trait.
#[async_trait::async_trait]
pub trait LyricsProvider: Send + Sync {
    /// Unique identifier for this provider instance.
    fn name(&self) -> &str;
    /// Search for lyrics by keyword. Returns multiple candidate results.
    async fn search(&self, keyword: &str, token: &str) -> anyhow::Result<Vec<LyricSearchResult>>;
    /// Fetch LRC lyrics by result id.
    async fn fetch_lrc(&self, id: &str, token: &str) -> anyhow::Result<Option<LrcFile>>;
}

/// Registry of lyrics providers. Also holds the API token.
pub struct LyricsProviderRegistry {
    providers: Vec<Box<dyn LyricsProvider>>,
    token: Option<String>,
}

impl LyricsProviderRegistry {
    pub fn new() -> Self {
        Self { providers: Vec::new(), token: None }
    }

    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(OpenApiProvider::new()));
        reg
    }

    pub fn register(&mut self, provider: Box<dyn LyricsProvider>) {
        self.providers.push(provider);
    }

    /// Set the API token. Empty string clears the token.
    pub fn set_token(&mut self, token: String) {
        let trimmed = token.trim().to_string();
        self.token = if trimmed.is_empty() { None } else { Some(trimmed) };
    }

    /// Get the current token, if any.
    pub fn get_token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Whether a token has been configured.
    pub fn has_token(&self) -> bool {
        self.token.as_ref().map_or(false, |t| !t.is_empty())
    }

    /// Search with failover: query providers in order; the first one returning
    /// a non-empty result set wins and is truncated to `limit` entries.
    ///
    /// Returns an error string if no token is configured.
    pub async fn search_with_failover(
        &self,
        keyword: &str,
        limit: usize,
    ) -> Result<Vec<LyricSearchResult>, String> {
        let token = self.token.as_deref().ok_or_else(|| {
            "请先在设置中填写 API Token".to_string()
        })?;

        for provider in &self.providers {
            match provider.search(keyword, token).await {
                Ok(items) => {
                    if !items.is_empty() {
                        return Ok(items.into_iter().take(limit).collect());
                    }
                    tracing::debug!(
                        "Lyrics provider '{}' returned 0 results, trying next",
                        provider.name()
                    );
                }
                Err(e) => {
                    tracing::warn!(
                        "Lyrics provider '{}' search error: {}, trying next",
                        provider.name(),
                        e
                    );
                }
            }
        }
        Ok(Vec::new())
    }

    /// Fetch lyrics from a specific provider by name (base URL) and id.
    pub async fn fetch(&self, source: &str, id: &str) -> anyhow::Result<Option<LrcFile>> {
        let token = self
            .token
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("请先在设置中填写 API Token"))?;

        for provider in &self.providers {
            if provider.name() == source {
                return provider.fetch_lrc(id, token).await;
            }
        }
        Err(anyhow::anyhow!("Unknown provider: {}", source))
    }
}

// ── OpenAPI 52VMY Provider ────────────────────────────────────────────────

/// OpenAPI 52VMY lyrics provider (https://openapi.52vmy.cn).
///
/// Two-step API flow:
/// 1. `GET /api/music/kg/lrc?token={token}&word={keyword}` (no `n`)
///    → returns a list of available lyrics versions (as JSON array or text).
/// 2. `GET /api/music/kg/lrc?token={token}&word={keyword}&n={index}`
///    → returns the LRC text for the selected version.
pub struct OpenApiProvider {
    client: reqwest::Client,
}

impl OpenApiProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    fn lrc_url() -> String {
        format!("{}/api/music/kg/lrc", OPENAPI_BASE_URL)
    }

    fn composite_key(n: u32, keyword: &str) -> String {
        format!("openapi:{}:{}", n, keyword)
    }

    fn parse_key(composite_key: &str) -> Option<(u32, String)> {
        let rest = composite_key.strip_prefix("openapi:")?;
        let colon = rest.find(':')?;
        let n: u32 = rest[..colon].parse().ok()?;
        let keyword = rest[colon + 1..].to_string();
        Some((n, keyword))
    }

    /// Parse `"artist - title"` from a text line.
    fn parse_title_artist(line: &str) -> (String, String) {
        if let Some(pos) = line.find(" - ") {
            (line[pos + 3..].trim().to_string(), line[..pos].trim().to_string())
        } else {
            (line.to_string(), String::new())
        }
    }

    /// Check if the response body looks like an error from the API.
    fn check_api_error(body: &str) -> Result<(), &str> {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Err("服务器返回空响应");
        }
        if trimmed.contains("110") {
            return Err("缺少认证Token");
        }
        if trimmed.contains("120") {
            return Err("无效的认证Token，请检查Token是否正确");
        }
        if trimmed.contains("160") {
            return Err("API点数不足，请充值");
        }
        if trimmed.contains("300") {
            return Err("请求IP不在Token白名单内");
        }
        if trimmed.contains("400") {
            return Err("接口维护中，请稍后重试");
        }
        Ok(())
    }

    /// Parse the list response (without `n`) into individual search results.
    ///
    /// The response may be:
    /// - JSON array: `["歌手A - 歌名", "歌手B - 歌名", ...]`
    /// - JSON object with a `data` array
    /// - Plain text with one entry per line
    fn parse_list_response(body: &str, keyword: &str) -> Vec<LyricSearchResult> {
        let trimmed = body.trim();

        // Try JSON array: ["歌手A - 歌名", "歌手B - 歌名"]
        if let Ok(arr) = serde_json::from_str::<Vec<String>>(trimmed) {
            if arr.is_empty() {
                return Vec::new();
            }
            return arr.into_iter().enumerate().map(|(i, line)| {
                let n = (i + 1) as u32;
                let (title, artist) = Self::parse_title_artist(&line);
                LyricSearchResult {
                    id: Self::composite_key(n, keyword),
                    title: if title.is_empty() { line.clone() } else { title },
                    artist,
                    album: None,
                    duration_ms: None,
                    source: OPENAPI_BASE_URL.to_string(),
                }
            }).collect();
        }

        // Try JSON object with `data` array: {"code":200, "data":["..."]}
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(data) = obj.get("data").and_then(|v| v.as_array()) {
                return data.iter().enumerate().filter_map(|(i, v)| {
                    let line = v.as_str()?;
                    let n = (i + 1) as u32;
                    let (title, artist) = Self::parse_title_artist(line);
                    Some(LyricSearchResult {
                        id: Self::composite_key(n, keyword),
                        title: if title.is_empty() { line.to_string() } else { title },
                        artist,
                        album: None,
                        duration_ms: None,
                        source: OPENAPI_BASE_URL.to_string(),
                    })
                }).collect();
            }
        }

        // Fallback: treat as plain text, one entry per line
        trimmed.lines()
            .filter(|l| !l.trim().is_empty())
            .enumerate()
            .map(|(i, line)| {
                let n = (i + 1) as u32;
                let (title, artist) = Self::parse_title_artist(line.trim());
                LyricSearchResult {
                    id: Self::composite_key(n, keyword),
                    title: if title.is_empty() { line.trim().to_string() } else { title },
                    artist,
                    album: None,
                    duration_ms: None,
                    source: OPENAPI_BASE_URL.to_string(),
                }
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl LyricsProvider for OpenApiProvider {
    fn name(&self) -> &str {
        OPENAPI_BASE_URL
    }

    /// Search: call API without `n` to get the list of available versions.
    async fn search(&self, keyword: &str, token: &str) -> anyhow::Result<Vec<LyricSearchResult>> {
        let resp = self
            .client
            .get(Self::lrc_url())
            .query(&[("token", token), ("word", keyword)])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }
        let body = resp.text().await?;

        if let Err(err) = Self::check_api_error(&body) {
            return Err(anyhow::anyhow!("{}", err));
        }

        Ok(Self::parse_list_response(&body, keyword))
    }

    /// Fetch: call API with specific `n` to get the LRC text.
    async fn fetch_lrc(&self, composite_key: &str, token: &str) -> anyhow::Result<Option<LrcFile>> {
        let (n, keyword) = match Self::parse_key(composite_key) {
            Some(v) => v,
            None => return Err(anyhow::anyhow!("Invalid lyrics key: {}", composite_key)),
        };

        let resp = self
            .client
            .get(Self::lrc_url())
            .query(&[("token", token), ("word", &*keyword), ("n", &*n.to_string())])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }
        let text = resp.text().await?;

        if let Err(err) = Self::check_api_error(&text) {
            return Err(anyhow::anyhow!("{}", err));
        }

        let lrc = crate::lyrics::parser::parse_lrc(&text);
        if lrc.lines.is_empty() {
            return Ok(None);
        }
        Ok(Some(lrc))
    }
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    // – OpenApiProvider key roundtrip –

    #[test]
    fn openapi_composite_key_roundtrip() {
        let key = OpenApiProvider::composite_key(2, "晴天");
        let (n, kw) = OpenApiProvider::parse_key(&key).unwrap();
        assert_eq!(n, 2);
        assert_eq!(kw, "晴天");
    }

    #[test]
    fn openapi_parse_key_rejects_foreign_format() {
        assert!(OpenApiProvider::parse_key("lrclib:123").is_none());
        assert!(OpenApiProvider::parse_key("garbage").is_none());
    }

    // – Title/Artist parsing from list entry line –

    #[test]
    fn parse_title_artist_with_dash() {
        let (title, artist) = OpenApiProvider::parse_title_artist("周杰伦 - 晴天");
        assert_eq!(title, "晴天");
        assert_eq!(artist, "周杰伦");
    }

    #[test]
    fn parse_title_artist_no_dash() {
        let (title, artist) = OpenApiProvider::parse_title_artist("晴天");
        assert_eq!(title, "晴天");
        assert_eq!(artist, "");
    }

    // – List response parsing –

    #[test]
    fn parse_list_json_array() {
        let body = r#"["周杰伦 - 晴天", "刘瑞琦 - 晴天"]"#;
        let results = OpenApiProvider::parse_list_response(body, "晴天");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "晴天");
        assert_eq!(results[0].artist, "周杰伦");
        assert_eq!(results[0].id, "openapi:1:晴天");
        assert_eq!(results[1].title, "晴天");
        assert_eq!(results[1].artist, "刘瑞琦");
        assert_eq!(results[1].id, "openapi:2:晴天");
    }

    #[test]
    fn parse_list_json_object_with_data() {
        let body = r#"{"code":200,"data":["周杰伦 - 晴天","刘瑞琦 - 晴天"]}"#;
        let results = OpenApiProvider::parse_list_response(body, "晴天");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].artist, "周杰伦");
        assert_eq!(results[1].artist, "刘瑞琦");
    }

    #[test]
    fn parse_list_plain_text() {
        let body = "周杰伦 - 晴天\n刘瑞琦 - 晴天\n";
        let results = OpenApiProvider::parse_list_response(body, "晴天");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].artist, "周杰伦");
        assert_eq!(results[1].artist, "刘瑞琦");
    }

    #[test]
    fn parse_list_empty_array() {
        let results = OpenApiProvider::parse_list_response("[]", "晴天");
        assert!(results.is_empty());
    }

    // – API error detection –

    #[test]
    fn check_api_error_ok() {
        assert!(OpenApiProvider::check_api_error("normal text").is_ok());
    }

    #[test]
    fn check_api_error_empty() {
        assert!(OpenApiProvider::check_api_error("").is_err());
    }

    #[test]
    fn check_api_error_missing_token() {
        let err = OpenApiProvider::check_api_error("code:110").unwrap_err();
        assert!(err.contains("缺少"));
    }

    #[test]
    fn check_api_error_invalid_token() {
        let err = OpenApiProvider::check_api_error("code:120").unwrap_err();
        assert!(err.contains("无效"));
    }

    // – Registry tests –

    #[test]
    fn registry_with_defaults_has_provider() {
        let reg = LyricsProviderRegistry::with_defaults();
        assert!(!reg.has_token());
    }

    #[test]
    fn registry_set_token() {
        let mut reg = LyricsProviderRegistry::with_defaults();
        reg.set_token("abc123".into());
        assert!(reg.has_token());
        assert_eq!(reg.get_token(), Some("abc123"));
    }

    #[test]
    fn registry_clear_token() {
        let mut reg = LyricsProviderRegistry::with_defaults();
        reg.set_token("abc123".into());
        reg.set_token("".into());
        assert!(!reg.has_token());
    }
}