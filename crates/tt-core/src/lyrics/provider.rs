/// Online lyrics provider — search and fetch lyrics from music platforms.
use crate::lyrics::parser::LrcFile;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use parking_lot::Mutex;

/// Default 52VMY lyrics API URL (https://api.52vmy.cn).
///
/// Free lyrics API supporting search-by-keyword with multiple result versions
/// (selected via the `n` parameter). Returns standard LRC text by default or
/// JSON with `type=json`. QPS limit: 4 requests per 2 seconds.
pub const DEFAULT_52VMY_URL: &str = "https://api.52vmy.cn";

/// Default LRCLIB server URL (https://lrclib.net).
///
/// LRCLIB is an open-source lyrics database that returns **multiple search
/// results** as a JSON array — each item includes both plain and synced (LRC)
/// lyrics. This is the preferred provider because it directly supports the
/// multi-result selection workflow.
pub const DEFAULT_LRCLIB_URL: &str = "https://lrclib.net";

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
    /// Unique identifier for this provider instance (e.g. its base URL).
    fn name(&self) -> &str;
    async fn search(&self, keyword: &str) -> anyhow::Result<Vec<LyricSearchResult>>;
    async fn fetch_lrc(&self, id: &str) -> anyhow::Result<Option<LrcFile>>;
}

/// Registry of all lyrics providers.
///
/// Providers are ordered by priority (first = highest). `search_with_failover`
/// queries them in order and returns the first non-empty result set, enabling
/// automatic service switching when a server is down or has no match.
pub struct LyricsProviderRegistry {
    providers: Vec<Box<dyn LyricsProvider>>,
}

impl LyricsProviderRegistry {
    pub fn new() -> Self {
        Self { providers: Vec::new() }
    }

