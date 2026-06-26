/// Crossfade buffer: holds decoded samples from the next track during crossfade.
///
/// Architecture:
///   Current track's decode thread detects near-EOF (within crossfade window).
///   It signals `needs_next` → external code spawns next track's decoder.
///   Next track's decode thread sends frames via `sender` channel.
///   Current track's decode thread mixes fade-out(current) + fade-in(next).
///
/// The buffer is designed to be cheaply cloneable (Arc internals) and used
/// across two threads (current decode thread reads, next decode thread writes).
use crate::buffer::AudioBuffer;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use std::sync::mpsc;

/// Default crossfade duration: 3 seconds (in samples @ sample_rate, channels)
pub const CROSSFADE_DURATION_MS: u64 = 3000;

/// How many samples of the current track to buffer for the fade-out tail.
/// Computed at runtime from sample_rate × channels × duration_ms.

/// A chunk of interleaved audio from the next track's decoder.
pub struct CrossfadeChunk {
    pub samples: Vec<f32>,
    pub frames: usize,
}

/// Channel for the next track's decoder to send decoded samples to the mixer.
pub type CrossfadeSender = mpsc::SyncSender<CrossfadeChunk>;
pub type CrossfadeReceiver = mpsc::Receiver<CrossfadeChunk>;

/// Create a crossfade channel pair (bounded, back-pressured).
pub fn crossfade_channel() -> (CrossfadeSender, CrossfadeReceiver) {
    mpsc::sync_channel::<CrossfadeChunk>(30)
}

/// Streaming format converter for the crossfade "next track" path.
///
/// The [`CrossfadeMixer`] blends two tracks additively and assumes they share
/// the *current* track's sample rate and channel count. When the next track is
/// a different format (e.g. a 48 kHz FLAC following a 44.1 kHz MP3), this
/// converter resamples it (band-limited sinc via `rubato`) and up/down-mixes
/// the channels to stereo before the samples reach the mixer.
///
/// It is **streaming**: decoded chunks are variable-length, but `rubato`'s
/// `SincFixedIn` wants fixed-size input chunks. We accumulate input until the
/// resampler's required chunk size is reached, process, and buffer the
/// (variable-length) output. Surplus output carries to the next call so the
/// caller never has to care about chunk boundaries.
///
/// Channel policy (to the mixer's expected stereo):
///   * mono → stereo: duplicate the channel
///   * stereo → stereo: passthrough
///   * other (5.1, 7.1, etc.): only the first two channels are kept. This is a
///     crossfade tail, not a quality-critical decode path, so the slight
///     channel drop is acceptable; full surround downmix is out of scope.
pub struct CrossfadeResampler {
    src_channels: u16,
    dst_channels: u16,
    /// rubato resampler, planar, `dst_channels` lanes. `None` when rates match.
    resampler: Option<SincFixedIn<f32>>,
    /// Planar input accumulator, one Vec per dst channel.
    in_planar: Vec<Vec<f32>>,
    /// Interleaved resampled output carry-over (in dst format).
    out_buf: std::collections::VecDeque<f32>,
}

impl CrossfadeResampler {
    /// Build a converter. Returns `Ok(None)` when no conversion is needed
    /// (rates AND channels already match), so the caller can take the cheap
    /// passthrough path.
    pub fn new(src_rate: u32, src_channels: u16, dst_rate: u32, dst_channels: u16) -> anyhow::Result<Option<Self>> {
        if src_rate == dst_rate && src_channels == dst_channels {
            return Ok(None);
        }
        // rubato output_frames = input_frames * (dst/src). ratio is out/in.
        let ratio = dst_rate as f64 / src_rate as f64;
        let params = SincInterpolationParameters {
            sinc_len: 256,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 256,
            window: WindowFunction::BlackmanHarris2,
        };
        let resampler = SincFixedIn::<f32>::new(ratio, 2.0, params, 1024, dst_channels as usize)
            .map_err(|e| anyhow::anyhow!("rubato init failed: {e}"))?;
        Ok(Some(Self {
            src_channels,
            dst_channels,
            resampler: Some(resampler),
            in_planar: (0..dst_channels).map(|_| Vec::with_capacity(1024)).collect(),
            out_buf: std::collections::VecDeque::with_capacity(4096),
        }))
    }

