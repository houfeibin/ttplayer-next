//! tt-playlist — Playlist manager
//!
//! Supports multiple playlists and 5 play modes (Single/Sequential/Loop/LoopOne/Random).
//! Includes JSON persistence to the user config directory so playlists survive restarts.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tt_common::{PlayMode, TrackInfo};

/// Default persistence filename inside the app config directory.
const PLAYLIST_FILE: &str = "playlists.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub items: Vec<TrackInfo>,
    pub play_mode: PlayMode,
    pub current_index: usize,
}

impl Playlist {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            items: Vec::new(),
            play_mode: PlayMode::Sequential,
            current_index: 0,
        }
    }

    pub fn add_file(&mut self, path: &Path) {
        let info = TrackInfo::from_path(path);
        self.items.push(info);
    }

    pub fn add_files(&mut self, paths: &[PathBuf]) {
        for path in paths {
            self.add_file(path);
        }
    }

    pub fn remove(&mut self, index: usize) {
        if index < self.items.len() {
            self.items.remove(index);
            if self.current_index >= self.items.len() {
                self.current_index = self.items.len().saturating_sub(1);
            }
        }
    }

    /// Move a track from `from` to `to`. Adjusts `current_index` so playback
    /// keeps following the same track. Out-of-range indices are ignored.
    pub fn move_item(&mut self, from: usize, to: usize) {
        let len = self.items.len();
        if from >= len || to >= len || from == to {
            return;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);

        // Keep current_index pointing at the same track.
        self.current_index = match self.current_index {
            ci if ci == from => to,
            ci if from < ci && to >= ci => ci - 1,
            ci if from > ci && to <= ci => ci + 1,
            ci => ci,
        };
    }

    /// Recursively scan `dir` for audio files and add them (sorted by path).
    /// Returns the number of files added.
    pub fn add_folder(&mut self, dir: &Path) -> usize {
        const AUDIO_EXTS: &[&str] = &[
            "flac", "mp3", "wav", "ape", "tak", "ogg", "opus",
            "m4a", "aac", "alac", "wma", "mpc", "ac3", "dts", "eac3",
            "mod", "xm", "s3m", "it",
        ];
        let mut paths = Vec::new();
        collect_audio_files(dir, AUDIO_EXTS, &mut paths);
        paths.sort();
        let count = paths.len();
        self.add_files(&paths);
        count
    }

    /// Convenience accessor for the current play mode.
    pub fn play_mode(&self) -> PlayMode {
        self.play_mode
    }

    /// Set the play mode.
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
    }

    pub fn current(&self) -> Option<&TrackInfo> {
        self.items.get(self.current_index)
    }

    pub fn current_path(&self) -> Option<&str> {
        self.current().map(|t| t.path.as_str())
    }

    /// Compute the next index according to `play_mode` without advancing the
    /// cursor. Returns `None` when there is no successor (Single/Sequential at
    /// the end). Used by crossfade to *peek* the upcoming track without
    /// disturbing the currently-playing index.
    pub fn peek_next_index(&self) -> Option<usize> {
        if self.items.is_empty() {
            return None;
        }
        match self.play_mode {
            PlayMode::Single | PlayMode::Sequential => {
                if self.current_index + 1 >= self.items.len() {
                    None
                } else {
                    Some(self.current_index + 1)
                }
            }
            PlayMode::LoopOne => Some(self.current_index),
            PlayMode::Loop => Some((self.current_index + 1) % self.items.len()),
            PlayMode::Random => Some(self.random_index_excluding_current()),
        }
    }

    /// Peek the next track's path without mutating state.
    pub fn peek_next_path(&self) -> Option<&str> {
        self.peek_next_index()
            .and_then(|i| self.items.get(i))
            .map(|t| t.path.as_str())
    }

    /// Pick a random index different from the current one.
    ///
    /// Fixes the original `rand::random::<usize>() % len` which could pick the
    /// same track repeatedly. When the playlist has only one item, it is
    /// returned as-is (no alternative exists).
    fn random_index_excluding_current(&self) -> usize {
        let len = self.items.len();
        if len <= 1 {
            return self.current_index;
        }
        let mut candidate = rand::random::<usize>() % len;
        let mut attempts = 0;
        while candidate == self.current_index && attempts < 16 {
            candidate = rand::random::<usize>() % len;
            attempts += 1;
        }
        // Fallback: deterministic next index if RNG kept colliding.
        if candidate == self.current_index {
            (self.current_index + 1) % len
        } else {
            candidate
        }
    }

    /// Move to next track, return its path. Advances `current_index`.
    pub fn next(&mut self) -> Option<&str> {
        let new_index = self.peek_next_index()?;
        self.current_index = new_index;
        self.current_path()
    }

    /// Move to previous track
    pub fn prev(&mut self) -> Option<&str> {
        if self.items.is_empty() {
            return None;
        }
        self.current_index = self.current_index.saturating_sub(1);
        self.current_path()
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = 0;
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistManager {
    pub playlists: Vec<Playlist>,
    pub active_index: usize,
    /// Root directory for persistence. `None` means the real user config dir;
    /// `Some(p)` is used in tests to avoid polluting real data.
    #[serde(skip)]
    persistence_root: Option<PathBuf>,
}

impl PlaylistManager {
    pub fn new() -> Self {
        Self {
            playlists: vec![Playlist::new("Default")],
            active_index: 0,
            persistence_root: None,
        }
    }

    pub fn active(&self) -> &Playlist {
        &self.playlists[self.active_index]
    }

    pub fn active_mut(&mut self) -> &mut Playlist {
        &mut self.playlists[self.active_index]
    }

    // ── Persistence ──────────────────────────────────────────────────────

    /// Resolve the persistence file path.
    fn persistence_path(&self) -> PathBuf {
        let root = self
            .persistence_root
            .clone()
            .unwrap_or_else(|| dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")));
        root.join("ttplayer-next").join(PLAYLIST_FILE)
    }

    /// Serialize the manager (all playlists + active index) to JSON on disk.
    /// Best-effort: errors are logged but not propagated so a failed save
    /// never breaks playback.
    pub fn save(&self) {
        let path = self.persistence_path();
        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("Failed to create playlist dir {:?}: {}", parent, e);
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::warn!("Failed to write playlists to {:?}: {}", path, e);
                }
            }
            Err(e) => tracing::warn!("Failed to serialize playlists: {}", e),
        }
    }

    /// Load playlists from the real persistence file (user config dir).
    /// Returns a fresh default manager if the file is missing or unreadable,
    /// so the app always boots into a usable state.
    pub fn load_or_default() -> Self {
        let mut mgr = Self::new();
        let path = mgr.persistence_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str::<PlaylistManager>(&content) {
                Ok(loaded) => {
                    mgr.playlists = loaded.playlists;
                    mgr.active_index = loaded.active_index;
                    // Sanitize: ensure at least one playlist and valid indices.
                    if mgr.playlists.is_empty() {
                        mgr.playlists.push(Playlist::new("Default"));
                        mgr.active_index = 0;
                    }
                    if mgr.active_index >= mgr.playlists.len() {
                        mgr.active_index = 0;
                    }
                    for pl in &mut mgr.playlists {
                        if pl.current_index >= pl.items.len() && !pl.items.is_empty() {
                            pl.current_index = pl.items.len() - 1;
                        } else if pl.items.is_empty() {
                            pl.current_index = 0;
                        }
                    }
                    tracing::info!(
                        "Loaded {} playlist(s) from {:?}",
                        mgr.playlists.len(),
                        path
                    );
                }
                Err(e) => {
                    tracing::warn!("Failed to parse playlists JSON: {}. Using default.", e);
                }
            },
            Err(_) => {
                tracing::debug!("No persisted playlists at {:?}, using default", path);
            }
        }
        mgr
    }
}

