use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Instant;
use parking_lot::Mutex;

use crate::codecs::CodecRegistry;
use crate::dsp::crossfade::CrossfadeMixer;
use crate::dsp::DspChain;
use crate::output::{AudioOutput, PlaybackRing};
use tt_common::{ErrorKind, PlaybackError, PlaybackState};

use super::{
    state_to_code, NextTrackProvider, PREBUFFER_MS, SharedCrossfadeRx, SharedOutput, SharedRing,
};
use crate::output::AtomicVolume;

impl super::AudioPipeline {
    pub(crate) fn bg_play_at(
        path: PathBuf,
        seek_to_ms: u64,
        state: Arc<AtomicU8>,
        volume: Arc<AtomicVolume>,
        duration_ref: Arc<Mutex<u64>>,
        dsp_chain: Arc<Mutex<DspChain>>,
        ring_slot: SharedRing,
        output_slot: SharedOutput,
        crossfade_enabled: Arc<AtomicBool>,
        crossfade_ms: Arc<AtomicU64>,
        crossfade_pending: Arc<AtomicBool>,
        crossfade_rx: SharedCrossfadeRx,
        next_provider: Arc<Mutex<Option<Arc<dyn NextTrackProvider>>>>,
        last_error: Arc<Mutex<Option<PlaybackError>>>,
        track_path: String,
    ) -> anyhow::Result<()> {
        let registry = CodecRegistry::with_defaults();

        let decoder = registry.probe(&path)
            .ok_or_else(|| anyhow::anyhow!("unsupported format: {:?}", path))?;

        let mut instance = decoder.open(&path)?;
        let sample_rate = instance.sample_rate();
        let channels = instance.channels();
        let dur_ms = instance.duration_ms().unwrap_or(0);

        *duration_ref.lock() = dur_ms;

        tracing::info!(
            "Opened: {} | {}Hz {}ch | {}ms (seek_to={}ms)",
            path.display(), sample_rate, channels, dur_ms, seek_to_ms,
        );

        // Attempt seek on the decoder instance
        if seek_to_ms > 0 {
            let target_frame = seek_to_ms * sample_rate as u64 / 1000;
            match instance.seek(target_frame) {
                Ok(()) => {
                    tracing::info!("Decoder seek to frame={} ({}ms) succeeded", target_frame, seek_to_ms);
                }
                Err(e) => {
                    tracing::warn!("Decoder seek failed: {} -?fast-forwarding instead", e);
                    let start_time = std::time::Instant::now();
                    let mut frames_decoded: u64 = 0;
                    while frames_decoded < target_frame {
                        match instance.decode() {
                            Ok(Some(decoded)) => {
                                frames_decoded += decoded.frames as u64;
                                if start_time.elapsed().as_secs() > 5 {
                                    tracing::warn!("Fast-forward timeout at {} frames", frames_decoded);
                                    break;
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                tracing::error!("Fast-forward error: {}", e);
                                break;
                            }
                        }
                    }
                    tracing::info!("Fast-forwarded {} frames in {:?}", frames_decoded, start_time.elapsed());
                }
            }
        }

        let ring = Arc::new(PlaybackRing::new(sample_rate, channels, 10.0, volume.clone()));

        // Reconfigure shared DSP chain with actual sample rate + channels
        {
            let mut chain = dsp_chain.lock();
            chain.set_sample_rate(sample_rate);
            chain.set_channels(channels);
        }

        // If we seeked, bump the ring read_pos to match output position
        if seek_to_ms > 0 {
            let ring_read_start = seek_to_ms * sample_rate as u64 / 1000;
            ring.seek_to_frame(ring_read_start);
        }

        // Trigger fade-in for smooth track start (50ms cosine ramp)
        {
            let mut chain = dsp_chain.lock();
            if let Some(fade) = chain.fade_processor() {
                fade.cancel();
                fade.fade_in(Some(50));
            }
        }

        // Decode thread — writes frames as ring frees up, with crossfade support
        let ring_clone = ring.clone();
        let total_frames = instance.total_frames();
        let dsp_chain_for_thread = dsp_chain.clone();

        // Crossfade config (snapshotted for the decode thread)
        let cf_enabled = crossfade_enabled.load(Ordering::Relaxed);
        let cf_ms = crossfade_ms.load(Ordering::Relaxed) as u64;
        let next_provider_clone = next_provider.clone();
        let crossfade_rx_clone = crossfade_rx.clone();
        // State clone for the decode thread so it can signal Stopped after
        // crossfade completes (see `mixer.is_complete()` below). Without this,
        // the frontend never observes a state transition and playback gets
        // stuck in `Playing` with a drained ring buffer.
        let state_for_thread = state.clone();
        let crossfade_pending_for_thread = crossfade_pending.clone();
        // Error reporting clones — on decode failure the thread records a
        // PlaybackError and transitions to Error state so the frontend can
        // log details and auto-skip to the next track.
        let last_error_for_thread = last_error.clone();
        let track_path_for_thread = track_path.clone();

        tracing::info!(
            "DECODE START: total_frames={total_frames:?} ring_cap={} crossfade={}ms",
            ring.capacity_frames, cf_ms,
        );

        // Capture tokio handle before spawning decode thread
        let rt_handle = tokio::runtime::Handle::current();

        std::thread::spawn(move || {
            let start = Instant::now();
            let mut last_log = start;
            let mut frames_written: u64 = 0;
            let mut mixer = CrossfadeMixer::new(cf_enabled, cf_ms, sample_rate as u64, channels);
            // Reused interleaved scratch buffer for the normal decode path,
            // avoiding a Vec allocation on every decoded chunk.
            let mut interleave_buf: Vec<f32> = Vec::new();

            loop {
                // Wait until ring has room (Notify-based, no sleep polling)
                let chunk_threshold = (sample_rate as usize * channels as usize * PREBUFFER_MS as usize / 1000) / 2;
                while ring_clone.free() < chunk_threshold {
                    rt_handle.block_on(ring_clone.wait_for_free(chunk_threshold));
                }

                // -- Crossfade mixing path --
                if mixer.is_active() {
                    // 1. Decode current frame, run DSP, or emit silence on EOF.
                    let current = match instance.decode() {
                        Ok(Some(mut decoded)) => {
                            if let Err(e) = dsp_chain_for_thread.lock().process(&mut decoded) {
                                tracing::error!("DSP error: {}", e);
                            }
                            decoded.interleaved()
                        }
                        Ok(None) => vec![0.0f32; 2048 * channels as usize],
                        Err(e) => {
                            tracing::error!("CROSSFADE current decode error: {}", e);
                            // Record error and signal Error state so the
                            // frontend can auto-skip. The crossfade mix is
                            // abandoned mid-way; the ring still holds some
                            // samples but the frontend will call playNext()
                            // which flushes everything via stop_inner().
                            let err = PlaybackError::new(
                                ErrorKind::DecoderError,
                                format!("Crossfade decode error: {}", e),
                                Some(track_path_for_thread.clone()),
                            );
                            *last_error_for_thread.lock() = Some(err);
                            state_for_thread.store(
                                state_to_code(PlaybackState::Error),
                                Ordering::Relaxed,
                            );
                            break;
                        }
                    };

                    // 2. Pull next track chunk (None = not ready / disconnected).
                    let next_chunk = {
                        let rx_guard = crossfade_rx_clone.lock();
                        match rx_guard.as_ref() {
                            Some(rx) => match rx.try_recv() {
                                Ok(chunk) => Some(chunk),
                                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    tracing::info!("CROSSFADE: channel disconnected");
                                    None
                                }
                            },
                            None => None,
                        }
                    };

                    // 3. Mix fade-out(current) + fade-in(next) and write to ring.
                    let mixed = mixer.mix(current, next_chunk);
                    frames_written += ring_clone.write(&mixed) as u64;

                    if mixer.is_complete() {
                        tracing::info!(
                            "CROSSFADE: complete after {} samples (target={})",
                            mixer.mixed_samples(),
                            mixer.target_samples(),
                        );
                        // Drain the ring before signaling Stopped.
                        //
                        // The mixer has finished writing the fade-out tail, but
                        // those samples still sit in the ring buffer awaiting
                        // playback by the output callback. If we signal Stopped
                        // now, the frontend's `playNext()` calls `stop_inner()`
                        // which flushes the ring — discarding the un-played
                        // fade-out tail (and any un-mixed current-track samples
                        // still buffered). The listener would hear the track
                        // cut off early by the ring's buffered amount (up to
                        // ~10s) and the crossfade transition would be inaudible.
                        //
                        // Instead we wait for the output callback to drain the
                        // ring (read_pos catches up to write_pos) before
                        // signaling completion. The drain budget is derived
                        // from the currently buffered duration plus a margin
                        // so it adapts to the actual fill level. A state change
                        // (user manual skip/stop, which transitions away from
                        // Playing/Paused) aborts the drain early so the old
                        // decode thread doesn't block a freshly started track.
                        let avail_frames = ring_clone.available();
                        let drain_budget_ms =
                            (avail_frames as u64 * 1000) / sample_rate as u64 + 3000;
                        let drain_deadline = Instant::now()
                            + std::time::Duration::from_millis(drain_budget_ms);
                        loop {
                            if ring_clone.available() == 0 {
                                tracing::info!("CROSSFADE: ring drained, signaling completion");
                                break;
                            }
                            let st = state_for_thread.load(Ordering::Relaxed);
                            if st != state_to_code(PlaybackState::Playing)
                                && st != state_to_code(PlaybackState::Paused)
                            {
                                tracing::info!(
                                    "CROSSFADE drain: state changed (code={}), aborting drain",
                                    st
                                );
                                break;
                            }
                            let now = Instant::now();
                            if now >= drain_deadline {
                                tracing::warn!(
                                    "CROSSFADE drain: timed out after {}ms (avail={} frames), forcing completion",
                                    drain_budget_ms,
                                    ring_clone.available(),
                                );
                                break;
                            }
                            // Wait in short steps so we can re-check
                            // state/timeout between waits (e.g. detect a
                            // manual skip promptly).
                            let remaining = drain_deadline.saturating_duration_since(now);
                            let step = std::cmp::min(
                                remaining,
                                std::time::Duration::from_millis(500),
                            );
                            let _ = rt_handle.block_on(async {
                                tokio::time::timeout(
                                    step,
                                    ring_clone.wait_until_drained(),
                                ).await
                            });
                        }
                        // Order matters: set Stopped BEFORE clearing
                        // crossfade_pending. The event-push thread polls
                        // these two atomics independently; if it observes
                        // crossfadePending=false while state is still Playing,
                        // the frontend's `else if` branch clears
                        // crossfadeActiveRef, and the subsequent Stopped tick
                        // no longer triggers playNext — playback stalls. Setting
                        // Stopped first guarantees the frontend sees the
                        // terminal state while crossfadeActiveRef is still true.
                        state_for_thread.store(state_to_code(PlaybackState::Stopped), Ordering::Relaxed);
                        crossfade_pending_for_thread.store(false, Ordering::Relaxed);
                        break;
                    }
                    continue;
                }

                // -- Normal decode path --
                match instance.decode() {
                    Ok(Some(mut decoded)) => {
                        // Run DSP chain (ReplayGain -> EQ -> Volume -> Fade)
                        if let Err(e) = dsp_chain_for_thread.lock().process(&mut decoded) {
                            tracing::error!("DSP error: {}", e);
                        }
                        decoded.interleaved_into(&mut interleave_buf);
                        let n = ring_clone.write(&interleave_buf);
                        frames_written += n as u64;

                        // -- Crossfade window detection & trigger --
                        let current_pos_ms = (frames_written / sample_rate as u64) * 1000;
                        if mixer.should_trigger(current_pos_ms, dur_ms) {
                            tracing::info!(
                                "CROSSFADE: window reached at {}/{}ms, auto-triggering",
                                current_pos_ms, dur_ms,
                            );
                            let next_path = next_provider_clone
                                .lock()
                                .as_ref()
                                .and_then(|p| p.next_track());
                            match next_path {
                                Some(next_path) => {
                                    Self::spawn_next_track_decoder(
                                        next_path,
                                        sample_rate,
                                        channels,
                                        &crossfade_rx_clone,
                                        &crossfade_pending,
                                        &rt_handle,
                                    );
                                }
                                None => {
                                    tracing::info!("CROSSFADE: no next track available, skipping");
                                }
                            }
                            mixer.mark_triggered();
                        }

                        // Activate the mixing path once the next-track channel is ready.
                        if mixer.should_activate(crossfade_rx_clone.lock().is_some()) {
                            mixer.activate();
                        }

                        let now = Instant::now();
                        if now.duration_since(last_log).as_secs_f32() > 2.0 {
                            let av = ring_clone.available();
                            let cap = ring_clone.capacity_frames;
                            let elapsed = now.duration_since(start).as_secs_f32();
                            tracing::info!(
                                "DECODE: written={} avail={}/{} free={} elapsed={:.1}s",
                                frames_written, av, cap, ring_clone.free(), elapsed,
                            );
                            last_log = now;
                        }
                    }
                    Ok(None) => {
                        tracing::info!("DECODE EOF: frames_written={}", frames_written);
                        break;
                    }
                    Err(e) => {
                        tracing::error!("DECODE ERR: {}", e);
                        // Record the error and transition to Error state so the
                        // frontend can log details and auto-skip to the next
                        // track. Without this the decode thread would exit
                        // silently and leave the state stuck at `Playing`
                        // with a frozen progress bar.
                        let err = PlaybackError::new(
                            ErrorKind::DecoderError,
                            format!("Decode error: {}", e),
                            Some(track_path_for_thread.clone()),
                        );
                        *last_error_for_thread.lock() = Some(err);
                        state_for_thread.store(
                            state_to_code(PlaybackState::Error),
                            Ordering::Relaxed,
                        );
                        break;
                    }
                }
            }
            tracing::info!("Decode thread finished (total={} frames)", frames_written);
        });

        // Prebuffer: async wait until ring has enough frames
        let prebuffer_frames = (sample_rate as u64 * PREBUFFER_MS / 1000) as usize * channels as usize;
        let prebuffer_timeout = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            ring.wait_for_free(prebuffer_frames),
        );
        match tokio::runtime::Handle::current().block_on(prebuffer_timeout) {
            Ok(_) => {}
            Err(_) => {
                // Prebuffer timed out — the decoder couldn't produce enough
                // samples in 3s, likely due to a corrupt or extremely slow
                // file. Record the error and transition to Error state so the
                // frontend can auto-skip.
                let err = PlaybackError::new(
                    ErrorKind::Cancelled,
                    "Prebuffer timeout (decoder too slow)",
                    Some(track_path.clone()),
                );
                *last_error.lock() = Some(err);
                state.store(state_to_code(PlaybackState::Error), Ordering::Relaxed);
                return Err(anyhow::anyhow!("prebuffer timeout"));
            }
        }

        tracing::info!(
            "Prebuffer done: {} frames (target={}, capacity={})",
            ring.available(), prebuffer_frames, ring.capacity_frames,
        );

        let output = AudioOutput::start(ring.clone())?;

        // Store into shared slots -?pipeline can access them
        *ring_slot.lock() = Some(ring);
        *output_slot.lock() = Some(output);

        state.store(state_to_code(PlaybackState::Playing), Ordering::Relaxed);
        Ok(())
    }
}
