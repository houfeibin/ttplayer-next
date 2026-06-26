/// Unified audio buffer: f32 planar layout
#[derive(Debug, Clone)]
pub struct AudioBuffer {
    /// data[channel][sample]
    pub data: Vec<Vec<f32>>,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: usize,
}

impl AudioBuffer {
    pub fn new(channels: u16, frames: usize, sample_rate: u32) -> Self {
        Self {
            data: vec![vec![0.0f32; frames]; channels as usize],
            sample_rate,
            channels,
            frames,
        }
    }

    pub fn from_interleaved(interleaved: &[f32], channels: u16, sample_rate: u32) -> Self {
        let frames = interleaved.len() / channels as usize;
        let mut data = vec![vec![0.0f32; frames]; channels as usize];
        for (i, sample) in interleaved.iter().enumerate() {
            data[i % channels as usize][i / channels as usize] = *sample;
        }
        Self { data, sample_rate, channels, frames }
    }

    pub fn interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.frames * self.channels as usize);
        for frame in 0..self.frames {
            for ch in 0..self.channels as usize {
                out.push(self.data[ch][frame]);
            }
        }
        out
    }

    /// Write the planar data as interleaved samples into `out`, reusing its
    /// allocation (cleared first, grown only if necessary).
    ///
    /// Use this on the decode hot path instead of [`interleaved`] to avoid
    /// allocating a new `Vec` on every decoded chunk.
    pub fn interleaved_into(&self, out: &mut Vec<f32>) {
        out.clear();
        out.reserve(self.frames * self.channels as usize);
        for frame in 0..self.frames {
            for ch in 0..self.channels as usize {
                out.push(self.data[ch][frame]);
            }
        }
    }

    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        (self.frames as f64 * 1000.0 / self.sample_rate as f64) as u64
    }
}
