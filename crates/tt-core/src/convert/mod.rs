use std::path::{Path, PathBuf};

use crate::codecs::CodecRegistry;

pub mod wav_encoder;

/// Output format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Wav,
}

impl OutputFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "wav" => Some(Self::Wav),
            _ => None,
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Self::Wav => "wav",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Wav => "WAV (PCM)",
        }
    }

    /// All supported output formats
    pub fn all() -> &'static [OutputFormat] {
        &[Self::Wav]
    }
}

/// Conversion options
#[derive(Debug, Clone)]
pub struct ConvertOptions {
    pub output_format: OutputFormat,
    pub output_dir: Option<PathBuf>,
    pub bit_depth: u16,      // 16 or 24 for WAV
    pub preserve_tags: bool,
}

impl Default for ConvertOptions {
    fn default() -> Self {
        Self {
            output_format: OutputFormat::Wav,
            output_dir: None,
            bit_depth: 16,
            preserve_tags: true,
        }
    }
}

/// Progress callback type
pub type ProgressCallback = Box<dyn Fn(f32) + Send + Sync>;

/// Convert a single audio file
pub fn convert_file(
    input: &Path,
    options: &ConvertOptions,
    progress: Option<&ProgressCallback>,
) -> anyhow::Result<PathBuf> {
    // Determine output path
    let output = if let Some(dir) = &options.output_dir {
        let stem = input.file_stem().unwrap_or_default();
        dir.join(format!("{}.{}", stem.to_string_lossy(), options.output_format.extension()))
    } else {
        let parent = input.parent().unwrap_or(Path::new("."));
        let stem = input.file_stem().unwrap_or_default();
        parent.join(format!("{}.{}", stem.to_string_lossy(), options.output_format.extension()))
    };

    // Decode the entire file using the codec registry
    let registry = CodecRegistry::with_defaults();
    let decoder = registry.probe(input)
        .ok_or_else(|| anyhow::anyhow!("Unsupported format: {:?}", input))?;
    let mut instance = decoder.open(input)?;

    let sample_rate = instance.sample_rate();
    let channels = instance.channels();

    // Decode all frames into a buffer
    let mut all_samples: Vec<f32> = Vec::new();
    let total_frames = instance.total_frames().unwrap_or(0);

    loop {
        match instance.decode()? {
            Some(buffer) => {
                // Interleave the planar buffer
                let interleaved = buffer.interleaved();
                all_samples.extend_from_slice(&interleaved);

                // Report progress
                if let Some(cb) = progress {
                    if total_frames > 0 {
                        let current_frames = all_samples.len() / channels as usize;
                        let p = (current_frames as f32 / total_frames as f32).clamp(0.0, 1.0);
                        cb(p);
                    }
                }
            }
            None => break,
        }
    }

    if all_samples.is_empty() {
        anyhow::bail!("No audio data decoded from {:?}", input);
    }

    // Write output
    match options.output_format {
        OutputFormat::Wav => {
            wav_encoder::write_wav(&output, &all_samples, channels, sample_rate, options.bit_depth)?;
        }
    }

    // Copy tags if requested
    if options.preserve_tags {
        if let Ok(tags) = tt_tags::read(input) {
            let mut updates = std::collections::HashMap::new();
            if !tags.title.is_empty() { updates.insert("title".to_string(), tags.title); }
            if !tags.artist.is_empty() { updates.insert("artist".to_string(), tags.artist); }
            if !tags.album.is_empty() { updates.insert("album".to_string(), tags.album); }
            if !tags.album_artist.is_empty() { updates.insert("album_artist".to_string(), tags.album_artist); }
            if !tags.genre.is_empty() { updates.insert("genre".to_string(), tags.genre); }
            if !tags.comment.is_empty() { updates.insert("comment".to_string(), tags.comment); }
            if let Some(year) = tags.year { updates.insert("year".to_string(), year.to_string()); }
            if let Some(track) = tags.track { updates.insert("track".to_string(), track.to_string()); }

            if !updates.is_empty() {
                let _ = tt_tags::write(&output, &updates);
            }
        }
    }

    Ok(output)
}

/// Get supported input extensions
pub fn supported_input_extensions() -> Vec<&'static str> {
    vec![
        "mp3", "flac", "wav", "aac", "m4a", "ogg", "opus",
        "wma", "ape", "ac3", "eac3",
        "mod", "xm", "s3m", "it",
    ]
}
