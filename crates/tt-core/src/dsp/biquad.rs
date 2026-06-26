/// IIR Biquad filter (Direct Form I)
///
/// Transfer function:
///   H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
///
/// Supports: peaking EQ, low-shelf, high-shelf, low-pass, high-pass
#[derive(Debug, Clone)]
pub struct BiquadFilter {
    // Coefficients
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,

    // State (per channel, allocated lazily)
    x1: Vec<f64>,
    x2: Vec<f64>,
    y1: Vec<f64>,
    y2: Vec<f64>,

    filter_type: BiquadType,
    freq: f64,      // Hz
    q: f64,
    gain_db: f64,   // dB (peaking/shelf only)
    sample_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BiquadType {
    Peaking,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
}

impl BiquadFilter {
    /// Create a peaking EQ filter (default: flat)
    pub fn new_peaking(sample_rate: f64, freq: f64, q: f64, gain_db: f64) -> Self {
        let mut f = Self {
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: vec![], x2: vec![], y1: vec![], y2: vec![],
            filter_type: BiquadType::Peaking,
            freq, q, gain_db, sample_rate,
        };
        f.recalc();
        f
    }

    pub fn new_low_shelf(sample_rate: f64, freq: f64, q: f64, gain_db: f64) -> Self {
        let mut f = Self {
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: vec![], x2: vec![], y1: vec![], y2: vec![],
            filter_type: BiquadType::LowShelf,
            freq, q, gain_db, sample_rate,
        };
        f.recalc();
        f
    }

    pub fn new_high_shelf(sample_rate: f64, freq: f64, q: f64, gain_db: f64) -> Self {
        let mut f = Self {
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: vec![], x2: vec![], y1: vec![], y2: vec![],
            filter_type: BiquadType::HighShelf,
            freq, q, gain_db, sample_rate,
        };
        f.recalc();
        f
    }

    /// Recalculate coefficients from freq, Q, gain
    pub fn recalc(&mut self) {
        self.recalc_impl();
    }

