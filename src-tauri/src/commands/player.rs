use tauri::State;
use std::path::PathBuf;

use crate::state::AppState;

/// Play a file (or toggle play/pause if already loaded)
#[tauri::command]
pub async fn player_play(
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<(), String> {
    let mut player = state.player.lock();

    if let Some(path) = path {
        let pb = PathBuf::from(&path);
        player.open_and_play(&pb).map_err(|e| e.to_string())?;
    } else if player.state() == tt_common::PlaybackState::Paused {
        player.resume();
    }

    Ok(())
}

/// Pause playback
#[tauri::command]
pub async fn player_pause(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.player.lock().pause();
    Ok(())
}

/// Stop playback
#[tauri::command]
pub async fn player_stop(
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.player.lock().stop();
    Ok(())
}

/// Toggle play/pause
#[tauri::command]
pub async fn player_toggle(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut player = state.player.lock();
    match player.state() {
        tt_common::PlaybackState::Playing => player.pause(),
        tt_common::PlaybackState::Paused => player.resume(),
        _ => {}
    }
    Ok(())
}

/// Get current playback state (includes spectrum data)
#[tauri::command]
pub async fn player_get_state(
    state: State<'_, AppState>,
) -> Result<serde_json::Value, String> {
    let player = state.player.lock();
    let mut state_json = serde_json::json!({
        "state": format!("{:?}", player.state()),
        "positionMs": player.position_ms(),
        "durationMs": player.duration_ms(),
        "sampleRate": player.sample_rate(),
        "channels": player.channels(),
        "volume": (player.volume() * 100.0) as u32,
        "currentFile": player.current_file().map(|p| p.to_string_lossy().to_string()),
        "metadata": player.metadata(),
        "crossfadePending": player.crossfade_is_pending(),
        "crossfadeActive": false,
        "spectrum": {
            "bands": [],
            "peak": 0.0_f32
        },
    });
    // Attach latest spectrum frame if available
    if let Some(ref spectrum) = player.spectrum() {
        let bands: Vec<f32> = spectrum.bands.iter().copied().take(256).collect();
        state_json["spectrum"] = serde_json::json!({
            "bands": bands,
            "peak": spectrum.peak,
        });
    }
    Ok(state_json)
}

/// Seek to position (milliseconds)
#[tauri::command]
pub async fn player_seek(
    state: State<'_, AppState>,
    position_ms: u64,
) -> Result<(), String> {
    state.player.lock().seek(position_ms).map_err(|e| e.to_string())
}

/// Set volume (0-100)
#[tauri::command]
pub async fn player_set_volume(
    state: State<'_, AppState>,
    volume: u8,
) -> Result<(), String> {
    let player = state.player.lock();
    player.set_volume(volume as f32 / 100.0);
    Ok(())
}

// ――― EQ commands ―――

/// Get EQ band gains (dB) — returns [f64; 10]
#[tauri::command]
pub async fn eq_get_bands(
    state: State<'_, AppState>,
) -> Result<Vec<f64>, String> {
    let player = state.player.lock();
    Ok(player.eq_bands().to_vec())
}

/// Set one EQ band gain (0..9, dB -12..+12)
#[tauri::command]
pub async fn eq_set_band(
    state: State<'_, AppState>,
    band: usize,
    gain_db: f64,
) -> Result<(), String> {
    let player = state.player.lock();
    player.set_eq_band(band, gain_db);
    Ok(())
}

/// Get EQ preamp (dB)
#[tauri::command]
pub async fn eq_get_preamp(
    state: State<'_, AppState>,
) -> Result<f64, String> {
    let player = state.player.lock();
    Ok(player.eq_preamp())
}

/// Set EQ preamp (dB)
#[tauri::command]
pub async fn eq_set_preamp(
    state: State<'_, AppState>,
    gain_db: f64,
) -> Result<(), String> {
    let player = state.player.lock();
    player.set_eq_preamp(gain_db);
    Ok(())
}

/// Reset EQ to flat
#[tauri::command]
pub async fn eq_reset(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let player = state.player.lock();
    player.eq_reset();
    Ok(())
}

// ――― Surround commands ―――

/// Set surround width (0-10 scale, 0=off, 8=default)
#[tauri::command]
pub async fn surround_set_width(
    state: State<'_, AppState>,
    width: u8,
) -> Result<(), String> {
    state.player.lock().set_surround_width(width);
    Ok(())
}

/// Get surround width (0-10)
#[tauri::command]
pub async fn surround_get_width(
    state: State<'_, AppState>,
) -> Result<u8, String> {
    Ok(state.player.lock().surround_width())
}

// ――― Crossfade commands ―――

/// Set crossfade duration in ms (0 to disable)
#[tauri::command]
pub async fn crossfade_set_duration(
    state: State<'_, AppState>,
    duration_ms: u64,
) -> Result<(), String> {
    state.player.lock().set_crossfade_duration_ms(duration_ms);
    Ok(())
}

/// Get crossfade duration in ms
#[tauri::command]
pub async fn crossfade_get_duration(
    state: State<'_, AppState>,
) -> Result<u64, String> {
    Ok(state.player.lock().crossfade_duration_ms())
}

/// Check if crossfade is pending (decode thread reached crossfade window)
#[tauri::command]
pub async fn crossfade_is_pending(
    state: State<'_, AppState>,
) -> Result<bool, String> {
    Ok(state.player.lock().crossfade_is_pending())
}