    /// Push a decoded (interleaved, src format) chunk and return any fully
    /// converted output now available, as interleaved dst-format samples.
    pub fn push(&mut self, interleaved: &[f32]) -> Vec<f32> {
        let resampler = match self.resampler.as_mut() {
            Some(r) => r,
            None => {
                // Rates match but channels differ — only channel-convert.
                return channel_convert(interleaved, self.src_channels, self.dst_channels);
            }
        };

        // Deinterleave + channel-convert into the planar accumulator.
        let frames = interleaved.len() / self.src_channels as usize;
        for f in 0..frames {
            let base = f * self.src_channels as usize;
            for ch in 0..self.dst_channels as usize {
                let s = if (ch as u16) < self.src_channels {
                    interleaved[base + ch as usize]
                } else if self.src_channels == 1 {
                    // mono → stereo: duplicate channel 0
                    interleaved[base]
                } else {
                    // more dst channels than src: silence the extras
                    0.0
                };
                self.in_planar[ch].push(s);
            }
        }

        // Process as many fixed-size input chunks as we have.
        let mut produced: Vec<f32> = Vec::new();
        loop {
            let need = resampler.input_frames_next();
            let have = self.in_planar[0].len();
            if have < need {
                break;
            }
            // rubato takes `&[Vec<f32>]` (planar). We hand it the first `need`
            // frames of each lane and drop them afterwards.
            let chunk: Vec<Vec<f32>> = self.in_planar.iter()
                .map(|lane| lane[..need].to_vec())
                .collect();
            for lane in &mut self.in_planar {
                lane.drain(..need);
            }
            match resampler.process(&chunk, None) {
                Ok(out) => {
                    let n = out[0].len().min(out.get(1).map(|c| c.len()).unwrap_or(0));
                    for i in 0..n {
                        for ch in 0..self.dst_channels as usize {
                            produced.push(out[ch][i]);
                        }
                    }
                }
                Err(e) => {
                    log::warn!("crossfade resampler error: {e}");
                    break;
                }
            }
        }

        // Append produced samples to the carry-over, then drain everything
        // we have (the mixer is happy to receive whatever's ready).
        self.out_buf.extend(produced);
        let drained: Vec<f32> = self.out_buf.drain(..).collect();
        drained
    }

    /// Flush any buffered output. Called on EOF to release residual samples
    /// still held inside rubato's filter delay lines.
    pub fn flush(&mut self) -> Vec<f32> {
        if let Some(resampler) = self.resampler.as_mut() {
            // rubato's `process` with an empty/zero tail flushes the delay
            // lines. We feed one last zero-padded chunk at the required size.
            let need = resampler.input_frames_next();
            let chunk: Vec<Vec<f32>> = (0..self.dst_channels).map(|_| vec![0.0; need]).collect();
            if let Ok(out) = resampler.process(&chunk, None) {
                let n = out[0].len().min(out.get(1).map(|c| c.len()).unwrap_or(0));
                for i in 0..n {
                    for ch in 0..self.dst_channels as usize {
                        self.out_buf.push_back(out[ch][i]);
                    }
                }
            }
        }
        self.out_buf.drain(..).collect()
    }

    /// True if this converter is a no-op (kept for callers that want to branch).
    pub fn is_identity(&self) -> bool {
        self.resampler.is_none() && self.src_channels == self.dst_channels
    }
}

