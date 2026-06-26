use tauri::State;

use crate::state::AppState;

/// Load lyrics from an LRC file path
#[tauri::command]
pub async fn lyrics_load(
    state: State<'_, AppState>,
    path: String,
) -> Result<bool, String> {
    let lrc_path = std::path::Path::new(&path);
    match tt_core::lyrics::parser::read_lrc_file(lrc_path) {
        Some(lrc) => {
            let mut engine = state.lyrics.lock();
            engine.load(lrc);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Search for LRC files matching an audio file path
#[tauri::command]
pub async fn lyrics_search(
    _state: State<'_, AppState>,
    audio_path: String,
) -> Result<Vec<String>, String> {
    let audio = std::path::Path::new(&audio_path);
    let results = tt_core::lyrics::parser::search_lrc_files(audio);
    Ok(results.iter().map(|p| p.to_string_lossy().to_string()).collect())
}

/// Auto-find and load lyrics for an audio file
#[tauri::command]
pub async fn lyrics_auto_load(
    state: State<'_, AppState>,
    audio_path: String,
) -> Result<bool, String> {
    let audio = std::path::Path::new(&audio_path);
    let results = tt_core::lyrics::parser::search_lrc_files(audio);
    if let Some(lrc_path) = results.first() {
        if let Some(lrc) = tt_core::lyrics::parser::read_lrc_file(lrc_path) {
            let mut engine = state.lyrics.lock();
            engine.load(lrc);
            return Ok(true);
        }
    }
    Ok(false)
}

/// Update lyrics timing with current playback position (ms)
/// Returns the current line index, text, and progress
#[tauri::command]
pub async fn lyrics_update(
    state: State<'_, AppState>,
    position_ms: u64,
) -> Result<serde_json::Value, String> {
    let mut engine = state.lyrics.lock();
    let update = engine.update(position_ms);
    Ok(serde_json::json!({
        "index": update.index,
        "text": update.text,
        "progress": update.progress,
        "totalLines": update.total_lines,
        "changed": update.changed,
    }))
}

/// Get all lyrics lines for frontend rendering
#[tauri::command]
pub async fn lyrics_get_lines(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let engine = state.lyrics.lock();
    let lines: Vec<serde_json::Value> = engine.lines().iter().map(|line| {
        serde_json::json!({
            "timeMs": line.time_ms,
            "text": line.text,
            "words": line.words.as_ref().map(|words| {
                words.iter().map(|w| {
                    serde_json::json!({
                        "timeMs": w.time_ms,
                        "text": w.text,
                    })
                }).collect::<Vec<_>>()
            }),
        })
    }).collect();
    Ok(lines)
}

/// Clear loaded lyrics
#[tauri::command]
pub async fn lyrics_clear(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.lyrics.lock().clear();
    Ok(())
}

/// Get lyrics metadata
#[tauri::command]
pub async fn lyrics_get_metadata(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let engine = state.lyrics.lock();
    Ok(serde_json::json!({
        "hasLyrics": engine.has_lyrics(),
        "totalLines": engine.lines().len(),
        "currentIndex": engine.current_index(),
    }))
}

/// Search online lyrics provider.
///
/// Requires a valid API token to be configured via `lyrics_set_token`.
/// Returns an error if no token is set.
#[tauri::command]
pub async fn lyrics_search_online(
    state: State<'_, AppState>,
    keyword: String,
) -> Result<Vec<serde_json::Value>, String> {
    let registry = state.lyrics_providers.read().await;
    let results = registry.search_with_failover(&keyword, 10).await?;
    Ok(results.iter().map(|r| {
        serde_json::json!({
            "id": r.id,
            "title": r.title,
            "artist": r.artist,
            "album": r.album,
            "durationMs": r.duration_ms,
            "source": r.source,
        })
    }).collect())
}

/// Fetch lyrics from an online provider and load into engine.
///
/// `source` must match a configured server URL (returned in search results).
#[tauri::command]
pub async fn lyrics_load_online(
    state: State<'_, AppState>,
    source: String,
    id: String,
) -> Result<bool, String> {
    let registry = state.lyrics_providers.read().await;
    match registry.fetch(&source, &id).await {
        Ok(Some(lrc)) => {
            if lrc.lines.is_empty() {
                return Ok(false);
            }
            let mut engine = state.lyrics.lock();
            engine.load(lrc);
            Ok(true)
        }
        Ok(None) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

// ── Token management ──────────────────────────────────────────────────────

/// Get the file path for persisting the lyrics API token.
fn token_file_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ttplayer-next").join("lyrics_token"))
}

/// Load persisted token from disk, returning None if not found.
pub fn load_token() -> Option<String> {
    let path = token_file_path()?;
    let content = std::fs::read_to_string(&path).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

/// Persist token to disk.
fn save_token(token: &str) {
    if let Some(path) = token_file_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, token);
    }
}

/// Set the API token for online lyrics search.
///
/// The token is validated (non-empty, alphanumeric) and persisted to disk.
/// Returns the current token after setting.
#[tauri::command]
pub async fn lyrics_set_token(
    state: State<'_, AppState>,
    token: String,
) -> Result<String, String> {
    let trimmed = token.trim().to_string();

    // Validate: non-empty and alphanumeric
    if trimmed.is_empty() {
        return Err("Token 不能为空".into());
    }
    if !trimmed.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err("Token 格式无效，仅允许字母、数字、下划线和连字符".into());
    }

    save_token(&trimmed);
    let mut registry = state.lyrics_providers.write().await;
    registry.set_token(trimmed);
    Ok(registry.get_token().unwrap_or("").to_string())
}

/// Get the current API token (masked: only first 4 and last 4 chars shown).
#[tauri::command]
pub async fn lyrics_get_token(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let registry = state.lyrics_providers.read().await;
    Ok(registry.get_token().unwrap_or("").to_string())
}

/// Check if a token is currently configured.
#[tauri::command]
pub async fn lyrics_has_token(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let registry = state.lyrics_providers.read().await;
    Ok(registry.has_token())
}

// ── Legacy server management (no-ops, kept for backward compat) ───────────

/// Get the list of configured lyrics server URLs (deprecated).
#[tauri::command]
pub async fn lyrics_get_servers() -> Result<Vec<String>, String> {
    Ok(vec![tt_core::lyrics::OPENAPI_BASE_URL.to_string()])
}

/// Replace the lyrics server list (deprecated — no-op).
#[tauri::command]
pub async fn lyrics_set_servers(
    urls: Vec<String>,
) -> Result<Vec<String>, String> {
    // Token-based API now; server list is fixed.
    let _ = urls;
    Ok(vec![tt_core::lyrics::OPENAPI_BASE_URL.to_string()])
}

/// Save the currently loaded lyrics as `{audio_stem}.lrc` in the same
/// directory as the audio file.
///
/// Returns the saved file path on success.  Does nothing if no lyrics
/// are currently loaded.
#[tauri::command]
pub async fn lyrics_save_to_file(
    state: State<'_, AppState>,
    audio_path: String,
) -> Result<String, String> {
    let audio = std::path::Path::new(&audio_path);
    let engine = state.lyrics.lock();
    if !engine.has_lyrics() {
        return Err("No lyrics loaded".into());
    }
    let lrc = engine.to_lrcfile();
    let target = tt_core::lyrics::parser::lrc_path_for_audio(audio)
        .ok_or_else(|| "Cannot determine LRC path for audio file".to_string())?;
    tt_core::lyrics::parser::write_lrc_file(&target, &lrc)
        .map_err(|e| e.to_string())?;
    tracing::info!("Saved lyrics to {}", target.display());
    Ok(target.to_string_lossy().to_string())
}