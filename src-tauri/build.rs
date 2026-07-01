fn main() {
    // ── Centralized version (single source: ../../version.json) ────────
    // Reads the project version at compile time so `env!("APP_VERSION")` is
    // available everywhere in Rust code. Simple inline parser to avoid adding
    // serde_json as a build dependency.
    let version = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("version.json"),
    )
    .ok()
    .and_then(|s| {
        // Minimal JSON parser for { "version": "x.y.z" } — no serde_json needed
        let after_key = s.find("\"version\"")?;
        let after_colon = s[after_key..].find(':')?;
        let after_open = s[after_key + after_colon..].find('"')?;
        let start = after_key + after_colon + after_open + 1;
        let end = s[start..].find('"')?;
        Some(s[start..start + end].to_string())
    })
    .unwrap_or_else(|| "0.0.0".into());
    println!("cargo:rustc-env=APP_VERSION={version}");

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
