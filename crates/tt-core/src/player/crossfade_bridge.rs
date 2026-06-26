use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::task;

use crate::codecs::CodecRegistry;
use crate::dsp::crossfade::{crossfade_channel, CrossfadeResampler, CrossfadeSender};

use super::{SharedCrossfadeRx, AudioPipeline};

impl AudioPipeline {
    /// Background thread that decodes the next track and sends samples via channel
    /// for mixing during crossfade.
    ///
    /// `conv` is an optional [`CrossfadeResampler`] that converts the next
    /// track's format to the current track's rate/channels before sending —
    /// this is what enables cross-format crossfade (e.g. 48 kHz FLAC → 44.1 kHz
    /// MP3). `None` means formats already match and samples pass through.
    pub(crate) fn bg_crossfade_decode(
        path: PathBuf,
        mut instance: Box<dyn crate::codecs::DecoderInstance>,
        tx: CrossfadeSender,
        mut conv: Option<CrossfadeResampler>,
    ) -> anyhow::Result<()> {
        let sample_rate = instance.sample_rate();
        let channels = instance.channels();

        tracing::info!(
            "CROSSFADE DECODE: {} | {}Hz {}ch{}",
            path.display(), sample_rate, channels,
            if conv.is_some() { " (resampling)" } else { "" },
        );

        let mut frames_sent: u64 = 0;
        loop {
            match instance.decode() {
                Ok(Some(decoded)) => {
                    let interleaved = decoded.interleaved();
                    let frames = decoded.frames;
                    // Run through the format converter when present; otherwise
                    // send the raw decoded samples unchanged.
                    let out = if let Some(c) = conv.as_mut() {
                        c.push(&interleaved)
                    } else {
                        interleaved
                    };
                    if out.is_empty() {
                        // Resampler buffered the chunk internally (waiting for a
                        // full input window); nothing to send yet.
                        frames_sent += frames as u64;
                        continue;
                    }
                    let out_frames = out.len() / channels.max(1) as usize;
                    if tx.send(
                        crate::dsp::crossfade::CrossfadeChunk { samples: out, frames: out_frames }
                    ).is_err() {
                        tracing::info!("Crossfade channel closed (receiver dropped)");
                        break;
                    }
                    frames_sent += frames as u64;
                }
                Ok(None) => {
                    // Flush any residual samples held in the resampler's delay
                    // lines before signaling EOF so the crossfade tail isn't
                    // truncated by the filter latency.
                    if let Some(c) = conv.as_mut() {
                        let tail = c.flush();
                        if !tail.is_empty() {
                            let _ = tx.send(crate::dsp::crossfade::CrossfadeChunk {
                                samples: tail,
                                frames: 0,
                            });
                        }
                    }
                    tracing::info!("CROSSFADE DECODE EOF: frames_sent={}", frames_sent);
                    // Send sentinel empty chunk to signal EOF
                    let _ = tx.send(crate::dsp::crossfade::CrossfadeChunk {
                        samples: Vec::new(),
                        frames: 0,
                    });
                    break;
                }
                Err(e) => {
                    tracing::error!("CROSSFADE DECODE ERR: {}", e);
                    break;
                }
            }
        }

        // Keep tx alive until function returns (drops on exit)
        drop(tx);
        Ok(())
    }

    /// Spawn a background thread that decodes the next track and feeds its
    /// samples into the crossfade channel.
    ///
    /// On success the receiver is stored into `crossfade_rx` and the
    /// `crossfade_pending` flag is set; returns `true`. On any failure
    /// (no decoder, open error) it logs and returns `false` without setting
    /// the flag, leaving the caller to decide whether to retry.
    pub(crate) fn spawn_next_track_decoder(
        next_path: PathBuf,
        cur_sample_rate: u32,
        cur_channels: u16,
        crossfade_rx: &SharedCrossfadeRx,
        crossfade_pending: &Arc<std::sync::atomic::AtomicBool>,
    ) -> bool {
        let registry = CodecRegistry::with_defaults();
        let decoder = match registry.probe(&next_path) {
            Some(d) => d,
            None => {
                tracing::warn!("CROSSFADE: no decoder for {:?}", next_path);
                return false;
            }
        };
        let instance = match decoder.open(&next_path) {
            Ok(i) => i,
            Err(e) => {
                tracing::error!("CROSSFADE: failed to open {:?}: {}", next_path, e);
                return false;
            }
        };

        // Cross-format crossfade: when the next track's rate/channels differ
        // from the current track's, build a streaming converter that resamples
        // (band-limited sinc via rubato) and channel-converts to the current
        // track's format before the samples reach the mixer. This replaces the
        // previous hard bail that left format-mismatched transitions gapless.
        let next_rate = instance.sample_rate();
        let next_ch = instance.channels();
        let conv = match CrossfadeResampler::new(
            next_rate, next_ch, cur_sample_rate, cur_channels,
        ) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(
                    "CROSSFADE: resampler init failed ({}Hz {}ch → {}Hz {}ch): {}; skipping",
                    next_rate, next_ch, cur_sample_rate, cur_channels, e,
                );
                return false;
            }
        };

        let (tx, rx) = crossfade_channel();
        *crossfade_rx.lock() = Some(rx);
        let cf_path = next_path.clone();
        task::spawn_blocking(move || {
            if let Err(e) = Self::bg_crossfade_decode(cf_path, instance, tx, conv) {
                tracing::error!("Crossfade decode error: {}", e);
            }
        });

        crossfade_pending.store(true, Ordering::Relaxed);
        tracing::info!("CROSSFADE: decoder started for {:?}", next_path);
        true
    }
}