    /// Create a registry with the default providers:
    /// 1. LRCLIB — open lyrics database, returns multiple results with LRC
    /// 2. 52VMY — free lyrics API, supports keyword search across versions
    pub fn with_defaults() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(LrclibProvider::new(DEFAULT_LRCLIB_URL.to_string())));
        reg.register(Box::new(FiftyTwoVmyProvider::new(DEFAULT_52VMY_URL.to_string())));
        reg
    }

    pub fn register(&mut self, provider: Box<dyn LyricsProvider>) {
        self.providers.push(provider);
    }

    /// Replace the entire provider list with user-supplied servers.
    ///
    /// Each URL must point to a 52VMY-compatible lyrics API instance
    /// implementing the `/api/music/lrc` endpoint.
    ///
    /// Empty/duplicate URLs are skipped. If all entries are filtered out,
    /// the default 52VMY public server is restored so search never no-ops.
    pub fn set_servers(&mut self, urls: Vec<String>) {
        self.providers.clear();
        let mut seen = std::collections::HashSet::new();
        for url in urls {
            let url = url.trim().to_string();
            if url.is_empty() || !seen.insert(url.clone()) {
                continue;
            }
            self.providers.push(Box::new(FiftyTwoVmyProvider::new(url)));
        }
        if self.providers.is_empty() {
            self.providers.push(Box::new(FiftyTwoVmyProvider::new(
                DEFAULT_52VMY_URL.to_string(),
            )));
        }
    }

    /// Current server URLs (provider names), in priority order.
    pub fn get_servers(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.name().to_string()).collect()
    }

    /// Search with failover: query providers in order; the first one returning
    /// a non-empty result set wins and is truncated to `limit` entries.
    ///
    /// If a provider errors or returns empty, the next is tried. Returns an
    /// empty vec only when all providers are exhausted.
    pub async fn search_with_failover(
        &self,
        keyword: &str,
        limit: usize,
    ) -> Vec<LyricSearchResult> {
        for provider in &self.providers {
            match provider.search(keyword).await {
                Ok(items) => {
                    if !items.is_empty() {
                        return items.into_iter().take(limit).collect();
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
        Vec::new()
    }

    /// Fetch lyrics from a specific provider by name (base URL) and id.
    pub async fn fetch(&self, source: &str, id: &str) -> anyhow::Result<Option<LrcFile>> {
        for provider in &self.providers {
            if provider.name() == source {
                return provider.fetch_lrc(id).await;
            }
        }
        Err(anyhow::anyhow!("Unknown provider: {}", source))
    }
}

// LRCLIB Provider (open lyrics database, e.g. https://lrclib.net)

/// One entry in an LRCLIB `/api/search` JSON response.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibSearchItem {
    id: u64,
    #[serde(default)]
    track_name: String,
    #[serde(default)]
    artist_name: String,
    #[serde(default)]
    album_name: Option<String>,
    /// Duration in seconds (f64).
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    #[allow(dead_code)]
    instrumental: bool,
    /// Synced LRC lyrics (may be null).
    #[serde(default)]
    synced_lyrics: Option<String>,
    /// Plain text lyrics (may be null).
    #[serde(default)]
    plain_lyrics: Option<String>,
}

/// LRCLIB-based lyrics provider — the **preferred** multi-result source.
///
/// Uses the open [LRCLIB](https://lrclib.net) lyrics database:
///
/// - **Search** (`GET /api/search?q={keyword}`) returns a JSON array of
///   matching tracks (up to ~20 results). Each item includes `syncedLyrics`
///   (LRC format) and `plainLyrics`, so `fetch_lrc` can serve from the
///   search cache without a second round-trip.
/// - **Single** (`GET /api/get/{id}`) can be used as a cold-cache fallback,
///   though it is usually unnecessary because the search response already
///   contains the full lyrics.
///
/// No authentication is required for the public instance.
pub struct LrclibProvider {
    client: reqwest::Client,
    base_url: String,
    /// Cache: "lrclib:{id}" -> raw LRC text.
    cache: Mutex<HashMap<String, String>>,
}

impl LrclibProvider {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap_or_default();
        Self {
            client,
            base_url,
            cache: Mutex::new(HashMap::new()),
        }
    }

    fn search_url(&self) -> String {
        format!("{}/api/search", self.base_url)
    }

    fn get_url(&self) -> String {
        format!("{}/api/get", self.base_url)
    }

    fn composite_key(id: u64) -> String {
        format!("lrclib:{}", id)
    }

    fn parse_id_from_key(key: &str) -> Option<u64> {
        key.strip_prefix("lrclib:")?.parse().ok()
    }

    /// Pick the best LRC text: prefer `synced_lyrics`, fall back to `plain_lyrics`.
    fn pick_lrc(item: &LrclibSearchItem) -> Option<String> {
        item.synced_lyrics
            .clone()
            .or_else(|| item.plain_lyrics.clone())
    }
}

#[async_trait::async_trait]
impl LyricsProvider for LrclibProvider {
    fn name(&self) -> &str {
        &self.base_url
    }

    async fn search(&self, keyword: &str) -> anyhow::Result<Vec<LyricSearchResult>> {
        let resp = self
            .client
            .get(self.search_url())
            .query(&[("q", keyword)])
            .send()
            .await?;

        if !resp.status().is_success() {
            tracing::warn!(
                "LRCLIB search returned HTTP {} for keyword {:?}",
                resp.status(),
                keyword
            );
            return Ok(Vec::new());
        }

        let items: Vec<LrclibSearchItem> = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("LRCLIB search JSON parse error: {}", e);
                return Ok(Vec::new());
            }
        };

        if items.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::with_capacity(items.len());
        let mut cache = self.cache.lock();
        for item in items {
            let lrc_text = match Self::pick_lrc(&item) {
                Some(t) => t,
                None => continue, // skip entries without any lyrics
            };
            let key = Self::composite_key(item.id);
            let title = if item.track_name.is_empty() { keyword.to_string() } else { item.track_name.clone() };
            let artist = item.artist_name.clone();
            let album = item.album_name.clone().filter(|a| !a.is_empty() && !a.starts_with("Optional("));
            let duration_ms = item.duration.map(|s| (s * 1000.0) as u64);

            cache.insert(key.clone(), lrc_text);
            results.push(LyricSearchResult {
                id: key,
                title,
                artist,
                album,
                duration_ms,
                source: self.base_url.clone(),
            });
        }

        Ok(results)
    }

    async fn fetch_lrc(&self, composite_key: &str) -> anyhow::Result<Option<LrcFile>> {
        // 1) Hot path: serve from the search cache.
        if let Some(text) = self.cache.lock().get(composite_key).cloned() {
            return Ok(Some(crate::lyrics::parser::parse_lrc(&text)));
        }

        // 2) Cold path: re-fetch by id from /api/get/{id}.
        let id = match Self::parse_id_from_key(composite_key) {
            Some(v) => v,
            None => return Err(anyhow::anyhow!("Invalid LRCLIB lyrics key: {}", composite_key)),
        };

        let resp = self
            .client
            .get(format!("{}/{}", self.get_url(), id))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }
        let item: LrclibSearchItem = match resp.json().await {
            Ok(v) => v,
            Err(_) => return Ok(None),
        };
        let text = match Self::pick_lrc(&item) {
            Some(t) => t,
            None => return Ok(None),
        };
        Ok(Some(crate::lyrics::parser::parse_lrc(&text)))
    }
}

