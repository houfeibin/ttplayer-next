/// LRC lyrics parser — handles standard, enhanced, and Karaoke LRC variants.
///
/// Format reference:
///   Standard LRC:  [mm:ss.xx]Lyrics text
///   Enhanced LRC:  [mm:ss.xx]<mm:ss.xx>word1 <mm:ss.xx>word2
///   Multiple tags: [00:10.00][01:30.00]Same text for two timestamps
///   Metadata:      [ti:Title] [ar:Artist] [al:Album] [offset:+/-ms]
use std::path::Path;
use serde::{Deserialize, Serialize};

/// Decode raw LRC file bytes to a UTF-8 string.
///
/// Strategy (matches common behavior of Chinese music apps):
/// 1. If the bytes start with a UTF-8 BOM, decode as UTF-8 directly.
/// 2. Otherwise, try UTF-8 first — if valid, use it.
/// 3. Fall back to chardetng detection, which identifies GBK/Big5/Shift-JIS.
fn decode_lrc_bytes(bytes: &[u8]) -> String {
    // Fast path: UTF-8 BOM or valid UTF-8
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(&bytes[3..]).into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    // Legacy encoding — detect and convert
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let encoding = detector.guess(None, true);
    let (decoded, _enc_name, had_errors) = encoding.decode(bytes);
    if had_errors {
        tracing::warn!(
            "LRC file decoded with errors; encoding detected as {}",
            encoding.name()
        );
    }
    decoded.into_owned()
}

/// Parsed LRC file with metadata and timed lines.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrcFile {
    pub metadata: LrcMetadata,
    pub lines: Vec<LrcLine>,
}

/// LRC metadata tags.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LrcMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// Global offset in milliseconds (from [offset:+/-ms] tag).
    pub offset_ms: i32,
}

/// A single timed lyric line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LrcLine {
    /// Start time in milliseconds.
    pub time_ms: u32,
    /// Lyric text (empty string for instrumental breaks).
    pub text: String,
    /// Enhanced/Karaoke word timings (None for standard LRC).
    pub words: Option<Vec<WordTiming>>,
}

/// Word-level timing for enhanced/Karaoke LRC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordTiming {
    /// Start time of this word (ms).
    pub time_ms: u32,
    /// The word text.
    pub text: String,
}

/// Parse an LRC file from a string. Handles CRLF, LF, and BOM.
pub fn parse_lrc(content: &str) -> LrcFile {
    let content = content.trim_start_matches('\u{FEFF}'); // strip BOM
    let mut metadata = LrcMetadata::default();
    let mut lines: Vec<LrcLine> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // Try metadata tags: [key:value]
        if line.starts_with('[') && !line.contains("][") {
            // Could be metadata or single timestamp line
            if let Some(end) = line.find(']') {
                let tag_content = &line[1..end];
                let _after_tag = &line[end + 1..];

                // Check if it's a metadata tag (key:value where key is not a number)
                if let Some(colon_pos) = tag_content.find(':') {
                    let key = &tag_content[..colon_pos];
                    let value = tag_content[colon_pos + 1..].trim();

                    match key {
                        "ti" => { metadata.title = Some(value.to_string()); continue; }
                        "ar" => { metadata.artist = Some(value.to_string()); continue; }
                        "al" => { metadata.album = Some(value.to_string()); continue; }
                        "offset" => {
                            if let Ok(offset) = value.parse::<i32>() {
                                metadata.offset_ms = offset;
                            }
                            continue;
                        }
                        _ => {}
                    }
                }

                // Not metadata — try as timestamp line
                if let Some(parsed) = parse_timestamp_line(line) {
                    lines.extend(parsed);
                }
            }
        } else if line.starts_with('[') {
            // Multiple timestamp tags: [00:10.00][01:30.00]Text
            if let Some(parsed) = parse_timestamp_line(line) {
                lines.extend(parsed);
            }
        }
        // Lines without [ are ignored (continuation of previous, etc.)
    }

    // Sort by time
    lines.sort_by_key(|l| l.time_ms);

    LrcFile { metadata, lines }
}

