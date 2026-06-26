/// Lyrics timing engine — maps playback position (ms) to the current lyric line.
///
/// The engine holds parsed lyrics and provides O(log n) lookup via binary search.
/// It tracks the current line index and computes per-line progress for animations.
use crate::lyrics::parser::{LrcFile, LrcLine};
use serde::{Deserialize, Serialize};

/// The engine that tracks which lyric line is active.
pub struct LyricsEngine {
    lines: Vec<LrcLine>,
    /// Global offset from LRC [offset:...] tag (ms).
    offset_ms: i32,
    /// Current line index (None = not started or past end).
    current_index: Option<usize>,
}

/// Result of a timing update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricsUpdate {
    /// Current line index (None if no lyrics or past end).
    pub index: Option<usize>,
    /// Current line text.
    pub text: String,
    /// Progress within the current line (0.0..1.0) for animations.
    /// Only meaningful if the line has enhanced word timings.
    pub progress: f32,
    /// Total number of lines.
    pub total_lines: usize,
    /// Whether we've changed line since last update.
    pub changed: bool,
}

impl LyricsEngine {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            offset_ms: 0,
            current_index: None,
        }
    }

    /// Load lyrics from a parsed LRC file.
    pub fn load(&mut self, lrc: LrcFile) {
        self.lines = lrc.lines;
        self.offset_ms = lrc.metadata.offset_ms;
        self.current_index = None;
    }

    /// Clear all lyrics.
    pub fn clear(&mut self) {
        self.lines.clear();
        self.offset_ms = 0;
        self.current_index = None;
    }

    /// Whether any lyrics are loaded.
    pub fn has_lyrics(&self) -> bool {
        !self.lines.is_empty()
    }

    /// Export the current lyric data as an `LrcFile` for serialization.
    ///
    /// Metadata (title/artist/album) is left empty because the engine only
    /// preserves timed lines and the global offset.  The returned file
    /// round-trips perfectly through `parse_lrc`.
    pub fn to_lrcfile(&self) -> LrcFile {
        LrcFile {
            metadata: crate::lyrics::parser::LrcMetadata {
                offset_ms: self.offset_ms,
                ..Default::default()
            },
            lines: self.lines.clone(),
        }
    }

    /// Get all lines (for frontend rendering).
    pub fn lines(&self) -> &[LrcLine] {
        &self.lines
    }

    /// Get the current line index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Get the current line text.
    pub fn current_text(&self) -> &str {
        self.current_index
            .and_then(|i| self.lines.get(i))
            .map(|l| l.text.as_str())
            .unwrap_or("")
    }

    /// Update the timing engine with the current playback position (ms).
    /// Returns a LyricsUpdate with the current line info.
    pub fn update(&mut self, position_ms: u64) -> LyricsUpdate {
        if self.lines.is_empty() {
            return LyricsUpdate {
                index: None,
                text: String::new(),
                progress: 0.0,
                total_lines: 0,
                changed: false,
            };
        }

        // Apply offset
        let adjusted_ms = (position_ms as i64 + self.offset_ms as i64).max(0) as u64;

        // Binary search for the current line
        let new_index = self.find_line(adjusted_ms);
        let changed = new_index != self.current_index;

        if changed {
            self.current_index = new_index;
        }

        let (text, progress) = if let Some(idx) = new_index {
            let line = &self.lines[idx];
            let text = line.text.clone();
            let progress = if let Some(_words) = &line.words {
                // Enhanced: compute word-level progress
                let line_start = line.time_ms as u64;
                let line_end = if idx + 1 < self.lines.len() {
                    self.lines[idx + 1].time_ms as u64
                } else {
                    line_start + 5000 // default 5s for last line
                };
                let elapsed = adjusted_ms.saturating_sub(line_start);
                let duration = line_end.saturating_sub(line_start);
                if duration > 0 {
                    (elapsed as f32 / duration as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            } else {
                // Standard LRC: compute line-level progress
                let line_start = line.time_ms as u64;
                let line_end = if idx + 1 < self.lines.len() {
                    self.lines[idx + 1].time_ms as u64
                } else {
                    line_start + 5000
                };
                let elapsed = adjusted_ms.saturating_sub(line_start);
                let duration = line_end.saturating_sub(line_start);
                if duration > 0 {
                    (elapsed as f32 / duration as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            };
            (text, progress)
        } else {
            (String::new(), 0.0)
        };

        LyricsUpdate {
            index: new_index,
            text,
            progress,
            total_lines: self.lines.len(),
            changed,
        }
    }

    /// Binary search for the line that should be active at `position_ms`.
    /// Returns the index of the last line whose time_ms <= position_ms.
    fn find_line(&self, position_ms: u64) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }

        // Before the first line
        if position_ms < self.lines[0].time_ms as u64 {
            return None;
        }

        // Binary search
        let mut lo = 0usize;
        let mut hi = self.lines.len();
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if self.lines[mid].time_ms as u64 <= position_ms {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        // lo is now the first line whose time > position_ms
        // so lo - 1 is the last line whose time <= position_ms
        if lo > 0 {
            Some(lo - 1)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::parser::parse_lrc;

    #[test]
    fn test_timing_basic() {
        let lrc = parse_lrc("[00:10.00]First\n[00:20.00]Second\n[00:30.00]Third");
        let mut engine = LyricsEngine::new();
        engine.load(lrc);

        // Before first line
        let u = engine.update(5000);
        assert_eq!(u.index, None);

        // At first line
        let u = engine.update(10000);
        assert_eq!(u.index, Some(0));
        assert_eq!(u.text, "First");

        // Between first and second
        let u = engine.update(15000);
        assert_eq!(u.index, Some(0));

        // At second line
        let u = engine.update(20000);
        assert_eq!(u.index, Some(1));
        assert_eq!(u.text, "Second");
        assert!(u.changed);

        // Same line again
        let u = engine.update(25000);
        assert_eq!(u.index, Some(1));
        assert!(!u.changed);
    }

    #[test]
    fn test_timing_with_offset() {
        let lrc = parse_lrc("[offset:+500]\n[00:10.00]Line");
        let mut engine = LyricsEngine::new();
        engine.load(lrc);

        // 10000ms + 500ms offset = 10500ms adjusted
        // Line is at 10000ms, so at position 9600ms (9600+500=10100 >= 10000)
        let u = engine.update(9600);
        assert_eq!(u.index, Some(0));
    }
}
