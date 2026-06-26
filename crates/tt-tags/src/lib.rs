use std::path::Path;

use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use tt_common::SongMetadata;

/// Read metadata tags from an audio file using lofty 0.21.
/// Supports ID3v2 (MP3), Vorbis Comments (FLAC, Opus, Ogg Vorbis),
/// APE tags, MP4 (AAC/ALAC), and WAV.
pub fn read(path: &Path) -> anyhow::Result<SongMetadata> {
    let tagged_file = Probe::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open file for tags: {}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read tags: {}", e))?;

    let tag = tagged_file.primary_tag();
    let props = tagged_file.properties();
    let duration_ms = props.duration().as_millis() as u64;

    // ── text fields ──────────────────────────────────────────
    let title = tag
        .and_then(|t| t.title())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let artist = tag
        .and_then(|t| t.artist())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let album = tag
        .and_then(|t| t.album())
        .map(|s| s.to_string())
        .unwrap_or_default();

    // lofty 0.21 → Accessor::get_strings with ItemKey::AlbumArtist
    let album_artist = tag
        .and_then(|t| {
            t.get_strings(&lofty::tag::ItemKey::AlbumArtist)
                .next()
                .map(|s| s.to_string())
        })
        .unwrap_or_default();

    let year = tag.and_then(|t| t.year());
    let track = tag.and_then(|t| t.track());
    let genre = tag
        .and_then(|t| t.genre())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let comment = tag
        .and_then(|t| t.comment())
        .map(|s| s.to_string())
        .unwrap_or_default();

    // ── cover art ────────────────────────────────────────────
    let cover_art = tag.and_then(|t| {
        t.pictures()
            .first()
            .and_then(|p| {
                let mime_type = p.mime_type();
                let Some(mime) = mime_type.as_ref() else { return None };
                // Only return common web/browser-safe image types
                match mime.as_str() {
                    "image/jpeg" | "image/png" | "image/webp" | "image/bmp" => {
                        let data = p.data();
                        use base64::Engine;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(data);
                        Some(format!("data:{};base64,{}", mime, b64))
                    }
                    _ => None,
                }
            })
    });

    // ── replay gain ──────────────────────────────────────────
    let replay_gain = {
        let track_gain = tag.and_then(|t| {
            t.get_strings(&lofty::tag::ItemKey::ReplayGainTrackGain).next()
        });
        let track_peak = tag.and_then(|t| {
            t.get_strings(&lofty::tag::ItemKey::ReplayGainTrackPeak).next()
        });
        let album_gain = tag.and_then(|t| {
            t.get_strings(&lofty::tag::ItemKey::ReplayGainAlbumGain).next()
        });
        let album_peak = tag.and_then(|t| {
            t.get_strings(&lofty::tag::ItemKey::ReplayGainAlbumPeak).next()
        });

        if track_gain.is_some() || album_gain.is_some() {
            Some(tt_common::ReplayGainInfo {
                track_gain: track_gain
                    .and_then(|s| s.trim_end_matches(" dB").parse().ok())
                    .unwrap_or(0.0),
                track_peak: track_peak
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1.0),
                album_gain: album_gain
                    .and_then(|s| s.trim_end_matches(" dB").parse().ok())
                    .unwrap_or(0.0),
                album_peak: album_peak
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(1.0),
            })
        } else {
            None
        }
    };

    // ── audio properties ─────────────────────────────────────
    let metadata = SongMetadata {
        title,
        artist,
        album,
        album_artist,
        year,
        track,
        genre,
        comment,
        duration_ms: Some(duration_ms),
        bit_rate: props.audio_bitrate(),
        sample_rate: props.sample_rate(),
        channels: props.channels().map(|c| c as u16),
        bit_depth: props.bit_depth().map(|b| b as u16),
        cover_art,
        replay_gain,
    };

    Ok(metadata)
}

/// Write metadata tags to an audio file.
/// Only writes non-empty fields. Uses lofty 0.21 TagExt.
pub fn write(path: &Path, updates: &std::collections::HashMap<String, String>) -> anyhow::Result<()> {
    use lofty::prelude::*;
    use lofty::tag::ItemKey;

    let mut tagged_file = Probe::open(path)
        .map_err(|e| anyhow::anyhow!("Failed to open file: {}", e))?
        .read()
        .map_err(|e| anyhow::anyhow!("Failed to read tags: {}", e))?;

    let tag = tagged_file.primary_tag_mut()
        .ok_or_else(|| anyhow::anyhow!("No writable tag found"))?;

    for (key, value) in updates {
        match key.as_str() {
            "title" => {
                if value.is_empty() {
                    tag.remove_title();
                } else {
                    tag.set_title(value.clone());
                }
            }
            "artist" => {
                if value.is_empty() {
                    tag.remove_artist();
                } else {
                    tag.set_artist(value.clone());
                }
            }
            "album" => {
                if value.is_empty() {
                    tag.remove_album();
                } else {
                    tag.set_album(value.clone());
                }
            }
            "album_artist" => {
                tag.insert_text(ItemKey::AlbumArtist, value.clone());
            }
            "genre" => {
                if value.is_empty() {
                    tag.remove_genre();
                } else {
                    tag.set_genre(value.clone());
                }
            }
            "comment" => {
                if value.is_empty() {
                    tag.remove_comment();
                } else {
                    tag.set_comment(value.clone());
                }
            }
            "year" => {
                if let Ok(y) = value.parse::<u32>() {
                    tag.set_year(y);
                }
            }
            "track" => {
                if let Ok(t) = value.parse::<u32>() {
                    tag.set_track(t);
                }
            }
            _ => {}
        }
    }

    // Atomic write: save the modified tags to a temporary copy of the file in
    // the same directory, then rename it over the original. This prevents a
    // crash/power loss mid-write from leaving a half-written (corrupted) audio
    // file — the original stays intact until the rename, which is atomic on a
    // single filesystem. The temp name preserves the original extension so
    // lofty's format detection still works when writing the tag back.
    let tmp_path = atomic_temp_path(path);
    if let Err(e) = std::fs::copy(path, &tmp_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(anyhow::anyhow!("Failed to stage temp copy: {}", e));
    }

    if let Err(e) = tag.save_to_path(&tmp_path, lofty::config::WriteOptions::default()) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(anyhow::anyhow!("Failed to save tags: {}", e));
    }

    if let Err(e) = std::fs::rename(&tmp_path, path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(anyhow::anyhow!("Failed to replace original file: {}", e));
    }

    Ok(())
}

/// Build a temp-file path next to `path` (same directory → same filesystem, so
/// the later `rename` is atomic) that keeps the original extension so lofty
/// can still detect the container format when writing the tag back.
///
/// `song.mp3` → `song.tagtmp.mp3` in the same directory.
fn atomic_temp_path(path: &Path) -> std::path::PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("tt");
    let ext = path.extension().and_then(|s| s.to_str());
    let name = match ext {
        Some(ext) => format!("{}.tagtmp.{}", stem, ext),
        None => format!("{}.tagtmp", stem),
    };
    parent.join(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_temp_path_preserves_extension() {
        let p = atomic_temp_path(Path::new("/tmp/song.mp3"));
        assert_eq!(p.file_name().unwrap(), "song.tagtmp.mp3");
        assert_eq!(p.extension().unwrap(), "mp3");

        let p2 = atomic_temp_path(Path::new("noext"));
        assert_eq!(p2.file_name().unwrap(), "noext.tagtmp");
    }
}
