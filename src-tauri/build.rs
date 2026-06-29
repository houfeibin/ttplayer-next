fn main() {
    // Workaround for intermittent `embed_resource` panic on Windows:
    // `rustc_version` calls `Command::new("rustc").output()` which sometimes
    // returns `Err(Os { code: 0 })` due to a Rust std bug on Windows.
    // We retry a few times to let the transient failure pass.
    let mut last_err = None;
    for _ in 0..5 {
        let result = std::panic::catch_unwind(|| tauri_build::build());
        if result.is_ok() {
            return;
        }
        last_err = result.err();
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // All retries failed — fall back to minimal cfg so compilation can proceed
    // (loses Windows resource embedding, but allows the app to build).
    eprintln!("[build.rs] tauri_build::build() panicked after retries: {:?}", last_err);
    println!("cargo:rustc-check-cfg=cfg(desktop)");
    println!("cargo:rustc-cfg=desktop");
    println!("cargo:rustc-check-cfg=cfg(mobile)");
    println!("cargo:rustc-check-cfg=cfg(dev)");
    #[cfg(debug_assertions)]
    println!("cargo:rustc-cfg=dev");
}
