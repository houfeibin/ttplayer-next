#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

fn main() {
    // WebView2 默认使用 %LOCALAPPDATA%\<identifier>\EBWebView 作为用户数据目录。
    // 当应用异常退出时，子进程 msedgewebview2.exe 可能未被清理，持有目录锁，
    // 导致后续启动报错 HRESULT(0x800700AA) "请求的资源正在使用中"，
    // 表现为 webview 无法创建、白屏、或加载旧缓存代码。
    //
    // 将 WEBVIEW2_USER_DATA_FOLDER 指向 TEMP 下的专用目录，避开被锁的默认目录。
    // TEMP 目录在系统重启时会自动清理，不会累积残留锁文件。
    #[cfg(target_os = "windows")]
    {
        if let Some(temp_dir) = std::env::var_os("TEMP") {
            let webview_data_dir = std::path::PathBuf::from(temp_dir)
                .join("ttplayer-next-webview2");
            let _ = std::fs::create_dir_all(&webview_data_dir);
            // Rust 2024 edition 起 set_var 标记为 unsafe
            unsafe {
                std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_data_dir);
            }
        }
    }

    ttplayer_next_lib::run();
}
