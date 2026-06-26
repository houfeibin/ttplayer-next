#![cfg_attr(
    all(target_os = "windows", not(debug_assertions),),
    windows_subsystem = "windows"
)]

mod commands;
mod hotkeys;
mod state;
mod tray;

use state::AppState;
use std::sync::atomic::Ordering;
use tauri::{Manager, Emitter};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState::new())
        .setup(|app| {
            tray::setup_tray(app.handle())?;
            hotkeys::setup_hotkeys(app.handle())?;

            // Event push thread: emit player state every ~50ms (20fps)
            //
            // Optimization: spectrum is downsampled from 256→64 bands before
            // serialization (frontend only renders 32 bars anyway), and is
            // omitted entirely when not playing. State-only fields are still
            // pushed every tick so position/volume stay responsive.
            let app_handle = app.handle().clone();
            let state_ref = app.state::<AppState>().player.clone();
            let lyrics_ref = app.state::<AppState>().lyrics.clone();
            let shutdown = app.state::<AppState>().shutdown.clone();

            // Track previous file to skip redundant metadata pushes.
            let mut prev_file: Option<String> = None;
            // Track metadata revision so asynchronously-read tags (which land
            // after the file-change tick) still get emitted to the frontend.
            let mut prev_meta_rev: u64 = 0;

            std::thread::spawn(move || {
                loop {
                    if shutdown.load(Ordering::SeqCst) {
                        tracing::info!("Event push thread shutting down");
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    if shutdown.load(Ordering::SeqCst) {
                        tracing::info!("Event push thread shutting down");
                        break;
                    }
                    let player = state_ref.lock();
                    let st = player.state();
                    let state_code = match st {
                        tt_common::PlaybackState::Idle => 0u8,
                        tt_common::PlaybackState::Loading => 1,
                        tt_common::PlaybackState::Playing => 2,
                        tt_common::PlaybackState::Paused => 3,
                        tt_common::PlaybackState::Stopped => 4,
                    };
                    let pos = player.position_ms();
                    let dur = player.duration_ms();
                    let vol = (player.volume() * 100.0) as u32;
                    let cur_file = player.current_file().map(|p| p.to_string_lossy().to_string());
                    let meta_rev = player.metadata_rev();
                    let crossfade_pending = player.crossfade_is_pending();

                    // Downsample spectrum: 256 → 64 bands (4:1 grouping).
                    // Frontend Spectrum.tsx only renders 32 bars via nonlinear
                    // sampling, so 64 is more than enough and cuts payload ~75%.
                    const SPECTRUM_OUT: usize = 64;
                    let mut spectrum_bands: Vec<f32> = Vec::new();
                    let mut spectrum_peak = 0.0_f32;
                    let is_playing = state_code == 2;
                    if is_playing {
                        if let Some(ref sp) = player.spectrum() {
                            spectrum_bands.reserve(SPECTRUM_OUT);
                            const GROUP: usize = 256 / SPECTRUM_OUT; // 4
                            for chunk in sp.bands.chunks(GROUP) {
                                let avg = chunk.iter().sum::<f32>() / chunk.len() as f32;
                                spectrum_bands.push(avg);
                            }
                            spectrum_peak = sp.peak;
                        }
                    }
                    // Fill empty spectrum when not playing so frontend can decay.
                    if spectrum_bands.is_empty() {
                        spectrum_bands = vec![0.0; SPECTRUM_OUT];
                    }

                    let file_changed = cur_file != prev_file;
                    // Re-emit metadata if the file changed OR if asynchronously-
                    // read tags just landed (revision bumped since last tick).
                    let metadata_changed = meta_rev != prev_meta_rev;
                    prev_file = cur_file.clone();
                    prev_meta_rev = meta_rev;
                    // Clone metadata (which may carry a multi-MB base64 cover) only
                    // when it actually changed, instead of every 50 ms tick. This
                    // is the bulk of the lock hold time and was the real source of
                    // event-thread/command contention; the dsp_chain and crossfade
                    // fields already use their own fine-grained locks, so the outer
                    // lock now only guards transport ops (open/seek) briefly.
                    let meta = if file_changed || metadata_changed {
                        Some(player.metadata())
                    } else {
                        None
                    };
                    drop(player);

                    // Update lyrics timing in the same tick so the frontend
                    // doesn't need a separate 100ms invoke polling loop.
                    let lyrics_update = {
                        let mut engine = lyrics_ref.lock();
                        let u = engine.update(pos);
                        serde_json::json!({
                            "index": u.index,
                            "text": u.text,
                            "progress": u.progress,
                            "totalLines": u.total_lines,
                            "changed": u.changed,
                        })
                    };

                    let payload = serde_json::json!({
                        "state": format!("{:?}", st),
                        "positionMs": pos,
                        "durationMs": dur,
                        "volume": vol,
                        "currentFile": cur_file,
                        // Only embed full metadata when the file changes (it's
                        // large: includes base64 cover art) or when freshly-read
                        // tags arrive asynchronously. Otherwise omit.
                        "metadata": meta.as_ref(),
                        "crossfadePending": crossfade_pending,
                        "spectrum": { "bands": spectrum_bands, "peak": spectrum_peak },
                        "lyrics": lyrics_update,
                    });

                    if let Err(e) = app_handle.emit("player-state-update", &payload) {
                        tracing::error!("emit player-state-update failed: {}", e);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::player::player_play,
            commands::player::player_pause,
            commands::player::player_stop,
            commands::player::player_toggle,
            commands::player::player_get_state,
            commands::player::player_seek,
            commands::player::player_set_volume,
            commands::player::eq_get_bands,
            commands::player::eq_set_band,
            commands::player::eq_get_preamp,
            commands::player::eq_set_preamp,
            commands::player::eq_reset,
            commands::player::surround_set_width,
            commands::player::surround_get_width,
            commands::player::crossfade_set_duration,
            commands::player::crossfade_get_duration,
            commands::player::crossfade_is_pending,
            commands::playlist::playlist_add_files,
            commands::playlist::playlist_get_items,
            commands::playlist::playlist_next,
            commands::playlist::playlist_prev,
            commands::playlist::playlist_play_index,
            commands::playlist::playlist_clear,
            commands::playlist::playlist_remove,
            commands::playlist::playlist_move_item,
            commands::playlist::playlist_add_folder,
            commands::playlist::playlist_get_play_mode,
            commands::playlist::playlist_set_play_mode,
            commands::tags::tags_read,
            commands::tags::tags_write,
            commands::lyrics::lyrics_load,
            commands::lyrics::lyrics_search,
            commands::lyrics::lyrics_auto_load,
            commands::lyrics::lyrics_update,
            commands::lyrics::lyrics_get_lines,
            commands::lyrics::lyrics_clear,
            commands::lyrics::lyrics_get_metadata,
            commands::lyrics::lyrics_search_online,
            commands::lyrics::lyrics_load_online,
            commands::lyrics::lyrics_get_servers,
            commands::lyrics::lyrics_set_servers,
            commands::lyrics::lyrics_save_to_file,
            commands::lyrics::lyrics_set_token,
            commands::lyrics::lyrics_get_token,
            commands::lyrics::lyrics_has_token,
            commands::skin::skin_list,
            commands::skin::skin_get_current,
            commands::skin::skin_apply,
            commands::skin::skin_install,
            commands::skin::skin_get_dir,
            commands::skin::skin_delete,
            commands::skin::skin_open_dir,
            commands::theme::theme_get_mode,
            commands::theme::theme_set_mode,
            commands::desktop_lyrics::desktop_lyrics_get,
            commands::desktop_lyrics::desktop_lyrics_set,
            commands::desktop_lyrics::desktop_lyrics_reset,
            commands::file_props::file_get_properties,
            commands::convert::convert_files,
            commands::convert::convert_get_formats,
        ])
        .build(tauri::generate_context!())
        .expect("error while building TTPlayer-Next")
        .run(|app_handle, event| {
            // Graceful shutdown: signal background threads to stop and flush any
            // pending debounced playlist writes before the process exits.
            if let tauri::RunEvent::Exit = event {
                let state = app_handle.state::<AppState>();
                state.shutdown.store(true, Ordering::SeqCst);
                state.playlist_saver.flush_now();
                tracing::info!("TTPlayer-Next exiting");
            }
        });
}
