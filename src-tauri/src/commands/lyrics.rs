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

/// Search online lyrics providers.
///
/// Uses the shared registry (reuses HTTP client + LRC caches across calls).
/// Providers are queried in priority order with failover; the first server
/// returning results wins. At most 10 results are returned.
#[tauri::command]
pub async fn lyrics_search_online(
    state: State<'_, AppState>,
    keyword: String,
) -> Result<Vec<serde_json::Value>, String> {
    let registry = state.lyrics_providers.read().await;
    let results = registry.search_with_failover(&keyword, 10).await;
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
            let mut engine = state.lyrics.lock();
            engine.load(lrc);
            Ok(true)
        }
        Ok(None) => Ok(false),
        Err(e) => Err(e.to_string()),
    }
}

/// Get the list of configured lyrics server URLs (in priority order).
#[tauri::command]
pub async fn lyrics_get_servers(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let registry = state.lyrics_providers.read().await;
    Ok(registry.get_servers())
}

/// Replace the lyrics server list.
///
/// Each entry must be a 52VMY-compatible server base URL
/// (e.g. `https://api.52vmy.cn`). Servers are queried in the order
/// given; the first returning results wins (failover).
#[tauri::command]
pub async fn lyrics_set_servers(
    state: State<'_, AppState>,
    urls: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut registry = state.lyrics_providers.write().await;
    registry.set_servers(urls);
    Ok(registry.get_servers())
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
