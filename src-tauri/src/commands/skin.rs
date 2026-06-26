use tauri::State;
use crate::state::AppState;
use tt_core::skin::SkinInfo;

/// List all available skins
#[tauri::command]
pub fn skin_list(state: State<'_, AppState>) -> Vec<SkinInfo> {
    let skin = state.skin.lock();
    skin.list_skins()
}

/// Get current skin ID
#[tauri::command]
pub fn skin_get_current(state: State<'_, AppState>) -> String {
    let skin = state.skin.lock();
    skin.current_skin_id().to_string()
}

/// Apply a skin by ID, returns CSS variables string
#[tauri::command]
pub fn skin_apply(state: State<'_, AppState>, skin_id: String) -> Result<String, String> {
    let mut skin = state.skin.lock();
    skin.apply_skin(&skin_id).map_err(|e| e.to_string())
}

/// Install a .ttskin file
#[tauri::command]
pub fn skin_install(state: State<'_, AppState>, path: String) -> Result<SkinInfo, String> {
    let mut skin = state.skin.lock();
    let p = std::path::Path::new(&path);
    skin.install_skin(p).map_err(|e| e.to_string())
}

/// Get the runtime skins directory path (where skin folders live on disk)
#[tauri::command]
pub fn skin_get_dir(state: State<'_, AppState>) -> String {
    let skin = state.skin.lock();
    skin.skin_dir_path().to_string_lossy().to_string()
}

/// Delete a skin from disk by ID (default skin cannot be deleted)
#[tauri::command]
pub fn skin_delete(state: State<'_, AppState>, skin_id: String) -> Result<(), String> {
    let skin = state.skin.lock();
    skin.delete_skin(&skin_id).map_err(|e| e.to_string())
}

/// Open the skins directory in the OS file explorer
#[tauri::command]
pub fn skin_open_dir(state: State<'_, AppState>) -> Result<(), String> {
    let skin = state.skin.lock();
    let path = skin.skin_dir_path();

    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(path);
        c
    };

    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    };

    #[cfg(target_os = "linux")]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };

    cmd.spawn().map_err(|e| e.to_string())?;
    Ok(())
}