/// Interleaved channel conversion (mono↔stereo, surround→stereo-take-first-2).
/// Used standalone when rates already match, and as part of the planar feed
/// when resampling is also active.
fn channel_convert(interleaved: &[f32], src_ch: u16, dst_ch: u16) -> Vec<f32> {
    if src_ch == dst_ch {
        return interleaved.to_vec();
    }
    let frames = interleaved.len() / src_ch as usize;
    let mut out = Vec::with_capacity(frames * dst_ch as usize);
    for f in 0..frames {
        let base = f * src_ch as usize;
        for ch in 0..dst_ch as usize {
            let s = if (ch as u16) < src_ch {
                interleaved[base + ch]
            } else if src_ch == 1 {
                interleaved[base]
            } else {
                0.0
            };
            out.push(s);
        }
    }
    out
}

/// Cosine ease-out: 1→0 (for current track fade-out)
#[inline]
pub fn cosine_ramp_down(t: f32) -> f32 {
    (t.clamp(0.0, 1.0) * std::f32::consts::PI * 0.5).cos()
}

/// Cosine ease-in: 0→1 (for next track fade-in)
#[inline]
pub fn cosine_ramp_up(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (t * std::f32::consts::PI * 0.5).cos()
}

/// C1-continuous soft limiter.
///
/// Identity (transparent) for `|x| <= 1.0`; above unity the overshoot is
/// compressed through a `tanh` curve so the signal approaches ~2.0 smoothly
/// instead of brick-wall clipping at the hard `clamp(-1, 1)` kink, which
/// introduces audible 削波 (clipping) distortion on overshoots.
///
/// Note: with the cosine ramps above, `fade_out + fade_in == 1.0` exactly, so
/// two normalized (|sample| <= 1) tracks summed during crossfade stay within
/// `[-1, 1]` and this limiter only engages on rare inter-sample/EQ overshoots.
#[inline]
pub fn soft_limit(x: f32) -> f32 {
    let ax = x.abs();
    if ax <= 1.0 {
        x
    } else {
        let sign = x.signum();
        let over = ax - 1.0;
        sign * (1.0 + over.tanh())
    }
}

/// Mix a planar AudioBuffer with interleaved samples from the next track
/// during the crossfade overlap period.
///
/// - `current`: planar buffer (current track), mutated in-place with fade-out applied
/// - `next_interleaved`: interleaved f32 samples from next track (to be faded in)
/// - `progress`: 0.0 = start of crossfade, 1.0 = end
/// - `channels`: number of audio channels
///
/// After this call, `current` contains the mixed result.
pub fn mix_crossfade(
    current: &mut AudioBuffer,
    next_interleaved: &[f32],
    progress: f32,
    channels: usize,
) {
    let frames = current.frames;
    let fade_out = cosine_ramp_down(progress);
    let fade_in = cosine_ramp_up(progress);

    for frame in 0..frames {
        for ch in 0..channels {
            // Current track: fade out
            current.data[ch][frame] *= fade_out;

            // Next track: fade in, mix additively
            let idx = frame * channels + ch;
            if idx < next_interleaved.len() {
                current.data[ch][frame] += next_interleaved[idx] * fade_in;
            }
        }
    }
}

/// Check whether we're within the crossfade window of the end of track.
///
/// Returns `true` if `remaining_ms <= crossfade_duration_ms`.
pub fn in_crossfade_window(
    position_ms: u64,
    duration_ms: u64,
    crossfade_ms: u64,
) -> bool {
    if duration_ms == 0 || crossfade_ms == 0 {
        return false;
    }
    let remaining = duration_ms.saturating_sub(position_ms);
    remaining <= crossfade_ms
}

