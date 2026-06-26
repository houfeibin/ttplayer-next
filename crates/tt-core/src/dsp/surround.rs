/// Surround sound effect: stereo image widening via Mid/Side (M/S) processing.
///
/// TTPlayer's original "surround" knob (0-10) is essentially a stereo width enhancer.
/// This processor works on stereo (2ch) signals only; mono passthrough.
///
/// Algorithm:
///   1. Convert L/R → Mid/Side:  M = (L+R)/2,  S = (L-R)/2
///   2. Scale side channel:  S' = S * depth (depth = 1.0..3.0)
///   3. Convert back:  L = M + S',  R = M - S'
///   4. Soft-clip output to prevent overshoot
///
/// Depth 1.0 = original stereo, 2.0 = double width (≈TTPlayer "8"), 3.0 = max.
/// For mono signals (1ch), no processing is applied.
use crate::buffer::AudioBuffer;
use super::AudioProcessor;

/// Default max width (maps from TTPlayer 0-10 → 1.0-3.0 depth)
const MAX_DEPTH: f32 = 3.0;
const MIN_DEPTH: f32 = 1.0;

pub struct SurroundProcessor {
    /// Width depth (1.0 = no change, 3.0 = max stereo widening)
    depth: f32,
    enabled: bool,
}

impl SurroundProcessor {
    pub fn new() -> Self {
        Self {
            depth: 1.0,
            enabled: false,
        }
    }

    /// Set width from TTPlayer-style 0-10 scale.
    /// 0 = no surround, 8 = default, 10 = max.
    pub fn set_width(&mut self, width: u8) {
        let w = width.min(10) as f32 / 10.0;
        self.depth = MIN_DEPTH + w * (MAX_DEPTH - MIN_DEPTH);
        self.enabled = width > 0;
    }

    /// Set raw depth directly (1.0-3.0)
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.clamp(MIN_DEPTH, MAX_DEPTH);
        self.enabled = self.depth > 1.01;
    }

    pub fn depth(&self) -> f32 { self.depth }

    /// Convert 0-10 TTPlayer scale back to width
    pub fn width(&self) -> u8 {
        ((self.depth - MIN_DEPTH) / (MAX_DEPTH - MIN_DEPTH) * 10.0).round().min(10.0).max(0.0) as u8
    }
}

/// Soft-clip using tanh approximation — prevents overshoot beyond ±1.0
/// while preserving natural dynamics better than hard clipping.
#[inline]
fn soft_clip(x: f32) -> f32 {
    x.tanh()
}

impl AudioProcessor for SurroundProcessor {
    fn name(&self) -> &'static str { "surround" }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, buffer: &mut AudioBuffer) -> anyhow::Result<()> {
        if !self.enabled { return Ok(()); }
        if buffer.channels != 2 { return Ok(()); }

        let depth = self.depth;
        let frames = buffer.frames;

        // SAFETY: we know channels == 2
        let (left, right) = {
            let (a, b) = buffer.data.split_at_mut(1);
            (&mut a[0], &mut b[0])
        };

        for i in 0..frames {
            let l = left[i];
            let r = right[i];

            // L/R → Mid/Side
            let mid = (l + r) * 0.5;
            let side = (l - r) * 0.5;

            // Scale side for wider image
            let side_wide = side * depth;

            // Mid/Side → L/R with soft-clip
            left[i]  = soft_clip(mid + side_wide);
            right[i] = soft_clip(mid - side_wide);
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.depth = 1.0;
        self.enabled = false;
    }

    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
}
