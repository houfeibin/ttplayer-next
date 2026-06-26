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

/// Write metadata tags to a file
#[tauri::command]
pub async fn tags_write(
    _state: State<'_, AppState>,
    path: String,
    updates: HashMap<String, String>,
) -> Result<(), String> {
    tt_tags::write(std::path::Path::new(&path), &updates)
        .map_err(|e| e.to_string())
}
