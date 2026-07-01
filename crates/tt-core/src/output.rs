use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use parking_lot::Mutex;
use rubato::Resampler;
use tokio::sync::Notify;

use crate::dsp::spectrum::{SpectrumAnalyzer, SpectrumFrame};

/// Wrapper around `cpal::Stream` that makes it `Send`.
///
/// # Why this is needed
/// `cpal::Stream` is `!Send` on WASAPI (it owns raw native thread handles).
/// However, `AudioOutput` is stored in an `Arc<Mutex<Option<AudioOutput>>>`
/// shared between the setup thread and the command thread, so the type must be
/// `Send` to compile.
///
/// # Safety invariant
/// The stream is created and dropped on the owning thread's behalf and is
/// never invoked concurrently from multiple threads: cpal drives the audio
/// callback on its own internal thread, and the only cross-thread operations
/// we perform are `play()` (once, at creation) and dropping the stream in
/// [`AudioOutput::stop`]. The handle is never sent *into* another thread to be
/// operated on; it merely lives behind a shared lock. If cpal's threading model
/// ever requires the stream to be fully thread-local, this must be replaced
/// with a dedicated owner thread + message passing.
struct SendStream(Option<cpal::Stream>);
unsafe impl Send for SendStream {}

/// Thread-safe volume control: 0.0-1.0 linear gain
#[derive(Debug)]
pub struct AtomicVolume {
    pub bits: std::sync::atomic::AtomicU32,
}

impl AtomicVolume {
    pub fn new(volume: f32) -> Self {
        Self {
            bits: std::sync::atomic::AtomicU32::new(volume.clamp(0.0, 2.0).to_bits()),
        }
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.bits.load(Ordering::Relaxed))
    }

    pub fn set(&self, volume: f32) {
        self.bits.store(volume.clamp(0.0, 2.0).to_bits(), Ordering::Relaxed);
    }
}

/// Safe ring buffer: decode thread writes, output callback reads.
/// Uses Mutex for safe concurrent access -?critical sections are memcpy only (~µs).
pub struct PlaybackRing {
    inner: Mutex<RingInner>,
    pub sample_rate: u32,
    pub channels: u16,
    pub capacity_frames: usize,
    pub volume: Arc<AtomicVolume>,
    /// Notified when the output callback consumes frames (writer can unblock)
    read_notify: Notify,
}

struct RingInner {
    buf: Vec<f32>,
    write_pos: u64, // monotonic frames
    read_pos: u64,  // monotonic frames
}

impl PlaybackRing {
    pub fn new(sample_rate: u32, channels: u16, duration_secs: f32, volume: Arc<AtomicVolume>) -> Self {
        let capacity_frames = (sample_rate as f32 * duration_secs) as usize;
        let capacity_samples = capacity_frames * channels as usize;
        Self {
            inner: Mutex::new(RingInner {
                buf: vec![0.0f32; capacity_samples],
                write_pos: 0,
                read_pos: 0,
            }),
            sample_rate,
            channels,
            capacity_frames,
            volume,
            read_notify: Notify::new(),
        }
    }

    pub fn available(&self) -> usize {
        let inner = self.inner.lock();
        (inner.write_pos - inner.read_pos) as usize
    }

    pub fn free(&self) -> usize {
        self.capacity_frames.saturating_sub(self.available())
    }

    /// Write interleaved samples. Returns frames actually written.
    pub fn write(&self, samples: &[f32]) -> usize {
        let ch = self.channels as usize;
        let frames = samples.len() / ch;
        if frames == 0 { return 0; }

        let mut inner = self.inner.lock();
        let avail = self.capacity_frames.saturating_sub((inner.write_pos - inner.read_pos) as usize);
        let to_write = frames.min(avail);
        if to_write == 0 { return 0; }

        let cap = self.capacity_frames;
        let frame_offset = inner.write_pos as usize % cap;
        let offset = frame_offset * ch;

        // Bulk copy with wrap handling. The ring is a flat sample array; a
        // write may straddle the end, so we split it into at most two
        // contiguous memcpy segments instead of the previous per-sample
        // (mod + index) loop. At 44.1 kHz stereo this was ~88k scalar
        // writes/second; copy_from_slice lets LLVM emit a single rep movsb.
        let contiguous = cap - frame_offset;
        let first = to_write.min(contiguous);
        let second = to_write - first;
        if first > 0 {
            let n = first * ch;
            inner.buf[offset..offset + n].copy_from_slice(&samples[..n]);
        }
        if second > 0 {
            let n = second * ch;
            let off = first * ch;
            inner.buf[..n].copy_from_slice(&samples[off..off + n]);
        }

        inner.write_pos += to_write as u64;
        to_write
    }

