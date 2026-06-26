//! APE decoder adapter using `ape-decoder` crate's `decode_frame()` for streaming decode.
//!
//! Each APE frame is large (~73728 samples ≈ 1.67s). We decode one frame at a time
//! and return it in ~100ms chunks via an internal buffer.

use std::io::BufReader;
use std::path::Path;

use crate::buffer::AudioBuffer;
use crate::codecs::{AudioDecoder, DecoderInstance, ProbeResult};
use tt_common::SongMetadata;

/// ~100ms at 44100 Hz = 4410 frames
const CHUNK_FRAMES: usize = 4410;

pub struct ApeDecoderFactory;

impl AudioDecoder for ApeDecoderFactory {
    fn name(&self) -> &'static str { "ape-decoder" }
    fn extensions(&self) -> &'static [&'static str] { &["ape"] }
    fn priority(&self) -> u8 { 15 }

    fn probe(&self, magic: &[u8], extension: &str) -> ProbeResult {
        if extension != "ape" {
            return ProbeResult::No;
        }
        if magic.starts_with(b"MAC ") {
            ProbeResult::Match
        } else {
            ProbeResult::Maybe
        }
    }

    fn open(&self, path: &Path) -> anyhow::Result<Box<dyn DecoderInstance>> {
        let file = std::fs::File::open(path)?;
        let decoder = ape_decoder::ApeDecoder::new(BufReader::new(file))
            .map_err(|e| anyhow::anyhow!(e))?;

        let info = decoder.info();
        let sample_rate = info.sample_rate;
        let channels = info.channels;
        let bits = info.bits_per_sample;
        let total_frame_count = decoder.total_frames();
        let total_samples = info.total_samples;
        let duration_ms = info.duration_ms;

        tracing::info!(
            "APE: {} Hz, {} ch, {}-bit, {} frames, {} samples ({} ms) | compression {}",
            sample_rate, channels, bits, total_frame_count, total_samples, duration_ms,
            info.compression_level,
        );

        Ok(Box::new(ApeInstance {
            decoder,
            sample_rate,
            channels,
            duration_ms,
            total_samples,
            ch: channels as usize,
            bits,
            current_frame: 0,
            total_frame_count,
            // Chunk buffer: holds remaining f32 samples from current APE frame
            chunk_buf: Vec::new(),
            chunk_read_pos: 0,
            position_samples: 0,
        }))
    }
}

struct ApeInstance {
    decoder: ape_decoder::ApeDecoder<BufReader<std::fs::File>>,
    sample_rate: u32,
    channels: u16,
    duration_ms: u64,
    total_samples: u64,
    ch: usize,
    bits: u16,
    current_frame: u32,
    total_frame_count: u32,
    /// Buffer for the current APE frame's remaining samples (interleaved f32)
    chunk_buf: Vec<f32>,
    /// Read position within chunk_buf
    chunk_read_pos: usize,
    position_samples: u64,
}

impl ApeInstance {
    fn pcm_to_f32(&self, pcm: &[u8]) -> Vec<f32> {
        match self.bits {
            16 => pcm.chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                .collect(),
            24 => pcm.chunks_exact(3)
                .map(|c| {
                    let s = (c[2] as i32) << 24 | (c[1] as i32) << 16 | (c[0] as i32) << 8;
                    (s >> 8) as f32 / 8388608.0
                })
                .collect(),
            32 => pcm.chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2147483648.0)
                .collect(),
            _ => pcm.chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                .collect(),
        }
    }

    /// Fill chunk_buf with next APE frame's samples.
    /// Returns true if a frame was decoded, false if EOF.
    fn fill_chunk_buf(&mut self) -> anyhow::Result<bool> {
        while self.current_frame < self.total_frame_count {
            let frame_idx = self.current_frame;
            self.current_frame += 1;

            let pcm = match self.decoder.decode_frame(frame_idx) {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("APE decode_frame({frame_idx}) error: {e}, skipping");
                    continue;
                }
            };

            if pcm.is_empty() {
                continue;
            }

            self.chunk_buf = self.pcm_to_f32(&pcm);
            self.chunk_read_pos = 0;
            return Ok(true);
        }
        Ok(false) // EOF
    }
}

impl DecoderInstance for ApeInstance {
    fn decode(&mut self) -> anyhow::Result<Option<AudioBuffer>> {
        // If chunk buffer is exhausted, decode next APE frame
        if self.chunk_read_pos >= self.chunk_buf.len() {
            if !self.fill_chunk_buf()? {
                return Ok(None); // EOF
            }
        }

        // Return a ~100ms chunk from the buffer
        let chunk_samples = CHUNK_FRAMES * self.ch; // total f32 values for one chunk
        let remaining = self.chunk_buf.len() - self.chunk_read_pos;
        let to_read = remaining.min(chunk_samples);

        let slice = &self.chunk_buf[self.chunk_read_pos..self.chunk_read_pos + to_read];
        self.chunk_read_pos += to_read;

        let frames_returned = to_read / self.ch;
        self.position_samples += frames_returned as u64;

        // Log first chunk for diagnostics
        if self.position_samples <= frames_returned as u64 {
            tracing::info!(
                "APE first chunk: sr={} ch={} bits={} chunk_buf.len={} to_read={} frames={}",
                self.sample_rate, self.channels, self.bits,
                self.chunk_buf.len(), to_read, frames_returned,
            );
        }

        Ok(Some(AudioBuffer::from_interleaved(
            slice,
            self.channels,
            self.sample_rate,
        )))
    }

    fn seek(&mut self, sample: u64) -> anyhow::Result<()> {
        match self.decoder.seek(sample) {
            Ok(result) => {
                self.current_frame = result.frame_index;
                self.position_samples = sample;
                // Clear chunk buffer so next decode() reads from new position
                self.chunk_buf.clear();
                self.chunk_read_pos = 0;
                tracing::info!(
                    "APE seek: sample {} → frame {} (skip {})",
                    sample, result.frame_index, result.skip_samples
                );
                Ok(())
            }
            Err(e) => {
                tracing::warn!("APE seek failed: {e}");
                Ok(())
            }
        }
    }

    fn total_frames(&self) -> Option<u64> { Some(self.total_samples) }
    fn sample_rate(&self) -> u32 { self.sample_rate }
    fn channels(&self) -> u16 { self.channels }
    fn duration_ms(&self) -> Option<u64> { Some(self.duration_ms) }
    fn metadata(&self) -> Option<SongMetadata> { None }
}
