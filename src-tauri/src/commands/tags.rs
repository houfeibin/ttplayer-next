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
