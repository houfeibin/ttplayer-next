pub mod biquad;
pub mod crossfade;
pub mod equalizer;
pub mod fade;
pub mod replay_gain;
pub mod spectrum;
pub mod surround;

use crate::buffer::AudioBuffer;
pub use equalizer::Equalizer;
pub use equalizer::EQ_CENTER_FREQS;
pub use fade::crossfade_mix;
pub use fade::FadeProcessor;
pub use replay_gain::ReplayGainProcessor;
pub use surround::SurroundProcessor;

/// Audio DSP processor trait
pub trait AudioProcessor: Send + Sync {
    fn name(&self) -> &'static str;
    fn process(&mut self, buffer: &mut AudioBuffer) -> anyhow::Result<()>;
    fn reset(&mut self) {}
    fn set_sample_rate(&mut self, _rate: u32) {}
    fn set_channels(&mut self, _channels: u16) {}
    fn enabled(&self) -> bool { true }
    fn set_enabled(&mut self, _enabled: bool) {}
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Simple volume control
pub struct VolumeProcessor {
    gain: f32,
}

impl VolumeProcessor {
    pub fn new() -> Self {
        Self { gain: 1.0 }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.gain = volume.clamp(0.0, 2.0);
    }

    pub fn volume(&self) -> f32 {
        self.gain
    }
}

impl AudioProcessor for VolumeProcessor {
    fn name(&self) -> &'static str { "volume" }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }

    fn process(&mut self, buffer: &mut AudioBuffer) -> anyhow::Result<()> {
        if (self.gain - 1.0).abs() < f32::EPSILON {
            return Ok(());
        }
        for ch in buffer.data.iter_mut() {
            for sample in ch.iter_mut() {
                *sample *= self.gain;
            }
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.gain = 1.0;
    }
}

/// DSP chain: ReplayGain → EQ → Volume → Surround → Fade
pub struct DspChain {
    processors: Vec<Box<dyn AudioProcessor>>,
}

impl DspChain {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            processors: vec![
                Box::new(ReplayGainProcessor::new()),
                Box::new(Equalizer::new(sample_rate as f64)),
                Box::new(VolumeProcessor::new()),
                Box::new(SurroundProcessor::new()),
                Box::new(FadeProcessor::new()),
            ],
        }
    }

    pub fn process(&mut self, buffer: &mut AudioBuffer) -> anyhow::Result<()> {
        for proc in &mut self.processors {
            if proc.enabled() {
                proc.process(buffer)?;
            }
        }
        Ok(())
    }

    pub fn equalizer(&mut self) -> Option<&mut Equalizer> {
        self.processors
            .iter_mut()
            .find_map(|p| p.as_any_mut().downcast_mut::<Equalizer>())
    }

    pub fn replay_gain(&mut self) -> Option<&mut ReplayGainProcessor> {
        self.processors
            .iter_mut()
            .find_map(|p| p.as_any_mut().downcast_mut::<ReplayGainProcessor>())
    }

    pub fn surround_processor(&mut self) -> Option<&mut SurroundProcessor> {
        self.processors
            .iter_mut()
            .find_map(|p| p.as_any_mut().downcast_mut::<SurroundProcessor>())
    }

    pub fn fade_processor(&mut self) -> Option<&mut FadeProcessor> {
        self.processors
            .iter_mut()
            .find_map(|p| p.as_any_mut().downcast_mut::<FadeProcessor>())
    }

    pub fn volume_processor(&mut self) -> Option<&mut VolumeProcessor> {
        self.processors
            .iter_mut()
            .find_map(|p| p.as_any_mut().downcast_mut::<VolumeProcessor>())
    }

    pub fn set_sample_rate(&mut self, rate: u32) {
        for proc in &mut self.processors {
            proc.set_sample_rate(rate);
        }
    }

    pub fn set_channels(&mut self, channels: u16) {
        for proc in &mut self.processors {
            proc.set_channels(channels);
        }
    }
}