    /// Read a contiguous block of source frames `[start, start + out.len())`
    /// into `out` as stereo pairs. Returns the number of frames actually read
    /// (may be less than `out.len()` on ring underrun). Does **not** advance
    /// `read_pos`; the caller advances it once via [`advance_read`].
    ///
    /// This batches what was previously a per-frame lock into a single lock,
    /// eliminating the ~88k mutex acquisitions/second the output callback used
    /// to perform at 44.1 kHz stereo.
    pub fn read_block(&self, start: u64, out: &mut [[f32; 2]]) -> usize {
        if out.is_empty() {
            return 0;
        }
        let inner = self.inner.lock();
        let avail = inner.write_pos.saturating_sub(start);
        let to_read = (out.len() as u64).min(avail) as usize;
        let ch = self.channels as usize;
        let cap = self.capacity_frames;
        for i in 0..to_read {
            let pos = start + i as u64;
            let offset = (pos as usize % cap) * ch;
            let s0 = inner.buf[offset];
            let s1 = if ch >= 2 { inner.buf[offset + 1] } else { s0 };
            out[i] = [s0, s1];
        }
        to_read
    }

    /// Advance `read_pos` by `frames` and notify the decode thread that ring
    /// space has freed up. Called once per output callback instead of per frame.
    pub fn advance_read(&self, frames: u64) {
        if frames == 0 {
            return;
        }
        {
            let mut inner = self.inner.lock();
            inner.read_pos = inner.read_pos.saturating_add(frames);
        }
        self.read_notify.notify_one();
    }

    /// Async wait until ring has free space. Returns free frames.
    pub async fn wait_for_free(&self, min_frames: usize) -> usize {
        loop {
            let free = self.free();
            if free >= min_frames {
                return free;
            }
            self.read_notify.notified().await;
        }
    }

    /// Async wait until the ring has been fully drained (no buffered frames
    /// left for the output callback to consume).
    ///
    /// Used by the crossfade path: after the mixer finishes writing the
    /// fade-out tail, the decode thread must wait for the output callback to
    /// actually play those samples before signaling `Stopped`. Without this,
    /// the frontend's `playNext()` calls `stop_inner()` which flushes the
    /// ring, discarding the un-played fade-out tail (and any un-mixed
    /// current-track samples) — causing the "track cut off a few seconds
    /// early, no crossfade heard" symptom.
    ///
    /// Resolves immediately if the ring is already empty. Driven by
    /// `read_notify` (signalled by `advance_read`), so idle CPU stays at zero.
    pub async fn wait_until_drained(&self) {
        loop {
            if self.available() == 0 {
                return;
            }
            self.read_notify.notified().await;
        }
    }

    pub fn position_ms(&self) -> u64 {
        let inner = self.inner.lock();
        if self.sample_rate == 0 { return 0; }
        (inner.read_pos as f64 * 1000.0 / self.sample_rate as f64) as u64
    }

    /// Seek: reset both positions to the given frame offset.
    pub fn seek_to_frame(&self, frame: u64) {
        let mut inner = self.inner.lock();
        inner.read_pos = frame;
        inner.write_pos = frame;
    }
}

// Mutex provides safe Send+Sync -?no unsafe impl needed.

/// Wakeup + shutdown signaling for the spectrum analysis thread.
///
/// Replaces the previous 30 ms `sleep` + `ready` flag polling loop: the
/// analysis thread blocks on the condvar and is woken *only* when the output
/// callback has fed new samples (or when shutting down), so idle CPU stays at
/// zero instead of busy-polling.
struct SpectrumWakeup {
    ready: std::sync::Mutex<bool>,
    cvar: std::sync::Condvar,
    shutdown: std::sync::atomic::AtomicBool,
}