// 52VMY Provider (free lyrics API, e.g. https://api.52vmy.cn)

/// JSON response from the 52VMY `/api/music/lrc` endpoint (`type=json`).
#[derive(Debug, Clone, Deserialize)]
struct VmyLrcResponse {
    code: i32,
    data: Option<Vec<String>>,
}

/// 52VMY-based lyrics provider — free music lyrics API.
///
/// Uses the [维梦API](https://api.52vmy.cn/doc/music/lrc.html):
///
/// - **Search** (`GET /api/music/lrc?msg={keyword}&n={index}&type=json`):
///   returns `{"code":200, "data":["歌名 - 歌手","词:...","曲:...","歌词行1",...]}`.
///   The `n` parameter selects which result version to return (1,2,3…).
///   The provider issues concurrent requests for n=1,2,3 to surface multiple
///   versions (e.g. original artist vs. cover) in one search call.
/// - **Fetch** (`GET /api/music/lrc?msg={keyword}&n={index}`, default TEXT):
///   returns standard LRC text (with `[mm:ss.xx]` timestamps) that can be
///   parsed into an `LrcFile` directly.
///
/// QPS limit: 4 requests per 2 seconds (burst-friendly within that window).
pub struct FiftyTwoVmyProvider {
    client: reqwest::Client,
    base_url: String,
}

impl FiftyTwoVmyProvider {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .build()
            .unwrap_or_default();
        Self { client, base_url }
    }

    fn lrc_url(&self) -> String {
        format!("{}/api/music/lrc", self.base_url)
    }

    fn composite_key(n: u32, keyword: &str) -> String {
        format!("52vmy:{}:{}", n, keyword)
    }

    fn parse_key(key: &str) -> Option<(u32, String)> {
        let rest = key.strip_prefix("52vmy:")?;
        let colon = rest.find(':')?;
        let n: u32 = rest[..colon].parse().ok()?;
        let keyword = rest[colon + 1..].to_string();
        Some((n, keyword))
    }

    /// Parse `"歌名 - 歌手"` from `data[0]` into `(title, artist)`.
    fn parse_title_artist(data: &[String]) -> (String, String) {
        let first = data.first().map(|s| s.as_str()).unwrap_or("");
        if let Some(pos) = first.find(" - ") {
            (first[pos + 3..].trim().to_string(), first[..pos].trim().to_string())
        } else {
            (first.to_string(), String::new())
        }
    }
}

