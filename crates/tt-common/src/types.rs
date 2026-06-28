use serde::{Deserialize, Serialize};

/// Supported audio formats
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioFormat {
    Flac,
    Mp3,
    Aac,
    Vorbis,
    Opus,
    Wav,
    Wma,
    Ape,
    Tak,
    Mpc,
    Ac3,
    Mod,
    Unknown,
}

/// Playback state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    Idle,
    Loading,
    Playing,
    Paused,
    Stopped,
    /// Playback failed (e.g. corrupt file, unsupported format, decode error).
    /// The frontend observes this and auto-skips to the next track.
    Error,
}

/// Playback modes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayMode {
    Single,       // 播完停止
    LoopOne,     // 单曲循环
    Sequential,   // 顺序播完停止
    Loop,         // 列表循环
    Random,       // 随机
}

/// Song metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongMetadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    #[serde(rename = "albumArtist")]
    pub album_artist: String,
    pub year: Option<u32>,
    pub track: Option<u32>,
    pub genre: String,
    pub comment: String,
    pub duration_ms: Option<u64>,
    #[serde(rename = "bitRate")]
    pub bit_rate: Option<u32>,
    #[serde(rename = "sampleRate")]
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    #[serde(rename = "bitDepth")]
    pub bit_depth: Option<u16>,
    /// Album cover (raw JPEG/PNG bytes, base64-encoded in JSON)
    #[serde(rename = "coverArt")]
    pub cover_art: Option<String>,

    /// ReplayGain (dB)
    #[serde(rename = "replayGain")]
    pub replay_gain: Option<ReplayGainInfo>,
}

impl Default for SongMetadata {
    fn default() -> Self {
        Self {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            album_artist: String::new(),
            year: None,
            track: None,
            genre: String::new(),
            comment: String::new(),
            duration_ms: None,
            bit_rate: None,
            sample_rate: None,
            channels: None,
            bit_depth: None,
            cover_art: None,
            replay_gain: None,
        }
    }
}

/// Track info returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub path: String,
    pub format: AudioFormat,
    pub metadata: SongMetadata,
    pub duration_ms: u64,
    #[serde(rename = "fileSize")]
    pub file_size: u64,
}

impl TrackInfo {
    pub fn from_path(path: &std::path::Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        let format = match ext.as_str() {
            "flac" => AudioFormat::Flac,
            "mp3" => AudioFormat::Mp3,
            "aac" | "m4a" => AudioFormat::Aac,
            "ogg" => AudioFormat::Vorbis,
            "opus" => AudioFormat::Opus,
            "wav" => AudioFormat::Wav,
            "wma" => AudioFormat::Wma,
            "ape" => AudioFormat::Ape,
            "tak" => AudioFormat::Tak,
            "mpc" => AudioFormat::Mpc,
            "ac3" | "dts" => AudioFormat::Ac3,
            "mod" | "s3m" | "xm" | "it" => AudioFormat::Mod,
            _ => AudioFormat::Unknown,
        };

        Self {
            path: path.to_string_lossy().to_string(),
            format,
            metadata: SongMetadata::default(),
            duration_ms: 0,
            file_size: 0,
        }
    }
}

/// ReplayGain info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayGainInfo {
    #[serde(rename = "trackGain")]
    pub track_gain: f32, // dB
    #[serde(rename = "trackPeak")]
    pub track_peak: f32, // linear 0.0-2.0
    #[serde(rename = "albumGain")]
    pub album_gain: f32,
    #[serde(rename = "albumPeak")]
    pub album_peak: f32,
}

/// Error kind for playback errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    NoFile,
    UnknownFormat,
    DecoderError,
    OutputError,
    SeekError,
    IoError,
    Cancelled,
}

/// A playback error with full context for logging and auto-skip decisions.
///
/// Stored in `AudioPipeline::last_error` and emitted to the frontend via the
/// `player-state-update` event payload so the UI can log details and trigger
/// an automatic skip to the next track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaybackError {
    pub kind: ErrorKind,
    pub message: String,
    /// Path of the track that failed (None if no track was involved).
    #[serde(rename = "trackPath")]
    pub track_path: Option<String>,
    /// Unix epoch timestamp (ms) when the error was recorded.
    #[serde(rename = "timestampMs")]
    pub timestamp_ms: u64,
}

impl PlaybackError {
    pub fn new(kind: ErrorKind, message: impl Into<String>, track_path: Option<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            track_path,
            timestamp_ms: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0),
        }
    }
}
