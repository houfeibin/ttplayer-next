pub mod parser;
pub mod provider;
pub mod timing;

pub use parser::{LrcFile, LrcLine, LrcMetadata, lrcfile_to_string, write_lrc_file, lrc_path_for_audio};
pub use provider::{LyricsProviderRegistry, LyricSearchResult, OPENAPI_BASE_URL};
pub use timing::LyricsEngine;