#[async_trait::async_trait]
impl LyricsProvider for FiftyTwoVmyProvider {
    fn name(&self) -> &str {
        &self.base_url
    }

    /// Search by issuing concurrent requests for n=1,2,3 (3 requests stay
    /// within the 4-per-2s QPS limit). Each response may represent a
    /// different version of the matched song.
    async fn search(&self, keyword: &str) -> anyhow::Result<Vec<LyricSearchResult>> {
        // Issue 3 concurrent requests for different result versions.
        let (res1, res2, res3) = tokio::join!(
            self.client
                .get(self.lrc_url())
                .query(&[("msg", keyword), ("n", "1"), ("type", "json")])
                .send(),
            self.client
                .get(self.lrc_url())
                .query(&[("msg", keyword), ("n", "2"), ("type", "json")])
                .send(),
            self.client
                .get(self.lrc_url())
                .query(&[("msg", keyword), ("n", "3"), ("type", "json")])
                .send(),
        );

        let mut results = Vec::new();

        for (n, resp_result) in [(1u32, res1), (2u32, res2), (3u32, res3)] {
            let resp = match resp_result {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!("52VMY n={} request error: {}", n, e);
                    continue;
                }
            };
            if !resp.status().is_success() {
                tracing::debug!("52VMY n={} HTTP {}", n, resp.status());
                continue;
            }
            let body: VmyLrcResponse = match resp.json().await {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!("52VMY n={} JSON error: {}", n, e);
                    continue;
                }
            };
            if body.code != 200 {
                continue;
            }
            let data = match body.data {
                Some(ref d) if !d.is_empty() => d,
                _ => continue,
            };
            let (title, artist) = Self::parse_title_artist(data);
            let key = Self::composite_key(n, keyword);
            results.push(LyricSearchResult {
                id: key,
                title,
                artist,
                album: None,
                duration_ms: None,
                source: self.base_url.clone(),
            });
        }

        Ok(results)
    }

    /// Fetch LRC text using the default TEXT mode (no `type=json`), which
    /// returns standard `[mm:ss.xx]` LRC format.
    async fn fetch_lrc(&self, composite_key: &str) -> anyhow::Result<Option<LrcFile>> {
        let (n, keyword) = match Self::parse_key(composite_key) {
            Some(v) => v,
            None => return Err(anyhow::anyhow!("Invalid 52VMY lyrics key: {}", composite_key)),
        };

        let resp = self
            .client
            .get(self.lrc_url())
            .query(&[("msg", &*keyword), ("n", &*n.to_string())])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }
        let text = resp.text().await?;
        if text.is_empty() || text.contains("<html") {
            return Ok(None);
        }
        Ok(Some(crate::lyrics::parser::parse_lrc(&text)))
    }
}

#[cfg(test)]
mod provider_tests {
    use super::*;

    // – LrclibProvider tests –

    #[test]
    fn lrclib_composite_key_roundtrip() {
        let key = LrclibProvider::composite_key(17788);
        let id = LrclibProvider::parse_id_from_key(&key).unwrap();
        assert_eq!(id, 17788);
    }

    #[test]
    fn lrclib_parse_id_rejects_foreign_format() {
        assert!(LrclibProvider::parse_id_from_key("52vmy:1:test").is_none());
        assert!(LrclibProvider::parse_id_from_key("garbage").is_none());
    }

    #[test]
    fn lrclib_pick_lrc_prefers_synced() {
        let item = LrclibSearchItem {
            id: 1, track_name: "test".into(), artist_name: "test".into(),
            album_name: None, duration: None, instrumental: false,
            synced_lyrics: Some("[00:01.00]synced".into()),
            plain_lyrics: Some("plain".into()),
        };
        assert_eq!(LrclibProvider::pick_lrc(&item).unwrap(), "[00:01.00]synced");
    }

