use tauri::{
    AppHandle, Emitter, Manager,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    menu::{Menu, MenuItem, PredefinedMenuItem},
};

/// Build and attach the system tray icon
pub fn setup_tray(app: &AppHandle) -> anyhow::Result<()> {
    // Create tray menu
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let play_pause = MenuItem::with_id(app, "play_pause", "播放/暂停", true, None::<&str>)?;
    let prev = MenuItem::with_id(app, "prev", "上一首", true, None::<&str>)?;
    let next = MenuItem::with_id(app, "next", "下一首", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[&show, &play_pause, &prev, &next, &separator, &quit])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .tooltip("TTPlayer-Next")
        .on_menu_event(move |app, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "play_pause" => {
                    // Emit event to frontend to handle
                    let _ = app.emit("tray-action", "play_pause");
                }
                "prev" => {
                    let _ = app.emit("tray-action", "prev");
                }
                "next" => {
                    let _ = app.emit("tray-action", "next");
                }
                "quit" => {
                    app.exit(0);
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
