/// ReplayGain processor: applies track/album gain during decoding.
///
/// ReplayGain stores gain as dB relative to a reference level of 89 dB SPL.
/// Conversion: linear_gain = 10^(gain_dB / 20)
///
/// This sits as the FIRST processor in the DSP chain (before EQ and volume),
/// so subsequent EQ bands still see the normalized signal.
use crate::buffer::AudioBuffer;
use super::AudioProcessor;

/// Apply ReplayGain if available. Ignores if gain is near 0 dB.
pub struct ReplayGainProcessor {
    linear_gain: f32,
    gain_db: f32,
    enabled: bool,
}

impl ReplayGainProcessor {
    pub fn new() -> Self {
        Self {
            linear_gain: 1.0,
            gain_db: 0.0,
            enabled: false,
        }
    }

    /// Set gain from a ReplayGainInfo, preferring track_gain over album_gain
    pub fn set_from_rg(&mut self, rg: &tt_common::ReplayGainInfo) {
        self.gain_db = if rg.track_gain.abs() > 0.01 {
            rg.track_gain as f32
        } else if rg.album_gain.abs() > 0.01 {
            rg.album_gain as f32
        } else {
            0.0
        };
        self.linear_gain = 10.0_f32.powf(self.gain_db / 20.0);
        self.enabled = self.gain_db.abs() > 0.01;
    }

    /// Direct dB gain set
    pub fn set_gain_db(&mut self, db: f32) {
        self.gain_db = db;
        self.linear_gain = 10.0_f32.powf(db / 20.0);
        self.enabled = db.abs() > 0.01;
    }

    pub fn gain_db(&self) -> f32 { self.gain_db }
}

impl AudioProcessor for ReplayGainProcessor {
    fn name(&self) -> &'static str { "replay_gain" }
    fn process(&mut self, buffer: &mut AudioBuffer) -> anyhow::Result<()> {
        if !self.enabled { return Ok(()); }
        for ch in buffer.data.iter_mut() {
            for sample in ch.iter_mut() {
                *sample *= self.linear_gain;
            }
        }
        Ok(())
    }
    fn reset(&mut self) {
        self.linear_gain = 1.0;
        self.gain_db = 0.0;
        self.enabled = false;
    }
    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, e: bool) { self.enabled = e; }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}
