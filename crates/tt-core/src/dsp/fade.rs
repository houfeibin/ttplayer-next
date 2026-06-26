/// Fade in/out processor with optional crossfade support.
///
/// Operates on interleaved f32 samples in the AudioBuffer.
/// Uses cosine ramp (1-cos/2) for smooth, natural-sounding transitions.
///
/// State machine:
///   Off → FadeIn(frames) → Off
///   Playing → FadeOut(frames) → Off
///   Crossfade: overlap two AudioBuffers with complementary ramps
use crate::buffer::AudioBuffer;
use super::AudioProcessor;

// ── timing constants (samples @ 44100 Hz) ───────────────────

/// Default fade-in duration: 50ms = ~2205 samples at 44.1k
pub const FADE_IN_DURATION_SAMPLES: usize = 2205;

/// Default fade-out duration: 200ms = ~8820 samples at 44.1k (for stop/seek)
pub const FADE_OUT_DURATION_SAMPLES: usize = 8820;

/// Default crossfade duration: 2s = ~88200 samples at 44.1k (for track transitions)
pub const CROSSFADE_DURATION_SAMPLES: usize = 88200;

// ── fade state ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FadeKind {
    Off,
    FadeIn,
    FadeOut,
}

#[derive(Debug, Clone, Copy)]
pub struct FadeState {
    pub kind: FadeKind,
    /// Total frames in this ramp
    pub total: usize,
    /// Frames already processed
    pub elapsed: usize,
}

impl FadeState {
    pub fn is_active(&self) -> bool { self.kind != FadeKind::Off }
    pub fn progress(&self) -> f32 {
        if self.total == 0 { 1.0 }
        else { (self.elapsed as f32 / self.total as f32).clamp(0.0, 1.0) }
    }
    pub fn ramp_up(&self) -> f32 { cosine_ramp_up(self.progress()) }
    pub fn ramp_down(&self) -> f32 { cosine_ramp_down(self.progress()) }
}

// ── cosine ramp helpers ─────────────────────────────────────

/// Cosine ease-in: 0→1, slow start, fast end
#[inline]
fn cosine_ramp_up(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (t * std::f32::consts::PI * 0.5).cos()
}

/// Cosine ease-out: 1→0, fast start, slow end
#[inline]
fn cosine_ramp_down(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    (t * std::f32::consts::PI * 0.5).cos()
}

// ── FadeProcessor ───────────────────────────────────────────

/// Handles fade-in, fade-out, and crossfade transitions.
/// Placed AFTER Volume in DspChain so RG+EQ+Volume are already applied.
pub struct FadeProcessor {
    /// Current fade (None = Off)
    state: Option<FadeState>,
    sample_rate: u32,
    channels: u16,
    enabled: bool,
}

impl FadeProcessor {
    pub fn new() -> Self {
        Self {
            state: None,
            sample_rate: 44100,
            channels: 2,
            enabled: true,
        }
    }

    /// Trigger fade-in (e.g. on track start)
    pub fn fade_in(&mut self, duration_ms: Option<u32>) {
        let ms = duration_ms.unwrap_or(50);
        let total = (self.sample_rate as u64 * ms as u64 / 1000) as usize * self.channels as usize;
        self.state = Some(FadeState { kind: FadeKind::FadeIn, total, elapsed: 0 });
    }

    /// Trigger fade-out (e.g. on stop/seek)
    pub fn fade_out(&mut self, duration_ms: Option<u32>) {
        let ms = duration_ms.unwrap_or(200);
        let total = (self.sample_rate as u64 * ms as u64 / 1000) as usize * self.channels as usize;
        self.state = Some(FadeState { kind: FadeKind::FadeOut, total, elapsed: 0 });
    }

    /// Whether a fade is currently in progress
    pub fn fading(&self) -> bool {
        self.state.as_ref().map(|s| s.is_active()).unwrap_or(false)
    }

    /// Cancel any active fade
    pub fn cancel(&mut self) {
        self.state = None;
    }
}

impl AudioProcessor for FadeProcessor {
    fn name(&self) -> &'static str { "fade" }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, buffer: &mut AudioBuffer) -> anyhow::Result<()> {
        let Some(ref mut state) = self.state else { return Ok(()); };
        if !state.is_active() { return Ok(()); }

        let total = state.total;
        let n_frames = buffer.frames;
        let n_channels = buffer.channels as usize;

        match state.kind {
            FadeKind::FadeIn => {
                for frame in 0..n_frames {
                    let sample_idx = state.elapsed + frame * n_channels;
                    if sample_idx >= total { break; }
                    let t = sample_idx as f32 / total as f32;
                    let gain = cosine_ramp_up(t);
                    for ch in 0..n_channels {
                        buffer.data[ch][frame] *= gain;
                    }
                }
            }
            FadeKind::FadeOut => {
                for frame in 0..n_frames {
                    let sample_idx = state.elapsed + frame * n_channels;
                    if sample_idx >= total {
                        // Zero samples past fade end
                        for ch in 0..n_channels {
                            buffer.data[ch][frame] = 0.0;
                        }
                        continue;
                    }
                    let t = sample_idx as f32 / total as f32;
                    let gain = cosine_ramp_down(t);
                    for ch in 0..n_channels {
                        buffer.data[ch][frame] *= gain;
                    }
                }
            }
            FadeKind::Off => {}
        }

        state.elapsed += n_frames * n_channels;
        if state.elapsed >= state.total {
            self.state = None;
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.state = None;
    }

    fn set_sample_rate(&mut self, rate: u32) { self.sample_rate = rate; }
    fn set_channels(&mut self, ch: u16) { self.channels = ch; }

    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
}

// ── crossfade utility (static, used externally) ─────────────

/// Mix two equal-length interleaved buffers with complementary cosine ramps.
/// `t` is the crossfade progress 0..1: at t=0.5 both signals at half power.
/// `dst` receives the mixed result; `src_a` is faded out, `src_b` is faded in.
pub fn crossfade_mix(dst: &mut [f32], src_a: &[f32], src_b: &[f32], t: f32) {
    let t = t.clamp(0.0, 1.0);
    let ramp_a = cosine_ramp_down(t);
    let ramp_b = cosine_ramp_up(t);
    for i in 0..dst.len() {
        let a = src_a.get(i).copied().unwrap_or(0.0);
        let b = src_b.get(i).copied().unwrap_or(0.0);
        dst[i] = a * ramp_a + b * ramp_b;
    }
}
