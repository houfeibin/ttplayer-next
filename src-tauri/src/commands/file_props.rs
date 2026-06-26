use serde::Serialize;
use std::path::Path;
use tt_common::AudioFormat;
use tt_core::codecs::CodecRegistry;
use tt_tags::read as read_tags;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProperties {
    // 基本信息
    pub file_name: String,
    pub file_path: String,
    pub file_size: u64,
    pub file_size_str: String,
    pub format: String,
    pub format_ext: String,

    // 音频参数
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub bit_depth: Option<u16>,
    pub bitrate: Option<u32>,       // kbps
    pub duration_ms: Option<u64>,
    pub duration_str: Option<String>,

    // 标签信息
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<u32>,
    pub track: Option<u32>,
    pub genre: Option<String>,
    pub comment: Option<String>,
    pub has_cover: bool,
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

fn format_duration(ms: u64) -> String {
    let total_sec = ms / 1000;
    let min = total_sec / 60;
    let sec = total_sec % 60;
    if min >= 60 {
        let hour = min / 60;
        let min = min % 60;
        format!("{}:{:02}:{:02}", hour, min, sec)
    } else {
        format!("{}:{:02}", min, sec)
    }
}

fn format_name(fmt: &AudioFormat) -> &'static str {
    match fmt {
        AudioFormat::Flac => "FLAC",
        AudioFormat::Mp3 => "MP3",
        AudioFormat::Aac => "AAC",
        AudioFormat::Vorbis => "OGG Vorbis",
        AudioFormat::Opus => "Opus",
        AudioFormat::Wav => "WAV",
        AudioFormat::Wma => "WMA",
        AudioFormat::Ape => "APE",
        AudioFormat::Tak => "TAK",
        AudioFormat::Mpc => "Musepack",
        AudioFormat::Ac3 => "AC-3",
        AudioFormat::Mod => "Tracker Module",
        AudioFormat::Unknown => "Unknown",
    }
}

#[tauri::command]
pub async fn file_get_properties(path: String) -> Result<FileProperties, String> {
    let p = Path::new(&path);

    // File info
    let meta = std::fs::metadata(p).map_err(|e| e.to_string())?;
    let file_size = meta.len();
    let ext = p.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let format = CodecRegistry::detect_format(p);

    // Read tags
    let tags = read_tags(p).ok();

    // Probe decoder for audio params
    let registry = CodecRegistry::with_defaults();
    let (sample_rate, channels, bit_depth, bitrate, duration_ms) =
        if let Some(decoder) = registry.probe(p) {
            match decoder.open(p) {
                Ok(instance) => {
                    let sr = Some(instance.sample_rate());
                    let ch = Some(instance.channels());
                    let dur = instance.duration_ms();
                    (sr, ch, None, None, dur)
                }
                Err(_) => (None, None, None, None, None),
            }
        } else {
            (None, None, None, None, None)
        };

    let props = FileProperties {
        file_name: p.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string(),
        file_path: path,
        file_size,
        file_size_str: format_file_size(file_size),
        format: format_name(&format).to_string(),
        format_ext: ext,

        sample_rate,
        channels,
        bit_depth,
        bitrate,
        duration_ms,
        duration_str: duration_ms.map(format_duration),

        title: tags.as_ref().and_then(|t| if t.title.is_empty() { None } else { Some(t.title.clone()) }),
        artist: tags.as_ref().and_then(|t| if t.artist.is_empty() { None } else { Some(t.artist.clone()) }),
        album: tags.as_ref().and_then(|t| if t.album.is_empty() { None } else { Some(t.album.clone()) }),
        album_artist: tags.as_ref().and_then(|t| if t.album_artist.is_empty() { None } else { Some(t.album_artist.clone()) }),
        year: tags.as_ref().and_then(|t| t.year),
        track: tags.as_ref().and_then(|t| t.track),
        genre: tags.as_ref().and_then(|t| if t.genre.is_empty() { None } else { Some(t.genre.clone()) }),
        comment: tags.as_ref().and_then(|t| if t.comment.is_empty() { None } else { Some(t.comment.clone()) }),
        has_cover: tags.as_ref().map(|t| t.cover_art.is_some()).unwrap_or(false),
    };

    Ok(props)
}