    #[test]
    fn lrclib_pick_lrc_falls_back_to_plain() {
        let item = LrclibSearchItem {
            id: 2, track_name: "test".into(), artist_name: "test".into(),
            album_name: None, duration: None, instrumental: false,
            synced_lyrics: None, plain_lyrics: Some("plain text".into()),
        };
        assert_eq!(LrclibProvider::pick_lrc(&item).unwrap(), "plain text");
    }

    #[test]
    fn lrclib_deserialize_search_response() {
        let json = r#"[
            {"id":1,"trackName":"晴天","artistName":"周杰伦","albumName":"叶惠美","duration":270,"instrumental":false,"syncedLyrics":"[00:01.00]晴天","plainLyrics":"晴天\n"}
        ]"#;
        let items: Vec<LrclibSearchItem> = serde_json::from_str(json).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].track_name, "晴天");
        assert_eq!(items[0].album_name, Some("叶惠美".into()));
        assert_eq!(items[0].duration, Some(270.0));
    }

    // – FiftyTwoVmyProvider tests –

    #[test]
    fn vmy_composite_key_roundtrip() {
        let key = FiftyTwoVmyProvider::composite_key(2, "晴天");
        let (n, kw) = FiftyTwoVmyProvider::parse_key(&key).unwrap();
        assert_eq!(n, 2);
        assert_eq!(kw, "晴天");
    }

    #[test]
    fn vmy_parse_key_rejects_foreign_format() {
        assert!(FiftyTwoVmyProvider::parse_key("lrclib:123").is_none());
        assert!(FiftyTwoVmyProvider::parse_key("garbage").is_none());
    }

    #[test]
    fn vmy_parse_title_artist_with_dash() {
        let data = vec!["周杰伦 - 晴天".into()];
        let (title, artist) = FiftyTwoVmyProvider::parse_title_artist(&data);
        assert_eq!(title, "晴天");
        assert_eq!(artist, "周杰伦");
    }

    #[test]
    fn vmy_parse_title_artist_no_dash() {
        let data = vec!["晴天".into()];
        let (title, artist) = FiftyTwoVmyProvider::parse_title_artist(&data);
        assert_eq!(title, "晴天");
        assert_eq!(artist, "");
    }

    #[test]
    fn vmy_deserialize_response() {
        let json = r#"{"code":200,"msg":"成功","data":["周杰伦 - 晴天","词：周杰伦","line1","line2"]}"#;
        let resp: VmyLrcResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.code, 200);
        let data = resp.data.unwrap();
        assert_eq!(data[0], "周杰伦 - 晴天");
        assert_eq!(data[2], "line1");
    }

    // – Registry tests –

    #[test]
    fn registry_with_defaults_has_lrclib_and_52vmy() {
        let reg = LyricsProviderRegistry::with_defaults();
        let servers = reg.get_servers();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0], DEFAULT_LRCLIB_URL);
        assert_eq!(servers[1], DEFAULT_52VMY_URL);
    }

    #[test]
    fn registry_set_servers_all_52vmy() {
        let mut reg = LyricsProviderRegistry::new();
        reg.set_servers(vec![
            "https://api.52vmy.cn".to_string(),
            "https://my-proxy.example.com".to_string(),
            "  ".to_string(),
            "https://api.52vmy.cn".to_string(),
        ]);
        let servers = reg.get_servers();
        assert_eq!(servers.len(), 2);
        assert_eq!(servers[0], "https://api.52vmy.cn");
        assert_eq!(servers[1], "https://my-proxy.example.com");
    }

    #[test]
    fn registry_set_servers_empty_falls_back_to_default() {
        let mut reg = LyricsProviderRegistry::new();
        reg.set_servers(vec!["   ".to_string()]);
        let servers = reg.get_servers();
        assert_eq!(servers, vec![DEFAULT_52VMY_URL]);
    }
}
