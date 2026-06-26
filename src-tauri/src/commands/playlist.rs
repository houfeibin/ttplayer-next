use tauri::State;
use std::path::PathBuf;

use crate::state::AppState;

/// Add files to playlist
#[tauri::command]
pub async fn playlist_add_files(
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<usize, String> {
    let mut playlist = state.playlist.lock();
    let pbs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    playlist.active_mut().add_files(&pbs);
    let len = playlist.active().len();
    state.playlist_saver.mark_dirty();
    Ok(len)
}

/// Get playlist items
#[tauri::command]
pub async fn playlist_get_items(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let playlist = state.playlist.lock();
    let items: Vec<_> = playlist.active().items
        .iter()
        .map(|item| serde_json::json!({
            "path": item.path,
            "format": format!("{:?}", item.format),
        }))
        .collect();
    Ok(serde_json::json!({ "items": items, "currentIndex": playlist.active().current_index }))
}

/// Play next track
#[tauri::command]
pub async fn playlist_next(
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let next_path = {
        let mut playlist = state.playlist.lock();
        let p = playlist.active_mut().next().map(|s| s.to_string());
        state.playlist_saver.mark_dirty();
        p
    };

    if let Some(ref path) = next_path {
        let mut player = state.player.lock();
        player.open_and_play(PathBuf::from(path).as_path())
            .map_err(|e| e.to_string())?;
    }

    Ok(next_path)
}

/// Play previous track
#[tauri::command]
pub async fn playlist_prev(
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let prev_path = {
        let mut playlist = state.playlist.lock();
        let p = playlist.active_mut().prev().map(|s| s.to_string());
        state.playlist_saver.mark_dirty();
        p
    };

    if let Some(ref path) = prev_path {
        let mut player = state.player.lock();
        player.open_and_play(PathBuf::from(path).as_path())
            .map_err(|e| e.to_string())?;
    }

    Ok(prev_path)
}

/// Play track at specific index
#[tauri::command]
pub async fn playlist_play_index(
    state: State<'_, AppState>,
    index: usize,
) -> Result<(), String> {
    let path = {
        let mut playlist = state.playlist.lock();
        playlist.active_mut().current_index = index;
        let p = playlist.active()
            .items
            .get(index)
            .map(|t| t.path.clone());
        state.playlist_saver.mark_dirty();
        p
    };

    if let Some(path) = path {
        let mut player = state.player.lock();
        player.open_and_play(PathBuf::from(path).as_path())
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

/// Clear playlist
#[tauri::command]
pub async fn playlist_clear(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut playlist = state.playlist.lock();
    playlist.active_mut().clear();
    state.playlist_saver.mark_dirty();
    Ok(())
}

/// Remove track
#[tauri::command]
pub async fn playlist_remove(
    state: State<'_, AppState>,
    index: usize,
) -> Result<(), String> {
    let mut playlist = state.playlist.lock();
    playlist.active_mut().remove(index);
    state.playlist_saver.mark_dirty();
    Ok(())
}

/// Move a track within the playlist (reorder).
/// `from` and `to` are zero-based indices.
#[tauri::command]
pub async fn playlist_move_item(
    state: State<'_, AppState>,
    from: usize,
    to: usize,
) -> Result<(), String> {
    let mut playlist = state.playlist.lock();
    playlist.active_mut().move_item(from, to);
    state.playlist_saver.mark_dirty();
    Ok(())
}

/// Recursively scan a folder for audio files and add them to the playlist.
/// Returns the number of files added.
#[tauri::command]
pub async fn playlist_add_folder(
    state: State<'_, AppState>,
    folder: String,
) -> Result<usize, String> {
    let dir = PathBuf::from(&folder);
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", folder));
    }
    let mut playlist = state.playlist.lock();
    let count = playlist.active_mut().add_folder(&dir);
    state.playlist_saver.mark_dirty();
    Ok(count)
}

/// Get the current play mode of the active playlist.
#[tauri::command]
pub async fn playlist_get_play_mode(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let playlist = state.playlist.lock();
    Ok(format!("{:?}", playlist.active().play_mode).to_lowercase())
}

/// Set the play mode of the active playlist.
#[tauri::command]
pub async fn playlist_set_play_mode(
    state: State<'_, AppState>,
    mode: String,
) -> Result<(), String> {
    let mode = match mode.to_lowercase().as_str() {
        "single" => tt_common::PlayMode::Single,
        "sequential" => tt_common::PlayMode::Sequential,
        "loop" => tt_common::PlayMode::Loop,
        "loop_one" | "loopone" => tt_common::PlayMode::LoopOne,
        "random" => tt_common::PlayMode::Random,
        other => return Err(format!("Unknown play mode: {}", other)),
    };
    let mut playlist = state.playlist.lock();
    playlist.active_mut().set_play_mode(mode);
    state.playlist_saver.mark_dirty();
    Ok(())
}