/// Parse an LRC line with one or more timestamp tags.
/// Returns one LrcLine per timestamp tag.
fn parse_timestamp_line(line: &str) -> Option<Vec<LrcLine>> {
    let mut timestamps: Vec<u32> = Vec::new();
    let mut rest = line;

    // Extract all [mm:ss.xx] or [mm:ss.xxx] tags
    while rest.starts_with('[') {
        if let Some(end) = rest.find(']') {
            let tag = &rest[1..end];
            if let Some(time_ms) = parse_timestamp(tag) {
                timestamps.push(time_ms);
                rest = &rest[end + 1..];
            } else {
                // Not a timestamp tag — stop parsing
                break;
            }
        } else {
            break;
        }
    }

    if timestamps.is_empty() {
        return None;
    }

    let text = rest.trim();
    let text_lower = text.to_lowercase();

    // Skip metadata lines that ended up here (e.g. [00:00.00][ver:v1.0])
    if text_lower.starts_with("[ver:") || text_lower.starts_with("[ve:") {
        return None;
    }

    // Parse enhanced/Karaoke word timings if present
    let words = if text.contains('<') {
        parse_enhanced_words(text)
    } else {
        None
    };

    let text_clean = if words.is_some() {
        // Strip <mm:ss.xx> tags from text for display
        strip_word_tags(text)
    } else {
        text.to_string()
    };

    let lines: Vec<LrcLine> = timestamps
        .into_iter()
        .map(|time_ms| LrcLine {
            time_ms,
            text: text_clean.clone(),
            words: words.clone(),
        })
        .collect();

    Some(lines)
}

/// Parse a timestamp string like "03:45.67" or "03:45.678" into milliseconds.
fn parse_timestamp(s: &str) -> Option<u32> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        return None;
    }
    let minutes: u32 = parts[0].parse().ok()?;
    let sec_parts: Vec<&str> = parts[1].splitn(2, '.').collect();
    let seconds: u32 = sec_parts[0].parse().ok()?;
    let centiseconds: u32 = if sec_parts.len() > 1 {
        // Pad or truncate to 3 digits (ms)
        let frac = sec_parts[1];
        if frac.len() >= 3 {
            frac[..3].parse().unwrap_or(0)
        } else if frac.len() == 2 {
            frac.parse::<u32>().unwrap_or(0) * 10
        } else if frac.len() == 1 {
            frac.parse::<u32>().unwrap_or(0) * 100
        } else {
            0
        }
    } else {
        0
    };

    Some(minutes * 60_000 + seconds * 1_000 + centiseconds)
}

/// Parse enhanced/Karaoke word timings: <mm:ss.xx>word <mm:ss.xx>word
fn parse_enhanced_words(text: &str) -> Option<Vec<WordTiming>> {
    let mut words = Vec::new();
    let mut remaining = text;

    while let Some(start) = remaining.find('<') {
        if let Some(end) = remaining[start + 1..].find('>') {
            let tag = &remaining[start + 1..start + 1 + end];
            if let Some(time_ms) = parse_timestamp(tag) {
                let after_tag = &remaining[start + 1 + end + 1..];
                // Word text extends to the next < or end of string
                let word_end = after_tag.find('<').unwrap_or(after_tag.len());
                let word_text = after_tag[..word_end].trim();
                if !word_text.is_empty() {
                    words.push(WordTiming {
                        time_ms,
                        text: word_text.to_string(),
                    });
                }
                remaining = &remaining[start + 1 + end + 1 + word_end..];
                continue;
            }
        }
        remaining = &remaining[start + 1..];
    }

    if words.is_empty() { None } else { Some(words) }
}