impl SpectrumWakeup {
    /// Called from the audio callback after new samples were fed.
    fn signal(&self) {
        {
            let mut r = self.ready.lock().unwrap();
            *r = true;
        }
        self.cvar.notify_one();
    }

    /// Called from [`AudioOutput::stop`] to end the analysis thread.
    fn request_shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.cvar.notify_all();
    }
}

/// Audio output layer (cpal wrapper)
pub struct AudioOutput {
    _host: cpal::Host,
    _device: cpal::Device,
    stream: SendStream,
    playing: Arc<Mutex<bool>>,
    /// Spectrum analyzer (updated every audio callback)
    pub spectrum: Arc<Mutex<Option<Arc<SpectrumFrame>>>>,
    /// Wakeup handle for the analysis thread (also used to shut it down).
    spectrum_wakeup: Arc<SpectrumWakeup>,
}

impl AudioOutput {
    pub fn start(ring: Arc<PlaybackRing>) -> anyhow::Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| anyhow::anyhow!("no output device found"))?;

        let default_config = device.default_output_config()?;
        let device_channels = default_config.channels() as usize;
        let device_sample_rate = default_config.sample_rate().0;
        let playing = Arc::new(Mutex::new(false));
        let playing_clone = playing.clone();

        let stream_config = default_config.config();

        let ring_ref = ring.clone();

        // Spectrum: feed data in output callback, analyze in bg thread
        let spectrum = Arc::new(Mutex::new(SpectrumAnalyzer::new(
            2048, 256, ring.sample_rate, 0.85
        )));
        let spectrum_result: Arc<Mutex<Option<Arc<SpectrumFrame>>>> = Arc::new(Mutex::new(None));
        let spectrum_feed = spectrum.clone();
        let spectrum_wakeup = Arc::new(SpectrumWakeup {
            ready: std::sync::Mutex::new(false),
            cvar: std::sync::Condvar::new(),
            shutdown: std::sync::atomic::AtomicBool::new(false),
        });

        // Analysis thread: blocks on the condvar and runs only when the output
        // callback signals new data (or on shutdown). Replaces the previous
        // 30 ms sleep + flag-poll loop so idle CPU stays at zero.
        {
            let result_ref = spectrum_result.clone();
            let wakeup_ref = spectrum_wakeup.clone();
            std::thread::spawn(move || {
                loop {
                    let should_run = {
                        let mut ready = wakeup_ref.ready.lock().unwrap();
                        while !*ready && !wakeup_ref.shutdown.load(Ordering::SeqCst) {
                            ready = match wakeup_ref.cvar.wait_timeout(
                                ready,
                                std::time::Duration::from_millis(250),
                            ) {
                                Ok((g, _)) => g,
                                Err(e) => e.into_inner().0,
                            };
                        }
                        if wakeup_ref.shutdown.load(Ordering::SeqCst) {
                            false
                        } else {
                            *ready = false;
                            true
                        }
                    };
                    if !should_run {
                        return;
                    }
                    let frame = {
                        let mut guard = spectrum.lock();
                        guard.analyze().clone()
                    };
                    *result_ref.lock() = Some(Arc::new(frame));
                }
            });
        }

        let spectrum_wakeup_cb = spectrum_wakeup.clone();
        // Reusable source-frame scratch buffer, grown on demand and kept across
        // callbacks to avoid per-callback allocation.
        let mut src_buf: Vec<[f32; 2]> = Vec::new();

        // ── rubato resampling state ──
        // When the device sample rate differs from the ring (source) rate, a
        // band-limited sinc resampler replaces the previous per-callback linear
        // interpolation. The old code (a) aliased high frequencies and (b)
        // snapped the fractional source read position to an integer every
        // callback, leaking sub-sample phase and causing periodic pitch drift.
        // rubato carries its own phase and anti-alias filtering. When rates match
        // within 1 Hz the resampler stays `None` and the direct-copy path runs
        // (no resampling overhead, ratio==1.0 → zero drift).
        let resample_ratio = device_sample_rate as f64 / ring.sample_rate as f64; // out/in
        let needs_resample = (resample_ratio - 1.0).abs() > 1e-3;
        let mut resampler: Option<rubato::SincFixedIn<f32>> = if needs_resample {
            let params = rubato::SincInterpolationParameters {
                sinc_len: 256,
                f_cutoff: 0.95,
                interpolation: rubato::SincInterpolationType::Linear,
                oversampling_factor: 256,
                window: rubato::WindowFunction::BlackmanHarris2,
            };
            match rubato::SincFixedIn::<f32>::new(resample_ratio, 2.0, params, 1024, 2) {
                Ok(r) => {
                    tracing::info!(
                        "rubato resampler active: {:.4}x ({}Hz → {}Hz)",
                        resample_ratio, ring.sample_rate, device_sample_rate
                    );
                    Some(r)
                }
                Err(e) => {
                    tracing::warn!("rubato init failed ({e}); falling back to linear interp");
                    None
                }
            }
        } else {
            None
        };
        // Planar input scratch (2 channels), reused across callbacks. Capacity is
        // reserved once so the per-callback fill loop only does amortized O(1) pushes.
        let mut in_planar: Vec<Vec<f32>> = vec![Vec::with_capacity(1024), Vec::with_capacity(1024)];
        let mut in_filled: usize = 0;
        // Carry-over buffer of resampled, interleaved output that the resampler
        // produced ahead of what the callback requested. Persists across calls so
        // the resampler's variable output length is decoupled from the device's
        // fixed callback size.
        let mut out_buf: std::collections::VecDeque<f32> =
            std::collections::VecDeque::with_capacity(4096);
        // Reused source-frame scratch for ring reads (avoids per-process alloc).
        let mut resample_scratch: Vec<[f32; 2]> = Vec::with_capacity(1024);

        let stream = device.build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                *playing_clone.lock() = true;

                let vol = ring_ref.volume.get();
                let frame_count = data.len() / device_channels;

                // ── rubato resampled path (device rate ≠ source rate) ──
                // Pull source frames from the ring in the resampler's fixed
                // input chunk size, deinterleave to planar, process, then buffer
                // the (variable-length) planar output interleaved. Drain exactly
                // `frame_count` frames into `data`; any surplus carries to the
                // next callback via `out_buf`. The resampler keeps its own phase,
                // so we advance the ring's read_pos only by the source frames
                // actually consumed.
                if let Some(resampler) = resampler.as_mut() {
                    let needed_out_samples = frame_count * 2;
                    let mut src_consumed_this_cb: u64 = 0;
                    let mut src_pos_u64 = ring_ref.inner.lock().read_pos;
                    // Guard against a pathological resampler that never produces
                    // output (e.g. stuck warming up on zero input): bound the
                    // number of process() calls per callback so we can't starve
                    // the realtime thread.
                    let mut process_calls = 0u32;
                    while out_buf.len() < needed_out_samples && process_calls < 64 {
                        let need = resampler.input_frames_next();
                        // Fill in_planar up to `need` frames from the ring.
                        while in_filled < need {
                            let want = need - in_filled;
                            if resample_scratch.len() < want {
                                resample_scratch.resize(want, [0.0; 2]);
                            }
                            let n = ring_ref.read_block(src_pos_u64, &mut resample_scratch[..want]);
                            if n == 0 {
                                break; // ring underrun
                            }
                            src_pos_u64 += n as u64;
                            src_consumed_this_cb += n as u64;
                            for i in 0..n {
                                in_planar[0].push(resample_scratch[i][0]);
                                in_planar[1].push(resample_scratch[i][1]);
                            }
                            in_filled += n;
                        }
                        // Underrun: zero-pad the remainder of the chunk so the
                        // resampler can still advance/flush. We do NOT advance
                        // src_pos for padded frames (there's no source there).
                        if in_filled < need {
                            in_planar[0].resize(need, 0.0);
                            in_planar[1].resize(need, 0.0);
                            in_filled = need;
                        }
                        process_calls += 1;
                        match resampler.process(&in_planar, None) {
                            Ok(out_planar) => {
                                in_planar[0].clear();
                                in_planar[1].clear();
                                in_filled = 0;
                                let n_out = out_planar[0].len().min(out_planar[1].len());
                                for i in 0..n_out {
                                    out_buf.push_back(out_planar[0][i]);
                                    out_buf.push_back(out_planar[1][i]);
                                }
                            }
                            Err(e) => {
                                log::warn!("rubato process error: {}", e);
                                in_planar[0].clear();
                                in_planar[1].clear();
                                in_filled = 0;
                                break;
                            }
                        }
                    }

                    // Drain exactly frame_count stereo frames into the device
                    // buffer. If the resampler underproduced (warm-up / sustained
                    // underrun), the missing samples are silence — preferable to
                    // repeating stale audio.
                    for out_frame in 0..frame_count {
                        let l = out_buf.pop_front().unwrap_or(0.0) * vol;
                        let r = out_buf.pop_front().unwrap_or(0.0) * vol;
                        data[out_frame * device_channels] = l;
                        if device_channels >= 2 {
                            data[out_frame * device_channels + 1] = r;
                        }
                    }
                    ring_ref.advance_read(src_consumed_this_cb);
                    spectrum_feed.lock().feed_interleaved(data);
                    spectrum_wakeup_cb.signal();
                    return;
                }

                // ── direct / linear-interpolation path (rate ratio ≈ 1.0) ──
                let rate_ratio = ring_ref.sample_rate as f64 / device_sample_rate as f64;

                // Snapshot the read position once (single brief lock) instead of
                // locking the ring mutex on every output frame.
                let read_pos = {
                    let inner = ring_ref.inner.lock();
                    inner.read_pos as f64
                };

                // Contiguous range of source frames needed for this callback.
                // +1 covers the linear-interpolation right neighbor of the last sample.
                let src_start = read_pos.floor() as u64;
                let src_end = (read_pos + frame_count as f64 * rate_ratio).ceil() as u64 + 1;
                let needed = src_end.saturating_sub(src_start) as usize;
                if needed > src_buf.len() {
                    src_buf.resize(needed, [0.0; 2]);
                }

                // Single lock for the whole callback's source-frame read.
                let n_read = ring_ref.read_block(src_start, &mut src_buf[..needed]);

                let mut actual_src_consumed = 0.0f64;

                for out_frame in 0..frame_count {
                    let src_pos = read_pos + out_frame as f64 * rate_ratio;
                    let src_idx = src_pos as u64;
                    let frac = (src_pos - src_idx as f64) as f32;

                    let local0 = src_idx.saturating_sub(src_start) as usize;
                    let local1 = local0 + 1;

                    let ok0 = local0 < n_read;
                    if !ok0 {
                        for ch in 0..device_channels {
                            data[out_frame * device_channels + ch] = 0.0;
                        }
                        continue;
                    }

                    let [s0_l, s0_r] = src_buf[local0];
                    let ok1 = local1 < n_read;
                    let [s1_l, s1_r] = if ok1 { src_buf[local1] } else { [0.0; 2] };

                    actual_src_consumed = (out_frame as f64 + 1.0) * rate_ratio;

                    let w1 = frac;
                    let w0 = 1.0 - w1;

                    let left = if ok1 { s0_l * w0 + s1_l * w1 } else { s0_l };
                    let right = if ok1 { s0_r * w0 + s1_r * w1 } else { s0_r };

                    data[out_frame * device_channels] = left * vol;
                    if device_channels >= 2 {
                        data[out_frame * device_channels + 1] = right * vol;
                    }
                }

                // Feed output to spectrum analyzer
                spectrum_feed.lock().feed_interleaved(data);
                spectrum_wakeup_cb.signal();

                // Advance read_pos once (single lock + single notify) instead of
                // mutating it on every frame.
                ring_ref.advance_read(actual_src_consumed as u64);
            },
            |err| {
                log::error!("Audio output error: {}", err);
            },
            None,
        )?;

        stream.play()?;
        *playing.lock() = true;
        tracing::info!(
            "Audio output started: device={}Hz {}ch, ring={}Hz {}ch, ratio={:.4}",
            device_sample_rate, device_channels,
            ring.sample_rate, ring.channels,
            ring.sample_rate as f64 / device_sample_rate as f64,
        );

        Ok(Self {
            _host: host,
            _device: device,
            stream: SendStream(Some(stream)),
            playing,
            spectrum: spectrum_result,
            spectrum_wakeup,
        })
    }

    pub fn stop(&mut self) {
        self.spectrum_wakeup.request_shutdown();
        self.stream.0.take();
        *self.playing.lock() = false;
    }

    pub fn is_playing(&self) -> bool {
        *self.playing.lock()
    }
}
