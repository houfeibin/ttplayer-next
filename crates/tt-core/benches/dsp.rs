//! Microbenchmarks for the real-time audio hot paths.
//!
//! Run with: `cargo bench -p tt-core`
//!
//! These cover the three per-callback hot spots the technical assessment flagged:
//!   * `DspChain::process` — runs on every decoded block
//!   * `SpectrumAnalyzer::analyze` — runs (via the condvar thread) on every
//!     callback that feeds new samples
//!   * `PlaybackRing::write` — the decode→ring copy, optimized to `copy_from_slice`
//!
//! The ring bench uses a 60 s capacity so the buffer never fills during the
//! measurement window, isolating the copy cost from the "ring full" early-return.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use tt_core::dsp::DspChain;
use tt_core::dsp::spectrum::SpectrumAnalyzer;
use tt_core::output::{AtomicVolume, PlaybackRing};
use tt_core::AudioBuffer;

fn signal(frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| (i as f32 * 0.01).sin() * 0.5)
        .collect()
}

fn bench_dsp_chain(c: &mut Criterion) {
    let mut chain = DspChain::new(44100);
    let mut buf = AudioBuffer::new(2, 1024, 44100);
    let wave = signal(1024);
    for ch in buf.data.iter_mut() {
        ch.copy_from_slice(&wave);
    }
    c.bench_function("dsp_chain_process_1024f_stereo", |b| {
        b.iter(|| {
            black_box(chain.process(black_box(&mut buf)))
        })
    });
}

fn bench_spectrum(c: &mut Criterion) {
    let mut analyzer = SpectrumAnalyzer::new(2048, 256, 44100, 0.85);
    analyzer.feed_interleaved(&signal(2048));
    c.bench_function("spectrum_analyze_2048", |b| {
        b.iter(|| {
            black_box(analyzer.analyze())
        })
    });
}

fn bench_ring_write(c: &mut Criterion) {
    let volume = Arc::new(AtomicVolume::new(1.0));
    // 60 s capacity so the ring never fills during the ~5 s measurement window.
    let ring = PlaybackRing::new(44100, 2, 60.0, volume);
    let samples = signal(2048); // 1024 stereo frames interleaved
    c.bench_function("ring_write_1024f_stereo", |b| {
        b.iter(|| {
            black_box(ring.write(black_box(&samples)))
        })
    });
}

criterion_group!(benches, bench_dsp_chain, bench_spectrum, bench_ring_write);
criterion_main!(benches);
