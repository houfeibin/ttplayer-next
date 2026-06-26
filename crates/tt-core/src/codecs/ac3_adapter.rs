//! AC-3 (Dolby Digital) decoder adapter using `oxideav-ac3`.
//!
//! Uses the oxideav codec pipeline: parse syncframes → send_packet → receive_frame.
//! Output: 1536 samples/frame per channel, 48kHz, S16LE → f32 interleaved.
//!
//! Also handles E-AC-3 (Dolby Digital Plus) via the same decoder.

use std::fs;
use std::path::Path;

use oxideav_ac3::{syncinfo, decoder};
use oxideav_core::{CodecId, CodecParameters, Packet, TimeBase, Frame};
use oxideav_core::packet::PacketFlags;

use crate::buffer::AudioBuffer;
use crate::codecs::{AudioDecoder, DecoderInstance, ProbeResult};
use tt_common::SongMetadata;

pub struct Ac3DecoderFactory;

impl AudioDecoder for Ac3DecoderFactory {
    fn name(&self) -> &'static str {
        "AC-3 / E-AC-3"
    }

    fn probe(&self, magic: &[u8], extension: &str) -> ProbeResult {
        // AC-3 syncword 0x0B77 at offset 0 (big-endian in file)
        if magic.len() >= 2 && magic[0] == 0x0B && magic[1] == 0x77 {
            return ProbeResult::Match;
        }
        match extension {
            "ac3" | "eac3" | "a52" => ProbeResult::Maybe,
            _ => ProbeResult::No,
        }
    }

    fn open(&self, path: &Path) -> anyhow::Result<Box<dyn DecoderInstance>> {
        let data = fs::read(path)?;
        Ok(Box::new(Ac3Instance::new(data)?))
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["ac3", "eac3", "a52"]
    }

    fn priority(&self) -> u8 {
        12 // between ape-decoder (15) and symphonia (10)
    }
}

struct Ac3Instance {
    data: Vec<u8>,
    offset: usize,
    sample_rate: u32,
    channels: u16,
    total_frames: u64,
    frame_index: u64,
    /// Reused across all frames instead of creating a fresh decoder per frame.
    decoder: Box<dyn oxideav_core::Decoder>,
}

impl Ac3Instance {
    fn new(raw_data: Vec<u8>) -> anyhow::Result<Self> {
        // Probe: find valid syncinfo at offset 0
        if raw_data.len() < 5 {
            anyhow::bail!("AC-3 file too small ({})", raw_data.len());
        }

        let si = syncinfo::parse(&raw_data)?;
        let total_frames = {
            let len = raw_data.len();
            let frame_len = si.frame_length as usize;
            if frame_len == 0 {
                anyhow::bail!("zero frame length in syncinfo");
            }
            (len / frame_len) as u64
        };

        // Create the decoder once and reuse it for every frame.
        let codec_id = CodecId::new("ac3");
        let mut params = CodecParameters::audio(codec_id);
        params.channels = Some(2);
        let dec = decoder::make_decoder(&params)?;

        Ok(Self {
            data: raw_data,
            offset: 0,
            sample_rate: si.sample_rate,
            channels: 2, // AC-3 decoder outputs stereo by default; actual depends on stream
            total_frames,
            frame_index: 0,
            decoder: dec,
        })
    }

    fn decode_one_frame(&mut self) -> anyhow::Result<Option<Vec<i16>>> {
        if self.offset >= self.data.len() {
            return Ok(None);
        }

        let si = syncinfo::parse(&self.data[self.offset..])?;
        let frame_bytes = si.frame_length as usize;
        if self.offset + frame_bytes > self.data.len() {
            return Ok(None); // partial frame at end
        }

        let frame_data = self.data[self.offset..self.offset + frame_bytes].to_vec();
        self.offset += frame_bytes;
        self.frame_index += 1;

        // Reuse the persistent decoder — no per-frame allocation.
        let pkt = Packet {
            data: frame_data,
            pts: None,
            dts: None,
            duration: None,
            stream_index: 0,
            time_base: TimeBase::new(1, 48_000),
            flags: PacketFlags::default(),
        };

        self.decoder.send_packet(&pkt)?;

        let mut all_samples: Vec<i16> = Vec::new();

        loop {
            match self.decoder.receive_frame() {
                Ok(Frame::Audio(af)) => {
                    // data is Vec<Vec<u8>> — one plane of S16LE interleaved
                    for plane in &af.data {
                        for chunk in plane.chunks_exact(2) {
                            let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                            all_samples.push(s);
                        }
                    }
                    if af.data.is_empty() {
                        break;
                    }
                }
                Ok(_) => break,
                Err(e) => {
                    // oxideav-core errors have variant-specific handling
                    let msg = format!("{}", e);
                    if msg.contains("more data") || msg.contains("again") {
                        break;
                    }
                    return Err(anyhow::anyhow!("AC-3 decode error: {msg}"));
                }
            }
        }

        // Don't flush — reusing the decoder across frames, flushing would
        // discard internal state needed for the next frame. Instead just drain
        // any remaining frames from this packet.
        loop {
            match self.decoder.receive_frame() {
                Ok(Frame::Audio(af)) => {
                    for plane in &af.data {
                        for chunk in plane.chunks_exact(2) {
                            let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                            all_samples.push(s);
                        }
                    }
                    if af.data.is_empty() {
                        break;
                    }
                }
                _ => break,
            }
        }

        if all_samples.is_empty() {
            Ok(None)
        } else {
            Ok(Some(all_samples))
        }
    }
}

impl DecoderInstance for Ac3Instance {
    fn decode(&mut self) -> anyhow::Result<Option<AudioBuffer>> {
        let samples = match self.decode_one_frame()? {
            Some(s) => s,
            None => return Ok(None),
        };

        let interleaved: Vec<f32> = samples.iter().map(|&s| s as f32 / 32768.0).collect();

        Ok(Some(AudioBuffer::from_interleaved(
            &interleaved,
            self.channels,
            self.sample_rate,
        )))
    }

    fn seek(&mut self, _frame: u64) -> anyhow::Result<()> {
        Ok(())
    }

    fn total_frames(&self) -> Option<u64> {
        // Estimate: total_frames × 1536 samples/frame
        Some(self.total_frames * 1536)
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn metadata(&self) -> Option<SongMetadata> {
        None
    }

    fn duration_ms(&self) -> Option<u64> {
        let total = self.total_frames * 1536;
        Some(total * 1000 / self.sample_rate as u64)
    }
}
