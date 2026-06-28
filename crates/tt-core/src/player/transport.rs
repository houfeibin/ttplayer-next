use std::path::Path;
use std::sync::atomic::Ordering;
use tokio::task;

use crate::output::AudioOutput;
use tt_common::{ErrorKind, PlaybackError, PlaybackState};

use super::state_to_code;

impl super::AudioPipeline {
    /// Open and start playing. Returns immediately.
    pub fn open_and_play(&mut self, path: &Path) -> anyhow::Result<()> {
        self.open_and_play_at(path, 0)
    }

    /// Classify an `anyhow::Error` into an `ErrorKind` by inspecting the
    /// error message. This is a best-effort heuristic since `anyhow` doesn't
    /// carry typed error variants — but it covers the common cases (unsupported
    /// format, IO errors, prebuffer timeout) well enough for logging.
    fn classify_error(e: &anyhow::Error) -> ErrorKind {
        let msg = e.to_string().to_lowercase();
        if msg.contains("unsupported format") {
            ErrorKind::UnknownFormat
        } else if msg.contains("prebuffer timeout") {
            ErrorKind::Cancelled
        } else if msg.contains("no such file")
            || msg.contains("not found")
            || msg.contains("permission denied")
            || msg.contains("access is denied")
        {
            ErrorKind::IoError
        } else if msg.contains("seek") {
            ErrorKind::SeekError
        } else {
            ErrorKind::DecoderError
        }
    }

