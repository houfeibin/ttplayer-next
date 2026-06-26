use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

/// Register global media control shortcuts
pub fn setup_hotkeys(app: &AppHandle) -> anyhow::Result<()> {
    let gs = app.global_shortcut();

    // Play/Pause: Ctrl+Alt+P
    let play_pause = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyP);
    let app_clone = app.clone();
    gs.on_shortcut(play_pause, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            let _ = app_clone.emit("global-hotkey", "play_pause");
        }
    })?;

    // Next: Ctrl+Alt+Right
    let next = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowRight);
    let app_clone = app.clone();
    gs.on_shortcut(next, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            let _ = app_clone.emit("global-hotkey", "next");
        }
    })?;

    // Prev: Ctrl+Alt+Left
    let prev = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowLeft);
    let app_clone = app.clone();
    gs.on_shortcut(prev, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            let _ = app_clone.emit("global-hotkey", "prev");
        }
    })?;

    // Stop: Ctrl+Alt+S
    let stop = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS);
    let app_clone = app.clone();
    gs.on_shortcut(stop, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            let _ = app_clone.emit("global-hotkey", "stop");
        }
    })?;

    // Volume Up: Ctrl+Alt+Up
    let vol_up = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowUp);
    let app_clone = app.clone();
    gs.on_shortcut(vol_up, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            let _ = app_clone.emit("global-hotkey", "volume_up");
        }
    })?;

    // Volume Down: Ctrl+Alt+Down
    let vol_down = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowDown);
    let app_clone = app.clone();
    gs.on_shortcut(vol_down, move |_app, _shortcut, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            let _ = app_clone.emit("global-hotkey", "volume_down");
        }
    })?;

    tracing::info!("Global hotkeys registered: Ctrl+Alt+P/Left/Right/S/Up/Down");
    Ok(())
}