/// Recursively collect audio files under `dir` matching one of `exts`.
fn collect_audio_files(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files(&path, exts, out);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if exts.contains(&ext_lower.as_str()) {
                out.push(path);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tt_common::PlayMode;

    fn make_playlist(mode: PlayMode, n: usize) -> Playlist {
        let mut pl = Playlist::new("test");
        pl.play_mode = mode;
        for i in 0..n {
            let mut t = TrackInfo::from_path(Path::new(&format!("/song_{i}.mp3")));
            t.metadata.title = format!("Song {i}");
            pl.items.push(t);
        }
        pl
    }

    #[test]
    fn random_does_not_repeat_current_immediately() {
        let mut pl = make_playlist(PlayMode::Random, 10);
        pl.current_index = 3;
        // Sample many times; the very next pick must never equal current.
        for _ in 0..200 {
            let next = pl.peek_next_index().unwrap();
            assert_ne!(next, pl.current_index, "random picked current index");
            pl.current_index = next;
        }
    }

    #[test]
    fn random_single_item_returns_self() {
        let pl = make_playlist(PlayMode::Random, 1);
        assert_eq!(pl.peek_next_index(), Some(0));
    }

    #[test]
    fn peek_next_does_not_mutate_index() {
        let mut pl = make_playlist(PlayMode::Sequential, 5);
        pl.current_index = 2;
        let peeked = pl.peek_next_index();
        assert_eq!(peeked, Some(3));
        // current_index unchanged
        assert_eq!(pl.current_index, 2);
    }

    #[test]
    fn sequential_at_end_returns_none() {
        let mut pl = make_playlist(PlayMode::Sequential, 3);
        pl.current_index = 2;
        assert_eq!(pl.peek_next_index(), None);
        assert_eq!(pl.next(), None);
    }

    #[test]
    fn loop_mode_wraps_around() {
        let mut pl = make_playlist(PlayMode::Loop, 3);
        pl.current_index = 2;
        assert_eq!(pl.peek_next_index(), Some(0));
        pl.next();
        assert_eq!(pl.current_index, 0);
    }

    #[test]
    fn loop_one_stays_on_current() {
        let mut pl = make_playlist(PlayMode::LoopOne, 3);
        pl.current_index = 1;
        assert_eq!(pl.peek_next_index(), Some(1));
        pl.next();
        assert_eq!(pl.current_index, 1);
    }

    #[test]
    fn move_item_tracks_current_index() {
        let mut pl = make_playlist(PlayMode::Sequential, 5); // 0..4, current=0
        pl.current_index = 2;

        // Move item 2 → 0: current should follow to 0.
        pl.move_item(2, 0);
        assert_eq!(pl.current_index, 0);
        assert_eq!(pl.items[0].metadata.title, "Song 2");

        // current=0, move 1→3: current unaffected (1>0, 3>0).
        pl.move_item(1, 3);
        assert_eq!(pl.current_index, 0);

        // current=0, move 0→4: current follows to 4.
        pl.move_item(0, 4);
        assert_eq!(pl.current_index, 4);
    }

    #[test]
    fn move_item_ignores_out_of_range() {
        let mut pl = make_playlist(PlayMode::Sequential, 3);
        pl.current_index = 1;
        pl.move_item(5, 0); // no-op
        assert_eq!(pl.items.len(), 3);
        assert_eq!(pl.current_index, 1);
    }

    #[test]
    fn set_and_get_play_mode() {
        let mut pl = make_playlist(PlayMode::Sequential, 2);
        assert_eq!(pl.play_mode(), PlayMode::Sequential);
        pl.set_play_mode(PlayMode::Random);
        assert_eq!(pl.play_mode(), PlayMode::Random);
    }

    #[test]
    fn save_then_load_roundtrip() {
        // Use a temp dir so tests never touch the real user config.
        let dir = std::env::temp_dir().join("ttplaylist_test_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut mgr = PlaylistManager {
            playlists: vec![make_playlist(PlayMode::Loop, 3)],
            active_index: 0,
            persistence_root: Some(dir.clone()),
        };
        mgr.active_mut().current_index = 1;
        mgr.save();

        // Verify the file was written inside the test dir.
        let file = dir.join("ttplayer-next").join(PLAYLIST_FILE);
        assert!(file.exists(), "persistence file was not created");

        // Load from the same dir.
        let loaded = PlaylistManager {
            playlists: vec![],
            active_index: 0,
            persistence_root: Some(dir.clone()),
        };
        let path = loaded.persistence_path();
        let content = std::fs::read_to_string(&path).unwrap();
        let mut reloaded: PlaylistManager = serde_json::from_str(&content).unwrap();
        reloaded.persistence_root = None;
        assert_eq!(reloaded.playlists.len(), 1);
        assert_eq!(reloaded.active().items.len(), 3);
        assert_eq!(reloaded.active().current_index, 1);
        assert_eq!(reloaded.active().play_mode, PlayMode::Loop);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = std::env::temp_dir().join("ttplaylist_test_missing");
        let _ = std::fs::remove_dir_all(&dir);
        let mgr = PlaylistManager {
            playlists: vec![],
            active_index: 0,
            persistence_root: Some(dir),
        };
        // No file exists → persistence_path() resolves but read fails.
        let path = mgr.persistence_path();
        assert!(std::fs::read_to_string(&path).is_err());
    }
}
