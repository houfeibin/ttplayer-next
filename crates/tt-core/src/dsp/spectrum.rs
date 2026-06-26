/// Real-time FFT spectrum analyzer
///
/// Uses `rustfft` for frequency analysis. Takes PCM samples from the
/// output ring buffer, computes FFT magnitude, maps to log-spaced bands.
use rustfft::{Fft, FftPlanner, num_complex::Complex};
use std::sync::Arc;

/// Spectrum data sent to frontend for visualization
#[derive(Debug, Clone)]
pub struct SpectrumFrame {
    /// Magnitude per band, normalized 0.0..1.0
    pub bands: Arc<[f32; 256]>,
    /// Peak magnitude across all bands
    pub peak: f32,
}

impl Default for SpectrumFrame {
    fn default() -> Self {
        Self {
            bands: Arc::new([0.0f32; 256]),
            peak: 0.0,
        }
    }
}

/// FFT-based spectrum analyzer
pub struct SpectrumAnalyzer {
    /// FFT size (power of 2)
    fft_size: usize,
    /// Sample buffer (interleaved, windowed)
    samples: Vec<f32>,
    /// Hann window coefficients
    window: Vec<f32>,
    /// Cached forward FFT plan. `FftPlanner` does cache internally, but every
    /// `analyze()` call (≈ every audio callback, ~88×/s at 48 kHz) would
    /// otherwise pay a HashMap lookup + mutex under `plan_fft_forward`. Holding
    /// the `Arc<dyn Fft>` directly removes that per-call overhead entirely.
    fft: Arc<dyn Fft<f32>>,
    /// Log-frequency band edges
    band_edges: Vec<usize>,
    /// Number of output bands
    num_bands: usize,
    /// Sample rate
    sample_rate: u32,
    /// Smoothing per band (exponential moving average)
    smooth_decay: f32,
    smoothed: Option<[f32; 256]>,
    /// Latest result
    latest: SpectrumFrame,
}

impl SpectrumAnalyzer {
    /// Create a new spectrum analyzer
    ///
    /// * `fft_size` — power of 2 (recommend 2048 or 4096)
    /// * `num_bands` — output band count (typically 256 for display)
    /// * `sample_rate` — audio sample rate
    /// * `smooth_decay` — EMA decay per update (0.0 = instantaneous, 0.9 = heavy smooth)
    pub fn new(fft_size: usize, num_bands: usize, sample_rate: u32, smooth_decay: f32) -> Self {
        assert!(fft_size.is_power_of_two(), "FFT size must be power of 2");
        assert!(num_bands > 0 && num_bands <= 256, "num_bands must be 1..256");

        // Hann window
        let window: Vec<f32> = (0..fft_size)
            .map(|n| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * n as f32 / (fft_size - 1) as f32).cos()))
            .collect();

        // Log-spaced band edges (nyquist = sample_rate/2)
        let nyquist = sample_rate as f64 / 2.0;
        let min_freq = 20.0f64;
        let log_min = min_freq.ln();
        let log_max = nyquist.ln();
        let band_edges: Vec<usize> = (0..=num_bands)
            .map(|i| {
                let freq = (log_min + (log_max - log_min) * i as f64 / num_bands as f64).exp();
                // Map frequency to FFT bin
                ((freq * fft_size as f64 / sample_rate as f64).round() as usize)
                    .min(fft_size / 2)
            })
            .collect();

        // Plan the forward FFT once. The planner is dropped right after —
        // `Arc<dyn Fft>` is self-contained and needs no further planning.
        let fft = {
            let mut planner = FftPlanner::<f32>::new();
            planner.plan_fft_forward(fft_size)
        };

        Self {
            fft_size,
            samples: vec![0.0f32; fft_size],
            window,
            fft,
            band_edges,
            num_bands,
            sample_rate,
            smooth_decay,
            smoothed: None,
            latest: SpectrumFrame::default(),
        }
    }

    /// Feed interleaved audio samples into the analyzer.
    /// Call this from the audio output callback.
    pub fn feed_interleaved(&mut self, interleaved: &[f32]) {
        // Simple ring-buffer push
        let n = interleaved.len().min(self.samples.len());
        // Shift old samples back
        if n < self.samples.len() {
            self.samples.copy_within(n.., 0);
        }
        let dest_start = self.samples.len() - n;
        self.samples[dest_start..].copy_from_slice(&interleaved[..n]);
    }

    /// Feed planar samples from one channel
    pub fn feed_channel(&mut self, channel_data: &[f32]) {
        self.feed_interleaved(channel_data);
    }

    /// Compute spectrum from accumulated samples.
    /// Returns a reference to the latest SpectrumFrame.
    pub fn analyze(&mut self) -> &SpectrumFrame {
        // Window and convert to complex
        let mut input: Vec<Complex<f32>> = self.samples
            .iter()
            .enumerate()
            .map(|(i, &s)| Complex::new(s * self.window[i], 0.0))
            .collect();

        self.fft.process(&mut input);

        // Compute magnitude per bin (first half only, real output)
        let mag: Vec<f32> = input[..self.fft_size / 2]
            .iter()
            .map(|c| c.norm() / (self.fft_size as f32).sqrt())
            .collect();

        // Aggregate into log-spaced bands
        let mut bands = [0.0f32; 256];
        for b in 0..self.num_bands {
            let start = self.band_edges[b];
            let end = self.band_edges[b + 1].max(start + 1);
            let sum: f32 = mag[start..end].iter().sum();
            let count = (end - start) as f32;
            bands[b] = (sum / count).sqrt(); // perceptual: sqrt of energy
        }

        // Normalize and smooth
        let max_mag = bands.iter().cloned().fold(0.0f32, f32::max).max(1e-6);
        for b in 0..self.num_bands {
            bands[b] = (bands[b] / max_mag).clamp(0.0, 1.0);
        }

        // EMA smoothing
        if let Some(ref prev) = self.smoothed {
            for b in 0..self.num_bands {
                bands[b] = prev[b] * self.smooth_decay + bands[b] * (1.0 - self.smooth_decay);
            }
        }
        self.smoothed = Some(bands);

        self.latest = SpectrumFrame {
            bands: Arc::new(bands),
            peak: max_mag,
        };

        &self.latest
    }

    pub fn latest(&self) -> &SpectrumFrame { &self.latest }
    pub fn sample_rate(&self) -> u32 { self.sample_rate }
}