/// Self-contained crossfade state machine.
///
/// Encapsulates everything the decode thread needs to perform an end-of-track
/// crossfade so that `player.rs` stays a thin orchestration layer:
///
/// 1. `should_trigger`        — are we inside the crossfade window?
/// 2. `mark_triggered`        — record that the next-track decoder has been launched
/// 3. `should_activate` / `activate` — the next-track channel is ready, switch to the mixing path
/// 4. `mix`                   — fade-out current + fade-in next, return interleaved result
/// 5. `is_complete`           — crossfade duration satisfied, decode thread may stop
///
/// All progress/fade/sample-count math lives here and is unit-testable in
/// isolation. Note: sample counts (frames × channels) are used consistently
/// for both `target` and `mixed`, unlike the previous inline code which mixed
/// frame and sample units and therefore never reached completion for stereo.
pub struct CrossfadeMixer {
    enabled: bool,
    duration_ms: u64,
    target_samples: u64,
    mixed_samples: u64,
    /// `true` once the next-track decoder has been spawned (prevents retrigger).
    triggered: bool,
    /// `true` once the mixing path is active.
    active: bool,
}

impl CrossfadeMixer {
    /// Build a mixer from snapshotted config.
    ///
    /// `sample_rate` / `channels` describe the *current* track; the next track
    /// is expected to share them (the crossfade path resamples/converts nothing).
    pub fn new(enabled: bool, duration_ms: u64, sample_rate: u64, channels: u16) -> Self {
        let target_samples = if enabled && duration_ms > 0 {
            (sample_rate * duration_ms / 1000) * channels as u64
        } else {
            0
        };
        Self {
            enabled,
            duration_ms,
            target_samples,
            mixed_samples: 0,
            triggered: false,
            active: false,
        }
    }

    /// Whether crossfade is enabled at all.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Configured crossfade duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Total samples (frames × channels) to mix before declaring completion.
    pub fn target_samples(&self) -> u64 {
        self.target_samples
    }

    /// Samples mixed so far.
    pub fn mixed_samples(&self) -> u64 {
        self.mixed_samples
    }

    /// Normalized crossfade progress in `[0.0, 1.0]`.
    pub fn progress(&self) -> f32 {
        if self.target_samples == 0 {
            return 1.0;
        }
        ((self.mixed_samples as f64 / self.target_samples as f64) as f32).clamp(0.0, 1.0)
    }

    /// Should the next-track decoder be spawned now?
    ///
    /// True only once, when the play position enters the crossfade window.
    pub fn should_trigger(&self, position_ms: u64, track_duration_ms: u64) -> bool {
        self.enabled
            && !self.triggered
            && in_crossfade_window(position_ms, track_duration_ms, self.duration_ms)
    }

    /// Record that the next-track decoder launch has been attempted (success
    /// or failure); prevents repeated triggering.
    pub fn mark_triggered(&mut self) {
        self.triggered = true;
    }

    /// Should the decode thread switch from the normal path to the mixing path?
    ///
    /// `next_channel_ready` is `true` once the receiver slot is populated.
    pub fn should_activate(&self, next_channel_ready: bool) -> bool {
        self.triggered && !self.active && next_channel_ready
    }

    /// Activate the mixing path.
    pub fn activate(&mut self) {
        if !self.active {
            tracing::info!("CROSSFADE: activating crossfade mix");
            self.active = true;
        }
    }

    /// Is the mixing path currently active?
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Has the crossfade mixed enough samples to stop the decode thread?
    pub fn is_complete(&self) -> bool {
        self.active && self.target_samples > 0 && self.mixed_samples >= self.target_samples
    }

