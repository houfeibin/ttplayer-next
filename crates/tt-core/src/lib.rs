#![allow(clippy::arc_with_non_send_sync)]

pub mod codecs;
pub mod convert;
pub mod dsp;
pub mod buffer;
pub mod lyrics;
pub mod player;
pub mod output;
pub mod skin;

pub use buffer::AudioBuffer;
pub use codecs::{AudioDecoder, CodecRegistry, DecoderInstance};
pub use output::{AudioOutput, AtomicVolume};
pub use player::AudioPipeline;
pub use skin::{SkinManager, SkinInfo, SkinDefinition};
