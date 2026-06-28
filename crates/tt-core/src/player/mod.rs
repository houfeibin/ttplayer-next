use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::task;

use crate::dsp::crossfade::{CrossfadeReceiver, CROSSFADE_DURATION_MS};
use crate::dsp::spectrum::SpectrumFrame;
use crate::dsp::DspChain;
use crate::output::{AtomicVolume, PlaybackRing};
use tt_common::{PlaybackError, PlaybackState, SongMetadata};

mod crossfade_bridge;
mod decode_thread;
mod transport;

/// Trait for providing the next track path during crossfade.
/// Implemented by the application layer (e.g. PlaylistManager wrapper).
pub trait NextTrackProvider: Send + Sync {
    fn next_track(&self) -> Option<PathBuf>;
}

/// Prebuffer duration in milliseconds before output starts.
pub(crate) const PREBUFFER_MS: u64 = 500;

pub(crate) fn state_to_code(s: PlaybackState) -> u8 {
    match s {
        PlaybackState::Idle => 0,
        PlaybackState::Loading => 1,
        PlaybackState::Playing => 2,
        PlaybackState::Paused => 3,
        PlaybackState::Stopped => 4,
        PlaybackState::Error => 5,
    }
}
pub(crate) fn code_to_state(c: u8) -> PlaybackState {
    match c {
        0 => PlaybackState::Idle,
        1 => PlaybackState::Loading,
        2 => PlaybackState::Playing,
        3 => PlaybackState::Paused,
        4 => PlaybackState::Stopped,
        5 => PlaybackState::Error,
        _ => PlaybackState::Idle,
    }
}

pub(crate) type SharedOutput = Arc<Mutex<Option<crate::output::AudioOutput>>>;
pub(crate) type SharedRing = Arc<Mutex<Option<Arc<PlaybackRing>>>>;
pub(crate) type SharedCrossfadeRx = Arc<Mutex<Option<CrossfadeReceiver>>>;

/// The main audio pipeline: decode -?ring buffer -?output
pub struct AudioPipeline {
    pub(crate) state: Arc<AtomicU8>,
    pub(crate) current_file: Arc<Mutex<Option<PathBuf>>>,
    pub(crate) volume: Arc<AtomicVolume>,
    pub(crate) file_duration_ms: Arc<Mutex<u64>>,
    pub(crate) metadata: Arc<Mutex<SongMetadata>>,
    /// Bumped whenever `metadata` is refreshed, so the event-push thread can
    /// detect late-arriving (asynchronously-read) metadata and re-emit it to
    /// the frontend even though the current file path hasn't changed.
    pub(crate) metadata_rev: Arc<AtomicU64>,
    /// Shared DSP chain -?decode thread reads, commands write EQ settings
    pub(crate) dsp_chain: Arc<Mutex<DspChain>>,

    /// Shared with bg thread -?bg writes, pipeline reads
    pub(crate) ring_slot: SharedRing,
    pub(crate) output_slot: SharedOutput,

    // ――-?Crossfade state ――-?
    pub(crate) crossfade_enabled: Arc<AtomicBool>,
    pub(crate) crossfade_ms: Arc<AtomicU64>,
    pub(crate) crossfade_pending: Arc<AtomicBool>,
    pub(crate) crossfade_rx: SharedCrossfadeRx,

    pub(crate) next_provider: Arc<Mutex<Option<Arc<dyn NextTrackProvider>>>>,

    /// Last playback error (None when no error has occurred, or the error was
    /// cleared by starting a new track). Emitted to the frontend so the UI can
    /// log details and auto-skip to the next track.
    pub(crate) last_error: Arc<Mutex<Option<PlaybackError>>>,
}