    fn recalc_impl(&mut self) {
        let f = self.freq;
        let q = self.q.max(0.1);
        let g = self.gain_db;

        // Guard: no-op if freq is outside valid range
        if f <= 1.0 || f >= self.sample_rate * 0.49 {
            self.b0 = 1.0; self.b1 = 0.0; self.b2 = 0.0;
            self.a1 = 0.0; self.a2 = 0.0;
            return;
        }

        let w0 = 2.0 * std::f64::consts::PI * f / self.sample_rate;
        let alpha = w0.sin() / (2.0 * q);

        match self.filter_type {
            BiquadType::Peaking => {
                let a = f64::powf(10.0, g / 40.0); // sqrt of linear gain
                let a_alpha = a * alpha;
                let a_alpha_inv = alpha / a;

                self.b0 = 1.0 + a_alpha;
                self.b1 = -2.0 * w0.cos();
                self.b2 = 1.0 - a_alpha;
                self.a1 = -2.0 * w0.cos();
                self.a2 = 1.0 - a_alpha_inv;

                // Normalize
                let a0 = 1.0 + a_alpha_inv;
                self.b0 /= a0;
                self.b1 /= a0;
                self.b2 /= a0;
                self.a1 = -2.0 * w0.cos() / a0;
                self.a2 = (1.0 - a_alpha_inv) / a0;
            }
            BiquadType::LowShelf => {
                let a = f64::powf(10.0, g / 40.0);
                let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

                self.b0 = a * ((a + 1.0) - (a - 1.0) * w0.cos() + two_sqrt_a_alpha);
                self.b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * w0.cos());
                self.b2 = a * ((a + 1.0) - (a - 1.0) * w0.cos() - two_sqrt_a_alpha);
                self.a1 = -2.0 * ((a - 1.0) + (a + 1.0) * w0.cos());
                self.a2 = (a + 1.0) + (a - 1.0) * w0.cos() - two_sqrt_a_alpha;

                let a0 = (a + 1.0) + (a - 1.0) * w0.cos() + two_sqrt_a_alpha;
                self.b0 /= a0; self.b1 /= a0; self.b2 /= a0;
                self.a1 = -(-2.0 * ((a - 1.0) + (a + 1.0) * w0.cos())) / a0;
                self.a2 = -((a + 1.0) + (a - 1.0) * w0.cos() - two_sqrt_a_alpha) / a0;
            }
            BiquadType::HighShelf => {
                let a = f64::powf(10.0, g / 40.0);
                let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;

                self.b0 = a * ((a + 1.0) + (a - 1.0) * w0.cos() + two_sqrt_a_alpha);
                self.b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * w0.cos());
                self.b2 = a * ((a + 1.0) + (a - 1.0) * w0.cos() - two_sqrt_a_alpha);
                self.a1 = 2.0 * ((a - 1.0) - (a + 1.0) * w0.cos());
                self.a2 = (a + 1.0) - (a - 1.0) * w0.cos() - two_sqrt_a_alpha;

                let a0 = (a + 1.0) - (a - 1.0) * w0.cos() + two_sqrt_a_alpha;
                self.b0 /= a0; self.b1 /= a0; self.b2 /= a0;
                self.a1 = -(2.0 * ((a - 1.0) - (a + 1.0) * w0.cos())) / a0;
                self.a2 = -((a + 1.0) - (a - 1.0) * w0.cos() - two_sqrt_a_alpha) / a0;
            }
            BiquadType::LowPass => {
                self.b0 = (1.0 - w0.cos()) / 2.0;
                self.b1 = 1.0 - w0.cos();
                self.b2 = (1.0 - w0.cos()) / 2.0;
                self.a1 = -2.0 * w0.cos();
                self.a2 = 1.0 - alpha;

                let a0 = 1.0 + alpha;
                self.b0 /= a0; self.b1 /= a0; self.b2 /= a0;
                self.a1 = -(-2.0 * w0.cos()) / a0;
                self.a2 = -(1.0 - alpha) / a0;
            }
            BiquadType::HighPass => {
                self.b0 = (1.0 + w0.cos()) / 2.0;
                self.b1 = -(1.0 + w0.cos());
                self.b2 = (1.0 + w0.cos()) / 2.0;
                self.a1 = -2.0 * w0.cos();
                self.a2 = 1.0 - alpha;

                let a0 = 1.0 + alpha;
                self.b0 /= a0; self.b1 /= a0; self.b2 /= a0;
                self.a1 = -(-2.0 * w0.cos()) / a0;
                self.a2 = -(1.0 - alpha) / a0;
            }
        }
    }

    /// Process one sample for one channel (Direct Form I)
    #[inline]
    pub fn process_sample(&mut self, ch: usize, input: f32) -> f32 {
        let x0 = input as f64;
        let y0 = self.b0 * x0
            + self.b1 * self.x1[ch]
            + self.b2 * self.x2[ch]
            - self.a1 * self.y1[ch]
            - self.a2 * self.y2[ch];

        self.x2[ch] = self.x1[ch];
        self.x1[ch] = x0;
        self.y2[ch] = self.y1[ch];
        self.y1[ch] = y0;

        y0 as f32
    }

    /// Ensure state buffers match channel count
    pub fn set_channels(&mut self, channels: u16) {
        let ch = channels as usize;
        if self.x1.len() != ch {
            self.x1 = vec![0.0f64; ch];
            self.x2 = vec![0.0f64; ch];
            self.y1 = vec![0.0f64; ch];
            self.y2 = vec![0.0f64; ch];
        }
    }

    pub fn reset_state(&mut self) {
        for v in &mut self.x1 { *v = 0.0; }
        for v in &mut self.x2 { *v = 0.0; }
        for v in &mut self.y1 { *v = 0.0; }
        for v in &mut self.y2 { *v = 0.0; }
    }

    pub fn set_freq(&mut self, freq: f64) { self.freq = freq; self.recalc(); }
    pub fn set_q(&mut self, q: f64) { self.q = q; self.recalc(); }
    pub fn set_gain(&mut self, gain_db: f64) { self.gain_db = gain_db; self.recalc(); }
    pub fn set_sample_rate(&mut self, sr: f64) { self.sample_rate = sr; self.recalc(); }
}