    /// Open and start playing from a given position (ms).
    pub fn open_and_play_at(&mut self, path: &Path, seek_ms: u64) -> anyhow::Result<()> {
        self.stop_inner();
        // Clear any error from the previous track so the frontend doesn't see
        // a stale error while the new track is loading.
        self.clear_error();

        self.state.store(state_to_code(PlaybackState::Loading), Ordering::Relaxed);
        *self.current_file.lock() = Some(path.to_path_buf());
        self.crossfade_pending.store(false, Ordering::Relaxed);
        *self.crossfade_rx.lock() = None;

        // Read tags off the command thread. Tag parsing (especially base64
        // cover-art encoding for large ID3/APIC frames) can take ~50ms, which
        // would block this command and any other player command queued behind
        // it. We instead spawn a blocking task: the result lands in `metadata`
        // as soon as it's ready (typically well before the 500ms prebuffer
        // completes, so audio still starts with correct metadata + ReplayGain),
        // and bumps `metadata_rev` so the event-push thread re-emits it.
        let tags_path = path.to_path_buf();
        let metadata_slot = self.metadata.clone();
        let metadata_rev = self.metadata_rev.clone();
        let dsp_chain_slot = self.dsp_chain.clone();
        // 克隆 current_file 用于完成时校验：快速切歌时旧歌的异步 tag 读取
        // 可能在新歌已开始播放后才完成，此时必须丢弃旧歌的 metadata，
        // 防止"显示A的标签却播放B"的错位。与 refresh_metadata_if_current 一致。
        let current_file_slot = self.current_file.clone();
        task::spawn_blocking(move || {
            if let Ok(tags) = tt_tags::read(&tags_path) {
                // 校验：仅当 current_file 仍是原路径时才写入 metadata
                let is_current = current_file_slot.lock().as_ref()
                    .map(|p| p == &tags_path)
                    .unwrap_or(false);
                if !is_current { return; }
                if let Some(ref rg) = tags.replay_gain {
                    if let Some(rg_proc) = dsp_chain_slot.lock().replay_gain() {
                        rg_proc.set_from_rg(rg);
                    }
                }
                *metadata_slot.lock() = tags;
                metadata_rev.fetch_add(1, Ordering::Relaxed);
            }
        });

        let path_buf = path.to_path_buf();
        let path_for_error = path_buf.to_string_lossy().to_string();
        let state = self.state.clone();
        let state_for_watchdog = self.state.clone();
        let volume = self.volume.clone();
        let duration = self.file_duration_ms.clone();
        let dsp_chain = self.dsp_chain.clone();
        let ring_slot = self.ring_slot.clone();
        let output_slot = self.output_slot.clone();
        let crossfade_enabled = self.crossfade_enabled.clone();
        let crossfade_ms = self.crossfade_ms.clone();
        let crossfade_pending = self.crossfade_pending.clone();
        let crossfade_rx = self.crossfade_rx.clone();
        let next_provider = self.next_provider.clone();
        let last_error_slot = self.last_error.clone();
        // Clone before moving into the spawn_blocking closure — the watchdog
        // below also needs these values.
        let last_error_for_watchdog = last_error_slot.clone();
        let track_path_for_watchdog = path_for_error.clone();

        task::spawn_blocking(move || {
            let state_for_reset = state.clone();
            let last_error_clone = last_error_slot.clone();
            let track_path_for_err = path_for_error.clone();
            if let Err(e) = Self::bg_play_at(
                path_buf, seek_ms, state, volume, duration,
                dsp_chain, ring_slot, output_slot,
                crossfade_enabled, crossfade_ms, crossfade_pending, crossfade_rx,
                next_provider,
                last_error_slot,
                path_for_error,
            ) {
                tracing::error!("bg_play failed: {}", e);
                // Record the error with full context and transition to Error
                // state so the frontend can log details and auto-skip.
                let kind = Self::classify_error(&e);
                let pb_err = PlaybackError::new(kind, e.to_string(), Some(track_path_for_err));
                *last_error_clone.lock() = Some(pb_err);
                state_for_reset.store(state_to_code(PlaybackState::Error), Ordering::Relaxed);
            }
        });

        // Watchdog: if the pipeline is still Loading after 3s (e.g. the decode
        // thread panicked or hung before reaching Playing), record a timeout
        // error so the frontend can auto-skip.
        {
            let state_wd = state_for_watchdog;
            let last_error_wd = last_error_for_watchdog;
            let track_path_wd = track_path_for_watchdog;
            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                if state_wd.load(Ordering::Relaxed) == state_to_code(PlaybackState::Loading) {
                    tracing::error!("Playback stuck in Loading for >3s, treating as error");
                    let err = PlaybackError::new(
                        ErrorKind::Cancelled,
                        "Loading timed out (>3s)",
                        Some(track_path_wd),
                    );
                    *last_error_wd.lock() = Some(err);
                    state_wd.store(state_to_code(PlaybackState::Error), Ordering::Relaxed);
                }
            });
        }

        Ok(())
    }

    pub(crate) fn stop_inner(&mut self) {
        if let Some(mut output) = self.output_slot.lock().take() {
            output.stop();
        }
        *self.ring_slot.lock() = None;
        *self.current_file.lock() = None;
        self.crossfade_pending.store(false, Ordering::Relaxed);
        *self.crossfade_rx.lock() = None;
        self.state.store(state_to_code(PlaybackState::Stopped), Ordering::Relaxed);
    }

    pub fn pause(&mut self) {
        if self.state.load(Ordering::Relaxed) == state_to_code(PlaybackState::Playing) {
            if let Some(ref mut output) = *self.output_slot.lock() {
                output.stop();
            }
            self.state.store(state_to_code(PlaybackState::Paused), Ordering::Relaxed);
        }
    }

    pub fn resume(&mut self) {
        if self.state.load(Ordering::Relaxed) == state_to_code(PlaybackState::Paused) {
            let ring = self.ring_slot.lock().clone();
            if let Some(ring) = ring {
                if let Ok(output) = AudioOutput::start(ring) {
                    *self.output_slot.lock() = Some(output);
                    self.state.store(state_to_code(PlaybackState::Playing), Ordering::Relaxed);
                }
            }
        }
    }

    pub fn stop(&mut self) { self.stop_inner(); }

    pub fn seek(&mut self, position_ms: u64) -> anyhow::Result<()> {
        let path = self.current_file.lock().clone();
        if let Some(path) = path {
            self.open_and_play_at(&path, position_ms)?;
            return Ok(());
        }
        anyhow::bail!("no file loaded")
    }
}
