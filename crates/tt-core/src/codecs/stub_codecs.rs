//! Stub decoders for formats not yet supported by Rust crates.
//! These detect the format but `open()` returns a descriptive error.

use std::path::Path;

use crate::codecs::{AudioDecoder, DecoderInstance, ProbeResult};

// Re-export for use in stub
// use crate::codecs::DecoderInstance as _;

/// Stub decoder that can detect a format but cannot decode.
struct FormatStub {
    name: &'static str,
    extensions: &'static [&'static str],
    priority: u8,
    magic_pattern: Option<&'static [u8]>,
    message: &'static str,
}

impl AudioDecoder for FormatStub {
    fn name(&self) -> &'static str {
        self.name
    }
    fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }
    fn priority(&self) -> u8 {
        self.priority
    }

    fn probe(&self, magic: &[u8], extension: &str) -> ProbeResult {
        if self.extensions.contains(&extension) {
            if let Some(pat) = self.magic_pattern {
                if magic.starts_with(pat) {
                    ProbeResult::Match
                } else {
                    ProbeResult::Maybe
                }
            } else {
                ProbeResult::Maybe
            }
        } else {
            ProbeResult::No
        }
    }

    fn open(&self, _path: &Path) -> anyhow::Result<Box<dyn DecoderInstance>> {
        anyhow::bail!("{}", self.message)
    }
}

/// TAK decoder stub (no open-source decoder available)
pub fn tak_stub() -> Box<dyn AudioDecoder> {
    Box::new(FormatStub {
        name: "tak-stub",
        extensions: &["tak"],
        priority: 1,
        magic_pattern: Some(b"tBaK"),
        message: "TAK format: no open-source decoder available. TAK is a proprietary lossless codec by Thomas Becker.",
    })
}

/// WMA/ASF decoder stub (symphonia 0.6 doesn't support WMA codec yet)
pub fn wma_stub() -> Box<dyn AudioDecoder> {
    Box::new(FormatStub {
        name: "wma-stub",
        extensions: &["wma", "asf", "wmv"],
        priority: 1,
        magic_pattern: Some(b"\x30\x26\xB2\x75\x8E\x66\xCF\x11\xA6\xD9\x00\xAA\x00\x62\xCE\x6C"),
        message: "WMA/ASF format: no open-source decoder available yet. Use FFI with Windows Media Foundation.",
    })
}

/// MPC (Musepack) decoder stub
pub fn mpc_stub() -> Box<dyn AudioDecoder> {
    Box::new(FormatStub {
        name: "mpc-stub",
        extensions: &["mpc", "mp+", "mpp"],
        priority: 1,
        magic_pattern: Some(b"MP+"),
        message: "MPC (Musepack) format: no pure-Rust decoder available. Consider FFI with libmpcdec.",
    })
}

/// DTS decoder stub.
///
/// AC-3 / E-AC-3 are handled by `ac3_adapter::Ac3DecoderFactory` (priority 12);
/// only DTS remains unsupported and falls back to this stub.
pub fn dts_stub() -> Box<dyn AudioDecoder> {
    Box::new(FormatStub {
        name: "dts-stub",
        extensions: &["dts"],
        priority: 1,
        // DTS sync word: 0x7FFE 0x8001 (big-endian, core sync at start of stream)
        magic_pattern: Some(b"\x7F\xFE\x80\x01"),
        message: "DTS format: no pure-Rust decoder available yet. Consider FFI with libdca or libavcodec.",
    })
}