impl AudioPipeline {
    pub fn new() -> Self {
        Self {
            state: Arc::new(AtomicU8::new(0)),
            current_file: Arc::new(Mutex::new(None)),
            volume: Arc::new(AtomicVolume::new(1.0)),
            file_duration_ms: Arc::new(Mutex::new(0)),
            metadata: Arc::new(Mutex::new(SongMetadata::default())),
            metadata_rev: Arc::new(AtomicU64::new(0)),
            dsp_chain: Arc::new(Mutex::new(DspChain::new(44100))),
            ring_slot: Arc::new(Mutex::new(None)),
            output_slot: Arc::new(Mutex::new(None)),
            crossfade_enabled: Arc::new(AtomicBool::new(true)),
            crossfade_ms: Arc::new(AtomicU64::new(CROSSFADE_DURATION_MS)),
            crossfade_pending: Arc::new(AtomicBool::new(false)),
            crossfade_rx: Arc::new(Mutex::new(None)),
            next_provider: Arc::new(Mutex::new(None)),
            last_error: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_next_track_provider(&self, provider: Arc<dyn NextTrackProvider>) {
        *self.next_provider.lock() = Some(provider);
    }

    pub fn set_volume(&self, volume: f32) { self.volume.set(volume); }
    pub fn volume(&self) -> f32 { self.volume.get() }

    pub fn state(&self) -> PlaybackState {
        code_to_state(self.state.load(Ordering::Relaxed))
    }

    /// Last recorded playback error (if any). Remains set until a new track is
    /// opened via [`transport::open_and_play_at`], which clears it.
    pub fn last_error(&self) -> Option<PlaybackError> {
        self.last_error.lock().clone()
    }

    /// Record a playback error and transition the state machine to `Error`.
    /// The error details (kind, message, track path, timestamp) are stored and
    /// emitted to the frontend via the event-push thread so the UI can log
    /// them and trigger an auto-skip.
    pub fn set_error(&self, error: PlaybackError) {
        tracing::error!(
            "PLAYBACK ERROR: kind={:?} track={:?} msg={}",
            error.kind, error.track_path, error.message,
        );
        *self.last_error.lock() = Some(error);
        self.state.store(state_to_code(PlaybackState::Error), Ordering::Relaxed);
    }

    /// Clear any stored playback error. Called when a new track starts so the
    /// frontend doesn't see a stale error from the previous track.
    pub fn clear_error(&self) {
        *self.last_error.lock() = None;
    }
    pub fn position_ms(&self) -> u64 {
        self.ring_slot.lock().as_ref().map(|r| r.position_ms()).unwrap_or(0)
    }
    pub fn duration_ms(&self) -> u64 {
        *self.file_duration_ms.lock()
    }
    pub fn sample_rate(&self) -> u32 {
        self.ring_slot.lock().as_ref().map(|r| r.sample_rate).unwrap_or(44100)
    }
    pub fn channels(&self) -> u16 {
        self.ring_slot.lock().as_ref().map(|r| r.channels).unwrap_or(2)
    }
    pub fn current_file(&self) -> Option<PathBuf> {
        self.current_file.lock().clone()
    }

    pub fn metadata(&self) -> SongMetadata {
        self.metadata.lock().clone()
    }

    /// Monotonic revision counter for [`metadata`], bumped whenever tags are
    /// refreshed (including the asynchronous read in [`transport::open_and_play_at`]).
    /// The event-push thread compares this against its last-seen value to
    /// re-emit metadata that arrived after the file-change tick.
    pub fn metadata_rev(&self) -> u64 {
        self.metadata_rev.load(Ordering::Relaxed)
    }

    /// Re-read tags from `path` and refresh the cached metadata, but only if
    /// `path` matches the currently loaded file. Used after external tag edits
    /// (e.g. via the TagEditor UI) so the player UI updates without a restart
    /// or manual refresh.
    ///
    /// Like [`transport::open_and_play_at`], tag parsing runs on a blocking
    /// task to avoid stalling the command thread (base64 cover-art encoding
    /// can take ~50ms). The event-push thread detects the bumped
    /// `metadata_rev` and re-emits the fresh metadata to the frontend on its
    /// next tick (~50ms), so the UI reflects the changes with no perceptible
    /// delay and zero interference with ongoing playback.
    pub fn refresh_metadata_if_current(&self, path: &Path) {
        let is_current = self
            .current_file
            .lock()
            .as_ref()
            .map(|p| p == path)
            .unwrap_or(false);
        if !is_current {
            return;
        }

        let tags_path = path.to_path_buf();
        let metadata_slot = self.metadata.clone();
        let metadata_rev = self.metadata_rev.clone();
        let dsp_chain_slot = self.dsp_chain.clone();
        task::spawn_blocking(move || {
            if let Ok(tags) = tt_tags::read(&tags_path) {
                if let Some(ref rg) = tags.replay_gain {
                    if let Some(rg_proc) = dsp_chain_slot.lock().replay_gain() {
                        rg_proc.set_from_rg(rg);
                    }
                }
                *metadata_slot.lock() = tags;
                metadata_rev.fetch_add(1, Ordering::Relaxed);
            }
        });
    }

    /// Latest spectrum frame from the output callback (may be None if not yet analyzed)
    pub fn spectrum(&self) -> Option<Arc<SpectrumFrame>> {
        self.output_slot.lock().as_ref()?.spectrum.lock().clone()
    }

    // ――-?Crossfade bridge ――-?
    /// Whether the decode thread has signaled that crossfade should start
    pub fn crossfade_is_pending(&self) -> bool {
        self.crossfade_pending.load(Ordering::Relaxed)
    }

    /// Clear the crossfade pending flag
    pub fn crossfade_clear_pending(&self) {
        self.crossfade_pending.store(false, Ordering::Relaxed);
    }

    /// Get crossfade duration in ms
    pub fn crossfade_duration_ms(&self) -> u64 {
        self.crossfade_ms.load(Ordering::Relaxed)
    }

    /// Set crossfade duration in ms (0 to disable)
    pub fn set_crossfade_duration_ms(&self, ms: u64) {
        self.crossfade_ms.store(ms, Ordering::Relaxed);
        self.crossfade_enabled.store(ms > 0, Ordering::Relaxed);
    }

    // ――-?Fade bridge ――-?
    /// Trigger fade-out on the DSP chain (e.g. before stop/seek).
    pub fn fade_out(&self, duration_ms: u32) -> bool {
        let mut chain = self.dsp_chain.lock();
        if let Some(fade) = chain.fade_processor() {
            fade.fade_out(Some(duration_ms));
            return true;
        }
        false
    }

    /// Whether a fade is currently in progress
    pub fn fading(&self) -> bool {
        self.dsp_chain.lock()
            .fade_processor()
            .map(|f| f.fading())
            .unwrap_or(false)
    }

    // ――-?Surround bridge ――-?
    pub fn set_surround_width(&self, width: u8) {
        if let Some(s) = self.dsp_chain.lock().surround_processor() {
            s.set_width(width);
        }
    }

    pub fn surround_width(&self) -> u8 {
        self.dsp_chain.lock()
            .surround_processor()
            .map(|s| s.width())
            .unwrap_or(0)
    }

    // ――-?Equalizer bridge ――-?
    pub fn eq_bands(&self) -> [f64; 10] {
        let mut chain = self.dsp_chain.lock();
        let mut bands = [0.0f64; 10];
        if let Some(eq) = chain.equalizer() {
            for i in 0..10 {
                bands[i] = eq.band_gain(i);
            }
        }
        bands
    }

    pub fn set_eq_band(&self, band: usize, gain_db: f64) {
        if let Some(eq) = self.dsp_chain.lock().equalizer() {
            eq.set_band_gain(band, gain_db);
        }
    }

    pub fn eq_preamp(&self) -> f64 {
        self.dsp_chain.lock().equalizer().map(|eq| eq.preamp()).unwrap_or(0.0)
    }

    pub fn set_eq_preamp(&self, gain_db: f64) {
        if let Some(eq) = self.dsp_chain.lock().equalizer() {
            eq.set_preamp(gain_db);
        }
    }

    pub fn eq_reset(&self) {
        if let Some(eq) = self.dsp_chain.lock().equalizer() {
            eq.reset();
        }
    }
}
