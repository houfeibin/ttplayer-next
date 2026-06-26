use tt_common::{AudioFormat, SongMetadata};
use crate::buffer::AudioBuffer;
use std::path::Path;

pub mod symphonia_adapter;
pub mod xmrs_adapter;
pub mod ape_adapter;
pub mod ac3_adapter;
pub mod stub_codecs;

/// Probe result for format detection
#[derive(Debug, PartialEq, Eq)]
pub enum ProbeResult {
    Match,
    Maybe,
    No,
}

/// Audio decoder factory
pub trait AudioDecoder: Send + Sync {
    fn name(&self) -> &'static str;
    fn probe(&self, magic: &[u8], extension: &str) -> ProbeResult;
    fn open(&self, path: &Path) -> anyhow::Result<Box<dyn DecoderInstance>>;
    fn extensions(&self) -> &'static [&'static str];
    fn priority(&self) -> u8 {
        0
    }
}

/// Decoder instance (owns the decode state)
pub trait DecoderInstance: Send {
    fn decode(&mut self) -> anyhow::Result<Option<AudioBuffer>>;
    fn seek(&mut self, frame: u64) -> anyhow::Result<()>;
    fn total_frames(&self) -> Option<u64>;
    fn sample_rate(&self) -> u32;
    fn channels(&self) -> u16;
    fn metadata(&self) -> Option<SongMetadata>;
    fn duration_ms(&self) -> Option<u64>;
}

/// Registry of all available audio decoders
pub struct CodecRegistry {
    decoders: Vec<Box<dyn AudioDecoder>>,
}

impl CodecRegistry {
    pub fn new() -> Self {
        Self {
            decoders: Vec::new(),
        }
    }

    pub fn with_defaults() -> Self {
        let mut reg = Self::new();

        // Priority: ape-decoder(15) > ac3-adapter(12) > symphonia(10) > xmrs(8) > stubs(1)
        // AC-3/E-AC-3 -> ac3_adapter; DTS has no decoder and falls back to dts_stub.
        reg.register(Box::new(ape_adapter::ApeDecoderFactory));
        reg.register(Box::new(ac3_adapter::Ac3DecoderFactory));
        reg.register(Box::new(symphonia_adapter::SymphoniaDecoder));
        reg.register(Box::new(xmrs_adapter::XmrsDecoder));
        reg.register(stub_codecs::tak_stub());
        reg.register(stub_codecs::wma_stub());
        reg.register(stub_codecs::mpc_stub());
        reg.register(stub_codecs::dts_stub());

        reg
    }

    pub fn register(&mut self, decoder: Box<dyn AudioDecoder>) {
        self.decoders.push(decoder);
    }

    /// Probe all decoders, return the best match
    pub fn probe(&self, path: &Path) -> Option<&dyn AudioDecoder> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let ext_lower = ext.to_lowercase();

        // Read magic bytes
        let mut magic = [0u8; 64];
        let magic_len = std::fs::File::open(path)
            .ok()
            .and_then(|mut f| {
                use std::io::Read;
                f.read(&mut magic).ok()
            })
            .unwrap_or(0);

        let magic = &magic[..magic_len];

        // Try exact matches first, then maybe matches
        let mut maybe_matches: Vec<(&dyn AudioDecoder, ProbeResult)> = Vec::new();

        for decoder in &self.decoders {
            let result = decoder.probe(magic, &ext_lower);
            match result {
                ProbeResult::Match => {
                    maybe_matches.push((decoder.as_ref(), ProbeResult::Match));
                }
                ProbeResult::Maybe => {
                    maybe_matches.push((decoder.as_ref(), ProbeResult::Maybe));
                }
                ProbeResult::No => {}
            }
        }

        // Sort: Matches first, then by priority descending
        maybe_matches.sort_by(|a, b| {
            let a_is_match = if a.1 == ProbeResult::Match { 0 } else { 1 };
            let b_is_match = if b.1 == ProbeResult::Match { 0 } else { 1 };
            a_is_match
                .cmp(&b_is_match)
                .then_with(|| b.0.priority().cmp(&a.0.priority()))
        });

        maybe_matches.first().map(|(d, _)| *d)
    }

    /// Map extension to AudioFormat
    pub fn detect_format(path: &Path) -> AudioFormat {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        match ext.as_str() {
            "flac" => AudioFormat::Flac,
            "mp3" => AudioFormat::Mp3,
            "aac" | "m4a" | "m4b" | "alac" => AudioFormat::Aac,
            "ogg" => AudioFormat::Vorbis,
            "opus" => AudioFormat::Opus,
            "wav" => AudioFormat::Wav,
            "wma" => AudioFormat::Wma,
            "ape" => AudioFormat::Ape,
            "tak" => AudioFormat::Tak,
            "mpc" | "mp+" | "mpp" => AudioFormat::Mpc,
            "ac3" | "dts" | "eac3" | "a52" => AudioFormat::Ac3,
            "mod" | "xm" | "s3m" | "it" | "dw" => AudioFormat::Mod,
            _ => AudioFormat::Unknown,
        }
    }
}
