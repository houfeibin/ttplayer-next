use std::sync::atomic::Ordering;
use tauri::State;
use crate::state::AppState;
use crate::state::ThemeMode;

/// Get the current theme mode: "light" | "dark" | "system"
#[tauri::command]
pub fn theme_get_mode(state: State<'_, AppState>) -> String {
    match state.theme_mode.load(Ordering::SeqCst) {
        0 => "light".into(),
        1 => "dark".into(),
        _ => "system".into(),
    }
}

/// Set the theme mode: "light" | "dark" | "system"
#[tauri::command]
pub fn theme_set_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let val: ThemeMode = match mode.as_str() {
        "light" => 0,
        "dark" => 1,
        "system" => 2,
        _ => return Err(format!("无效主题模式: {}", mode)),
    };
    state.theme_mode.store(val, Ordering::SeqCst);
    // Persist to a simple config file so it survives restarts.
    if let Err(e) = persist_theme_mode(&state.theme_mode) {
        tracing::warn!("Failed to persist theme mode: {}", e);
    }
    Ok(())
}

/// Load persisted theme mode, or default to "dark".
pub fn load_theme_mode() -> ThemeMode {
    let path = theme_config_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        match content.trim() {
            "light" => return 0,
            "system" => return 2,
            _ => {}
        }
    }
    1 // default: dark
}

fn persist_theme_mode(val: &std::sync::atomic::AtomicU8) -> std::io::Result<()> {
    let path = theme_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let s = match val.load(Ordering::SeqCst) {
        0 => "light",
        2 => "system",
        _ => "dark",
    };
    std::fs::write(path, s)
}

fn theme_config_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ttplayer-next")
        .join("theme_mode")
}
