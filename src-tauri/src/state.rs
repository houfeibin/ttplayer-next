use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Duration;
use parking_lot::Mutex;
use tokio::sync::RwLock;
use tt_core::lyrics::{LyricsEngine, LyricsProviderRegistry};
use tt_core::player::{AudioPipeline, NextTrackProvider};
use tt_core::skin::SkinManager;
use tt_playlist::PlaylistManager;

/// Theme mode stored as u8: 0=light, 1=dark, 2=system
pub type ThemeMode = u8;

/// Wraps PlaylistManager to provide next-track lookup for crossfade.
struct PlaylistNextProvider {
    playlist: Arc<Mutex<PlaylistManager>>,
}

impl NextTrackProvider for PlaylistNextProvider {
    fn next_track(&self) -> Option<PathBuf> {
        let pl = self.playlist.lock();
        // Use peek_next_path so the play mode is respected (Loop wraps, Random
        // avoids repeats, etc.) WITHOUT advancing current_index — the active
        // track is still playing and its index must not change until the
        // crossfade completes and a new track is explicitly requested.
        pl.active().peek_next_path().map(PathBuf::from)
    }
}

/// Debounced persistence for the playlist.
///
/// Mutations call [`PlaylistSaver::mark_dirty`] instead of writing to disk
/// synchronously. A dedicated thread flushes at most once per
/// `FLUSH_INTERVAL_MS` while dirty, coalescing bursts of edits (add/remove/
/// reorder) into a single write so the command thread is never blocked on
/// disk I/O. On drop the thread performs a final flush so pending changes are
/// not lost when the app shuts down.
pub struct PlaylistSaver {
    playlist: Arc<Mutex<PlaylistManager>>,
    dirty: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

const FLUSH_INTERVAL_MS: u64 = 500;
const SHUTDOWN_POLL_MS: u64 = 50;

impl PlaylistSaver {
    pub fn start(playlist: Arc<Mutex<PlaylistManager>>) -> Self {
        let dirty = Arc::new(AtomicBool::new(false));
        let shutdown = Arc::new(AtomicBool::new(false));
        let pl = playlist.clone();
        let dirty_c = dirty.clone();
        let shut_c = shutdown.clone();
        let handle = std::thread::spawn(move || {
            loop {
                // Sleep FLUSH_INTERVAL, polling for shutdown every SHUTDOWN_POLL
                // so a drop-induced shutdown is noticed promptly.
                let mut waited = 0u64;
                while waited < FLUSH_INTERVAL_MS {
                    if shut_c.load(Ordering::SeqCst) {
                        break;
                    }
                    let step = SHUTDOWN_POLL_MS.min(FLUSH_INTERVAL_MS - waited);
                    std::thread::sleep(Duration::from_millis(step));
                    waited += step;
                }
                if dirty_c.swap(false, Ordering::SeqCst) {
                    let pl = pl.lock();
                    pl.save();
                }
                if shut_c.load(Ordering::SeqCst) {
                    return;
                }
            }
        });
        Self { playlist, dirty, shutdown, handle: Some(handle) }
    }

    /// Mark the playlist as modified; it will be persisted within
    /// `FLUSH_INTERVAL_MS` (or immediately via [`PlaylistSaver::flush_now`]).
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Force an immediate flush of any pending changes.
    pub fn flush_now(&self) {
        if self.dirty.swap(false, Ordering::SeqCst) {
            let pl = self.playlist.lock();
            pl.save();
        }
    }
}

impl Drop for PlaylistSaver {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            // Join so the final flush (if pending) completes before exit.
            let _ = h.join();
        }
    }
}

/// Application global state
pub struct AppState {
    pub player: Arc<Mutex<AudioPipeline>>,
    pub playlist: Arc<Mutex<PlaylistManager>>,
    /// Debounced playlist persistence (replaces synchronous `save()` in commands).
    pub playlist_saver: PlaylistSaver,
    pub lyrics: Arc<Mutex<LyricsEngine>>,
    /// Shared, reusable lyrics provider registry (supports runtime config +
    /// keeps per-provider LRC caches alive across searches).
    pub lyrics_providers: Arc<RwLock<LyricsProviderRegistry>>,
    pub skin: Arc<Mutex<SkinManager>>,
    /// Current theme mode: 0 = light, 1 = dark, 2 = follow system
    pub theme_mode: Arc<AtomicU8>,
    /// Desktop lyrics runtime settings (font size / lock / family / style / color).
    pub desktop_lyrics: Arc<Mutex<crate::commands::desktop_lyrics::DesktopLyricsSettings>>,
    /// Set to `true` on app exit so background threads (event push, etc.) can
    /// wind down gracefully instead of running an infinite loop.
    pub shutdown: Arc<AtomicBool>,
}

impl AppState {
    pub fn new() -> Self {
        // Restore persisted playlists on startup (falls back to default).
        let playlist = Arc::new(Mutex::new(PlaylistManager::load_or_default()));
        let player = Arc::new(Mutex::new(AudioPipeline::new()));

        // Wire up crossfade: player auto-fetches next track from playlist
        {
            let provider: Arc<dyn NextTrackProvider> = Arc::new(PlaylistNextProvider {
                playlist: playlist.clone(),
            });
            player.lock().set_next_track_provider(provider);
        }

        let playlist_saver = PlaylistSaver::start(playlist.clone());

        // Seed built-in skins to the runtime skin directory on first launch
        // so users can edit or delete them as individual files.
        let skin = SkinManager::new();
        if let Err(e) = skin.ensure_skins_on_disk() {
            tracing::warn!("Failed to seed skins to disk: {}", e);
        }

        let theme_mode = Arc::new(AtomicU8::new(
            crate::commands::theme::load_theme_mode(),
        ));

        let desktop_lyrics = crate::commands::desktop_lyrics::load_settings();

        Self {
            player,
            playlist,
            playlist_saver,
            lyrics: Arc::new(Mutex::new(LyricsEngine::new())),
            lyrics_providers: Arc::new(RwLock::new(LyricsProviderRegistry::with_defaults())),
            skin: Arc::new(Mutex::new(skin)),
            theme_mode,
            desktop_lyrics,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
}
