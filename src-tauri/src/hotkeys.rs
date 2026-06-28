use tauri::{AppHandle, Emitter};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};

/// Register a single global shortcut, logging a warning instead of failing
/// when the hotkey is already registered (e.g. by a zombie process from a
/// previous run, or by another application). This prevents a single conflict
/// from crashing the entire app during setup.
fn register(
    app: &AppHandle,
    shortcut: Shortcut,
    label: &str,
    handler: impl Fn(&AppHandle) + Send + Sync + 'static,
) {
    let gs = app.global_shortcut();
    match gs.on_shortcut(shortcut, move |app, _sc, event| {
        if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
            handler(app);
        }
    }) {
        Ok(()) => {}
        Err(e) => {
            tracing::warn!("Failed to register hotkey {} (likely already held by another process): {}", label, e);
        }
    }
}

/// Register global media control shortcuts
pub fn setup_hotkeys(app: &AppHandle) -> anyhow::Result<()> {
    // Play/Pause: Ctrl+Alt+P
    register(
        app,
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyP),
        "Ctrl+Alt+P",
        |app| { let _ = app.emit("global-hotkey", "play_pause"); },
    );

    // Next: Ctrl+Alt+Right
    register(
        app,
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowRight),
        "Ctrl+Alt+Right",
        |app| { let _ = app.emit("global-hotkey", "next"); },
    );

    // Prev: Ctrl+Alt+Left
    register(
        app,
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowLeft),
        "Ctrl+Alt+Left",
        |app| { let _ = app.emit("global-hotkey", "prev"); },
    );

    // Stop: Ctrl+Alt+S
    register(
        app,
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyS),
        "Ctrl+Alt+S",
        |app| { let _ = app.emit("global-hotkey", "stop"); },
    );

    // Volume Up: Ctrl+Alt+Up
    register(
        app,
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowUp),
        "Ctrl+Alt+Up",
        |app| { let _ = app.emit("global-hotkey", "volume_up"); },
    );

    // Volume Down: Ctrl+Alt+Down
    register(
        app,
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::ArrowDown),
        "Ctrl+Alt+Down",
        |app| { let _ = app.emit("global-hotkey", "volume_down"); },
    );

    tracing::info!("Global hotkeys registration attempted: Ctrl+Alt+P/Left/Right/S/Up/Down");
    Ok(())
}
