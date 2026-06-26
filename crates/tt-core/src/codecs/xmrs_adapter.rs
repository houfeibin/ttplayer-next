//! xmrs-based tracker module decoder (MOD/XM/S3M/IT)
//!
//! xmrs parses tracker files into a DAW-agnostic data model (`Module`).
//! Full audio rendering of the patterns+dsp is deferred; this decoder
//! provides metadata extraction and sample access for Phase 2.

use std::path::Path;

use xmrs::prelude::*;

use crate::buffer::AudioBuffer;
use crate::codecs::{AudioDecoder, DecoderInstance, ProbeResult};
use tt_common::SongMetadata;

pub struct XmrsDecoder;

impl AudioDecoder for XmrsDecoder {
    fn name(&self) -> &'static str {
        "xmrs"
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["mod", "xm", "s3m", "it", "dw"]
    }
    fn priority(&self) -> u8 {
        8
    }

    fn probe(&self, magic: &[u8], extension: &str) -> ProbeResult {
        match extension {
            "mod" if magic.len() >= 4 && &magic[0..4] == b"M.K." => ProbeResult::Match,
            "mod" if magic.len() >= 20 => ProbeResult::Maybe,
            "xm" if magic.len() >= 17 && &magic[0..17] == b"Extended Module: " => {
                ProbeResult::Match
            }
            "s3m" if magic.len() >= 44 => ProbeResult::Maybe,
            "it" if magic.len() >= 4 && &magic[0..4] == b"IMPM" => ProbeResult::Match,
            "dw" => ProbeResult::Maybe,
            _ => ProbeResult::No,
        }
    }

    fn open(&self, path: &Path) -> anyhow::Result<Box<dyn DecoderInstance>> {
        let data = std::fs::read(path)?;

        let module = Module::load(&data)
            .map_err(|e| anyhow::anyhow!("xmrs import error: {e}"))?;
        let title = module.name.clone();
        let origin = format!("{:?}", module.origin);

        // Extract samples from instruments
        let sample_list: Vec<Vec<f32>> = module
            .instrument
            .iter()
            .filter_map(|instr| match &instr.instr_type {
                InstrumentType::Default(default_instr) => {
                    // Pick the first non-None sample
                    default_instr.sample.iter().find_map(|s| s.as_ref())
                }
                _ => None,
            })
            .filter_map(|sample| sample.data.as_ref().map(sample_data_to_f32))
            .collect();

        let has_samples = !sample_list.is_empty();
        let sample_rate = 44100u32;
        let total_sample_duration = if has_samples {
            let max_frames = sample_list.iter().map(|s| s.len()).max().unwrap_or(0);
            (max_frames as f64 * 1000.0 / sample_rate as f64) as u64
        } else {
            0
        };

        Ok(Box::new(XmrsInstance {
            title,
            origin,
            samples: sample_list,
            sample_rate,
            channels: 2,
            duration_ms: Some(total_sample_duration),
        }))
    }
}

struct XmrsInstance {
    title: String,
    origin: String,
    samples: Vec<Vec<f32>>,
    sample_rate: u32,
    channels: u16,
    duration_ms: Option<u64>,
}

impl DecoderInstance for XmrsInstance {
    fn decode(&mut self) -> anyhow::Result<Option<AudioBuffer>> {
        if self.samples.is_empty() {
            return Ok(Some(AudioBuffer::new(
                self.channels, 0, self.sample_rate,
            )));
        }

        // Return the longest sample as initial decode for verification
        let longest_idx = self
            .samples
            .iter()
            .enumerate()
            .max_by_key(|(_, data)| data.len())
            .map(|(i, _)| i);

        if let Some(idx) = longest_idx {
            let mono_data = self.samples[idx].clone();
            let frames = mono_data.len();
            let mut buffer = AudioBuffer::new(self.channels, frames, self.sample_rate);
            for ch_data in buffer.data.iter_mut() {
                ch_data.copy_from_slice(&mono_data);
            }
            buffer.frames = frames;
            self.samples.clear();
            Ok(Some(buffer))
        } else {
            Ok(None)
        }
    }

    fn seek(&mut self, _frame: u64) -> anyhow::Result<()> {
        anyhow::bail!("seek not supported for tracker modules")
    }

    fn total_frames(&self) -> Option<u64> {
        None
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn metadata(&self) -> Option<SongMetadata> {
        Some(SongMetadata {
            title: self.title.clone(),
            comment: self.origin.clone(),
            ..Default::default()
        })
    }
    fn duration_ms(&self) -> Option<u64> {
        self.duration_ms
    }
}

/// Convert xmrs SampleDataType to f32
fn sample_data_to_f32(data: &SampleDataType) -> Vec<f32> {
    match data {
        SampleDataType::Mono8(samples) => {
            samples.iter().map(|&s| s as f32 / 128.0).collect()
        }
        SampleDataType::Mono16(samples) => {
            samples.iter().map(|&s| s as f32 / 32768.0).collect()
        }
        SampleDataType::Stereo8(samples) => {
            // Return left channel only for simplicity
            samples.iter().step_by(2).map(|&s| s as f32 / 128.0).collect()
        }
        SampleDataType::Stereo16(samples) => {
            samples.iter().step_by(2).map(|&s| s as f32 / 32768.0).collect()
        }
        SampleDataType::StereoFloat(samples) => {
            samples.iter().step_by(2).copied().collect()
        }
    }
}
