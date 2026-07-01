use std::collections::HashMap;
use tauri::State;
use crate::state::AppState;

/// Get metadata for a file
#[tauri::command]
pub async fn tags_read(
    _state: State<'_, AppState>,
    path: String,
) -> Result<serde_json::Value, String> {
    let metadata = tt_tags::read(std::path::Path::new(&path))
        .map_err(|e| e.to_string())?;
    serde_json::to_value(&metadata).map_err(|e| e.to_string())
}

/// Write metadata tags to a file.
///
/// After a successful write, if the edited file is the currently loaded track,
/// the player's cached metadata is refreshed asynchronously so the UI reflects
/// the changes immediately (within ~50ms via the event-push thread) without
/// restarting or manual refresh. Playback continues uninterrupted.
#[tauri::command]
pub async fn tags_write(
    state: State<'_, AppState>,
    path: String,
    updates: HashMap<String, String>,
) -> Result<(), String> {
    tt_tags::write(std::path::Path::new(&path), &updates)
        .map_err(|e| e.to_string())?;
    // Refresh the player's in-memory metadata cache if this file is the
    // current track. Spawns a blocking task internally (tag parsing ~50ms),
    // so this returns immediately without blocking the command thread.
    state.player.lock().refresh_metadata_if_current(std::path::Path::new(&path));
    Ok(())
}

/// Read tags for many files. Runs on a blocking thread because each file
/// parse (~50ms × N) would otherwise stall the command thread.
#[tauri::command]
pub async fn tags_read_batch(
    _state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<Vec<tt_tags::BatchTagResult>, String> {
    tokio::task::spawn_blocking(move || tt_tags::read_batch(&paths))
        .await
        .map_err(|e| e.to_string())
}

/// Write tags to many files. Each file is written independently; failures on
/// some files are reported per-file in the returned results (not as a global
/// error) so partial success is surfaced to the user.
///
/// After writing, any file that matches the currently loaded track has its
/// in-memory metadata cache refreshed so the UI updates without a restart.
#[tauri::command]
pub async fn tags_write_batch(
    state: State<'_, AppState>,
    edits: Vec<tt_tags::BatchTagEdit>,
) -> Result<Vec<tt_tags::BatchTagResult>, String> {
    let player = state.player.clone();
    // Collect paths before moving `edits` into the blocking closure, so we can
    // refresh the player's metadata cache afterwards.
    let paths: Vec<String> = edits.iter().map(|e| e.path.clone()).collect();
    let results = tokio::task::spawn_blocking(move || tt_tags::write_batch(&edits))
        .await
        .map_err(|e| e.to_string())?;
    // Refresh metadata for any edited file that is the current track.
    for path in &paths {
        player
            .lock()
            .refresh_metadata_if_current(std::path::Path::new(path));
    }
    Ok(results)
}