    /// Mix one chunk: fade-out `current` (already DSP-processed, interleaved)
    /// with fade-in `next` (raw interleaved from the next-track decoder).
    ///
    /// `next` is `None` when the channel is empty/disconnected/eof-sentinel;
    /// in that case silence is faded in so the current track still fades out.
    ///
    /// Returns the interleaved mixed samples, length-aligned to `current`.
    pub fn mix(
        &mut self,
        current: Vec<f32>,
        next: Option<CrossfadeChunk>,
    ) -> Vec<f32> {
        let progress = self.progress();
        let fade_out = cosine_ramp_down(progress);
        let fade_in = cosine_ramp_up(progress);
        let len = current.len();

        // Fade-out the current track in place.
        let mut mixed = current;
        for s in &mut mixed {
            *s *= fade_out;
        }

        // Fade-in the next track, zero-padded/truncated to `len`.
        let mut next_plane = vec![0.0f32; len];
        if let Some(chunk) = next {
            if !chunk.samples.is_empty() {
                self.mixed_samples += chunk.samples.len() as u64;
                let copy = chunk.samples.len().min(len);
                for i in 0..copy {
                    next_plane[i] = chunk.samples[i] * fade_in;
                }
            }
        }

        // Additive mix with soft limiting (avoids hard-clip distortion).
        for i in 0..len {
            mixed[i] = soft_limit(mixed[i] + next_plane[i]);
        }
        mixed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_chunk(samples: Vec<f32>) -> CrossfadeChunk {
        let frames = samples.len();
        CrossfadeChunk { samples, frames }
    }

    #[test]
    fn disabled_mixer_never_triggers() {
        let m = CrossfadeMixer::new(false, 3000, 44100, 2);
        assert!(!m.should_trigger(0, 10000));
        assert!(!m.is_active());
        assert!(!m.is_complete());
        assert_eq!(m.target_samples(), 0);
    }

    #[test]
    fn triggers_only_once_inside_window() {
        let mut m = CrossfadeMixer::new(true, 3000, 44100, 2);
        // 10s track, crossfade 3s → window opens at 7s
        assert!(!m.should_trigger(6000, 10000));
        assert!(m.should_trigger(7000, 10000));
        m.mark_triggered();
        assert!(!m.should_trigger(9000, 10000)); // already triggered
    }

    #[test]
    fn target_samples_is_frames_times_channels() {
        let m = CrossfadeMixer::new(true, 1000, 44100, 2);
        // 1s × 44100 × 2ch = 88200 samples
        assert_eq!(m.target_samples(), 88200);
    }

    #[test]
    fn mix_fades_and_completes() {
        let mut m = CrossfadeMixer::new(true, 1000, 44100, 1);
        // target = 44100 samples (mono). Activate the mixing path first,
        // mirroring what the decode thread does before calling mix().
        m.activate();
        let total = m.target_samples();
        let mut sent = 0u64;
        while !m.is_complete() {
            let cur = vec![0.0f32; 1024];
            let next = vec![1.0f32; 1024];
            let mixed = m.mix(cur, Some(make_chunk(next)));
            // At progress 0, fade_out=1 fade_in≈0 → mixed ≈ current(0) + 0 ≈ 0
            // At progress 1, fade_out=0 fade_in=1 → mixed ≈ 0 + 1 = 1
            for s in &mixed {
                assert!(*s >= 0.0 && *s <= 1.0);
            }
            sent += 1024;
            if sent > total + 4096 {
                panic!("crossfade did not complete");
            }
        }
        assert!(m.mixed_samples() >= total);
    }

    #[test]
    fn mix_handles_missing_next_as_silence() {
        let mut m = CrossfadeMixer::new(true, 1000, 44100, 2);
        let cur = vec![0.5f32; 8];
        // No next chunk available → next is silence, current still fades out.
        let mixed = m.mix(cur, None);
        let p = m.progress();
        let expected = 0.5 * cosine_ramp_down(p);
        assert!((mixed[0] - expected).abs() < 1e-5);
        // mixed_samples must NOT advance when next is absent.
        assert_eq!(m.mixed_samples(), 0);
    }

    #[test]
    fn mix_pads_short_next_to_current_length() {
        let mut m = CrossfadeMixer::new(true, 1000, 44100, 1);
        let cur = vec![0.0f32; 8];
        let next = vec![1.0f32; 4]; // shorter than current
        let mixed = m.mix(cur, Some(make_chunk(next)));
        assert_eq!(mixed.len(), 8);
        // Only 4 samples contributed to mixed_samples.
        assert_eq!(m.mixed_samples(), 4);
    }
}