/// Strip <mm:ss.xx> word timing tags from text for plain display.
fn strip_word_tags(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            // Skip until '>'
            while let Some(nc) = chars.next() {
                if nc == '>' {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result.trim().to_string()
}

/// Try to read and parse an LRC file from the given path.
///
/// Handles files saved in UTF-8 (with or without BOM), and legacy
/// GBK / Big5 / Shift-JIS encodings commonly used by Chinese music
/// services. Returns `None` if the file cannot be read, decoded, or
/// contains no valid timed lines.
pub fn read_lrc_file(path: &Path) -> Option<LrcFile> {
    let bytes = std::fs::read(path).ok()?;
    let content = decode_lrc_bytes(&bytes);
    let lrc = parse_lrc(&content);
    if lrc.lines.is_empty() {
        None
    } else {
        Some(lrc)
    }
}

/// Serialize a parsed `LrcFile` back to the standard LRC text format.
///
/// Outputs metadata tags (`[ti:]`, `[ar:]`, `[al:]`, `[offset:]`) followed
/// by timed lines. Enhanced word timings are discarded — only the plain
/// display text is written, which is sufficient for round-tripping lyrics
/// from online sources to local cache files.
pub fn lrcfile_to_string(lrc: &LrcFile) -> String {
    let mut out = String::new();

    if let Some(ref t) = lrc.metadata.title {
        out.push_str(&format!("[ti:{}]\n", t));
    }
    if let Some(ref a) = lrc.metadata.artist {
        out.push_str(&format!("[ar:{}]\n", a));
    }
    if let Some(ref al) = lrc.metadata.album {
        out.push_str(&format!("[al:{}]\n", al));
    }
    if lrc.metadata.offset_ms != 0 {
        out.push_str(&format!("[offset:{}]\n", lrc.metadata.offset_ms));
    }

    for line in &lrc.lines {
        let ms = line.time_ms;
        let min = ms / 60_000;
        let sec = (ms % 60_000) / 1_000;
        let cs = (ms % 1_000) / 10;
        out.push_str(&format!("[{:02}:{:02}.{:02}]{}\n", min, sec, cs, line.text));
    }

    out
}

/// Write a parsed `LrcFile` to disk at the given path.
pub fn write_lrc_file(path: &Path, lrc: &LrcFile) -> std::io::Result<()> {
    let content = lrcfile_to_string(lrc);
    std::fs::write(path, content)
}

/// Build the expected LRC path for an audio file:
/// `{audio_dir}/{audio_stem}.lrc`
pub fn lrc_path_for_audio(audio_path: &Path) -> Option<std::path::PathBuf> {
    let dir = audio_path.parent()?;
    let stem = audio_path.file_stem()?.to_str()?;
    Some(dir.join(format!("{}.lrc", stem)))
}

/// Search for an LRC file matching the given audio file path.
/// Looks in the same directory with the same stem name.
pub fn find_lrc_for_audio(audio_path: &Path) -> Option<std::path::PathBuf> {
    let dir = audio_path.parent()?;
    let stem = audio_path.file_stem()?.to_str()?;
    let lrc_path = dir.join(format!("{}.lrc", stem));
    if lrc_path.exists() {
        Some(lrc_path)
    } else {
        None
    }
}

/// Search for LRC files in a directory matching the audio file name.
///
/// Matching is case-insensitive (Windows file system is case-insensitive,
/// and users often download `Song.LRC` alongside `song.mp3`). Returns exact
/// matches first, then partial-name matches.
pub fn search_lrc_files(audio_path: &Path) -> Vec<std::path::PathBuf> {
    let dir = match audio_path.parent() {
        Some(d) => d,
        None => return Vec::new(),
    };
    let stem = match audio_path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return Vec::new(),
    };
    let stem_lc = stem.to_lowercase();

    let mut results = Vec::new();

    // Exact match: song.lrc (case-insensitive on the stem; Windows resolves
    // the actual file on disk via `exists()`)
    let exact = dir.join(format!("{}.lrc", stem));
    if exact.exists() {
        results.push(exact);
    }

    // Scan directory for any .lrc files (case-insensitive extension match)
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext_match = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("lrc"))
                .unwrap_or(false);
            if !ext_match {
                continue;
            }
            let lrc_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let lrc_stem_lc = lrc_stem.to_lowercase();
            // Match if either stem contains the other (case-insensitive)
            if lrc_stem_lc.contains(&stem_lc) || stem_lc.contains(&lrc_stem_lc) {
                if !results.contains(&path) {
                    results.push(path);
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_utf8_with_bom() {
        let content = "[00:01.00]测试歌词\n";
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(content.as_bytes());
        let decoded = decode_lrc_bytes(&bytes);
        assert_eq!(decoded, content);
    }

    #[test]
    fn test_decode_plain_utf8() {
        let content = "[00:01.00]测试歌词\n";
        let decoded = decode_lrc_bytes(content.as_bytes());
        assert_eq!(decoded, content);
    }

    #[test]
    fn test_decode_gbk() {
        // A realistic GBK-encoded LRC excerpt. The ASCII `[ti:]` tag and
        // newlines give chardetng enough context to identify GBK reliably.
        // Source text:
        //   [ti:测试歌曲]
        //   [00:01.00]第一行歌词
        //   [00:03.50]第二行歌词
        let gbk_bytes: &[u8] = &[
            0x5B, 0x74, 0x69, 0x3A, 0xB2, 0xE2, 0xCA, 0xD4, 0xB8, 0xE8, 0xC7, 0xFA, 0x5D, 0x0A,
            0x5B, 0x30, 0x30, 0x3A, 0x30, 0x31, 0x2E, 0x30, 0x30, 0x5D, 0xB5, 0xDA, 0xD2, 0xBB,
            0xD0, 0xD0, 0xB8, 0xE8, 0xB4, 0xCA, 0x0A,
            0x5B, 0x30, 0x30, 0x3A, 0x30, 0x33, 0x2E, 0x35, 0x30, 0x5D, 0xB5, 0xDA, 0xB6, 0xFE,
            0xD0, 0xD0, 0xB8, 0xE8, 0xB4, 0xCA, 0x0A,
        ];
        let decoded = decode_lrc_bytes(gbk_bytes);
        assert!(decoded.contains("测试歌曲"), "missing title in: {}", decoded);
        assert!(decoded.contains("第一行歌词"), "missing line 1 in: {}", decoded);
        assert!(decoded.contains("第二行歌词"), "missing line 2 in: {}", decoded);
    }

    #[test]
    fn test_search_lrc_files_case_insensitive() {
        let dir = std::env::temp_dir();
        let audio = dir.join("MyCaseInsensitiveTest.mp3");
        std::fs::write(&audio, b"").unwrap();
        // Uppercase .LRC extension, different case in stem
        let lrc = dir.join("mycaseinsensitivetest.LRC");
        std::fs::write(&lrc, b"[00:01.00]test\n").unwrap();
        let results = search_lrc_files(&audio);
        assert!(
            results.iter().any(|p| p == &lrc),
            "expected case-insensitive match to find {:?}, got {:?}",
            lrc,
            results
        );
        let _ = std::fs::remove_file(&audio);
        let _ = std::fs::remove_file(&lrc);
    }

    #[test]
    fn test_parse_standard_lrc() {
        let lrc = parse_lrc(r#"
[ti:Test Song]
[ar:Test Artist]
[offset:+500]
[00:10.50]First line
[00:20.00]Second line
[00:30.00]
[00:40.00]Fourth line
"#);
        assert_eq!(lrc.metadata.title, Some("Test Song".to_string()));
        assert_eq!(lrc.metadata.artist, Some("Test Artist".to_string()));
        assert_eq!(lrc.metadata.offset_ms, 500);
        assert_eq!(lrc.lines.len(), 4);
        assert_eq!(lrc.lines[0].time_ms, 10500);
        assert_eq!(lrc.lines[0].text, "First line");
        assert_eq!(lrc.lines[2].text, ""); // instrumental break
    }

    #[test]
    fn test_parse_enhanced_lrc() {
        let lrc = parse_lrc("[00:10.00]<00:10.00>Hello <00:11.00>world");
        assert_eq!(lrc.lines.len(), 1);
        let words = lrc.lines[0].words.as_ref().unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[1].text, "world");
    }

    #[test]
    fn test_parse_multi_timestamp() {
        let lrc = parse_lrc("[00:10.00][00:30.00]Repeated line");
        assert_eq!(lrc.lines.len(), 2);
        assert_eq!(lrc.lines[0].time_ms, 10000);
        assert_eq!(lrc.lines[1].time_ms, 30000);
        assert_eq!(lrc.lines[0].text, "Repeated line");
    }

    #[test]
    fn test_serialize_roundtrip() {
        let original = "[ti:Test Song]\n[ar:Test Artist]\n[offset:500]\n[00:10.50]First line\n[00:20.00]Second line\n";
        let lrc = parse_lrc(original);
        let serialized = lrcfile_to_string(&lrc);
        let reparsed = parse_lrc(&serialized);
        assert_eq!(reparsed.metadata.title, Some("Test Song".to_string()));
        assert_eq!(reparsed.metadata.artist, Some("Test Artist".to_string()));
        assert_eq!(reparsed.metadata.offset_ms, 500);
        assert_eq!(reparsed.lines.len(), 2);
        assert_eq!(reparsed.lines[0].time_ms, 10500);
        assert_eq!(reparsed.lines[0].text, "First line");
    }

    #[test]
    fn test_serialize_no_metadata() {
        let lrc = parse_lrc("[00:01.00]Hello\n[00:02.00]World\n");
        let out = lrcfile_to_string(&lrc);
        assert!(out.starts_with("[00:01.00]Hello"));
    }

    #[test]
    fn test_lrc_path_for_audio() {
        let p = std::path::Path::new("/music/song.flac");
        let lrc = lrc_path_for_audio(p).unwrap();
        assert_eq!(lrc, std::path::Path::new("/music/song.lrc"));
    }
}
