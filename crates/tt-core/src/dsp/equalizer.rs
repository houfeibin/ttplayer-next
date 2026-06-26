/// 10-band graphic equalizer with biquad IIR filters
///
/// ISO standard center frequencies: 31, 62, 125, 250, 500, 1k, 2k, 4k, 8k, 16k Hz
use crate::buffer::AudioBuffer;
use crate::dsp::biquad::BiquadFilter;
use super::AudioProcessor;

const EQ_BAND_COUNT: usize = 10;

/// ISO 10-band center frequencies
pub const EQ_CENTER_FREQS: [f64; EQ_BAND_COUNT] = [
    31.0, 62.0, 125.0, 250.0, 500.0,
    1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

/// 10-band equalizer
pub struct Equalizer {
    bands: [BiquadFilter; EQ_BAND_COUNT],
    /// Gain per band in dB (-12 to +12)
    band_gains: [f64; EQ_BAND_COUNT],
    /// Preamp gain in dB (-12 to +12)
    preamp: f64,
    sample_rate: f64,
    channels: u16,
    enabled: bool,
}

impl Equalizer {
    pub fn new(sample_rate: f64) -> Self {
        let bands = std::array::from_fn(|i| {
            BiquadFilter::new_peaking(sample_rate, EQ_CENTER_FREQS[i], 0.7, 0.0)
        });
        Self {
            bands,
            band_gains: [0.0f64; EQ_BAND_COUNT],
            preamp: 0.0,
            sample_rate,
            channels: 2,
            enabled: true,
        }
    }

    /// Set gain for a band (0..=9), dB range [-12.0, 12.0]
    pub fn set_band_gain(&mut self, band: usize, gain_db: f64) {
        let clamped = gain_db.clamp(-12.0, 12.0);
        self.band_gains[band] = clamped;
        self.bands[band].set_gain(clamped);
    }

    /// Get current gain for a band
    pub fn band_gain(&self, band: usize) -> f64 {
        self.band_gains[band]
    }

    /// Set preamp gain in dB
    pub fn set_preamp(&mut self, gain_db: f64) {
        self.preamp = gain_db.clamp(-12.0, 12.0);
    }

    pub fn preamp(&self) -> f64 {
        self.preamp
    }

    /// Reset all bands to flat (0 dB)
    pub fn reset(&mut self) {
        for i in 0..EQ_BAND_COUNT {
            self.set_band_gain(i, 0.0);
        }
        self.preamp = 0.0;
    }

    fn reconfigure_channels(&mut self, channels: u16) {
        self.channels = channels;
        for band in &mut self.bands {
            band.set_channels(channels);
        }
    }
}

impl AudioProcessor for Equalizer {
    fn name(&self) -> &'static str { "equalizer" }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, en: bool) { self.enabled = en; }

    fn set_sample_rate(&mut self, rate: u32) {
        self.sample_rate = rate as f64;
        for band in &mut self.bands { band.set_sample_rate(rate as f64); }
    }

    fn set_channels(&mut self, channels: u16) {
        self.reconfigure_channels(channels);
    }

    fn process(&mut self, buffer: &mut AudioBuffer) -> anyhow::Result<()> {
        if !self.enabled { return Ok(()); }

        // Ensure state matches buffer channel count
        let ch = buffer.channels as usize;
        if ch != self.channels as usize {
            self.reconfigure_channels(buffer.channels);
        }

        // Apply preamp (linear scale)
        let preamp_lin = f64::powf(10.0, self.preamp / 20.0) as f32;

        for (ci, channel) in buffer.data.iter_mut().enumerate() {
            for sample in channel.iter_mut() {
                let mut val = *sample;
                // Apply each band
                for band in &mut self.bands {
                    val = band.process_sample(ci, val);
                }
                *sample = val * preamp_lin;
            }
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.bands.iter_mut().for_each(|b| b.reset_state());
    }
}
