import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { usePlayerStore } from '@/stores/player';
import { usePlaylistStore, pathToName } from '@/stores/playlist';
import { useLyricsStore } from '@/stores/lyrics';
import { getPlaylist, playNext, crossfadeGetDuration, type PlayerStateEvent } from '@/utils/ipc';
import { logWarn } from '@/utils/logger';

/** How close to the end (ms) we consider a track as having reached EOF. */
const EOF_THRESHOLD_MS = 800;
/** Minimum interval (ms) between EOF auto-next triggers. */
const EOF_DEBOUNCE_MS = 3000;

/**
 * Subscribes to Tauri `player-state-update` events and handles:
 * - Player store sync
 * - Playlist refresh on file change
 * - Crossfade state tracking
 * - EOF auto-next
 */
export function usePlayerEvents() {
  const applyEventPayload = usePlayerStore((s) => s.applyEventPayload);
  const setItems = usePlaylistStore((s) => s.setItems);
  const setCurrentIndex = usePlaylistStore((s) => s.setCurrentIndex);
  const setLyricsIndex = useLyricsStore((s) => s.setCurrentIndex);
  const setLyricsProgress = useLyricsStore((s) => s.setProgress);

  const eofAtRef = useRef(0);
  const crossfadeActiveRef = useRef(false);
  const prevFileRef = useRef<string | null>(null);

  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    // Load the persisted playlist on startup so the user sees their saved
    // tracks immediately, even before any playback begins. Without this, the
    // playlist only refreshes when `currentFile` changes — which never happens
    // on a fresh start since there's no file playing yet.
    getPlaylist().then((pl) => {
      if (pl?.items) {
        setItems(
          pl.items.map((it) => ({ path: it.path, format: it.format, name: pathToName(it.path) })),
          pl.currentIndex ?? -1,
        );
      }
    }).catch((e) => logWarn('playlist initial load', e));

    crossfadeGetDuration().catch((e) => logWarn('crossfadeGetDuration', e));

    listen<PlayerStateEvent>('player-state-update', (event) => {
      const p = event.payload;
      applyEventPayload(p);

      // Playlist sync: refresh when file changes
      const curFile = p.currentFile;
      if (curFile !== prevFileRef.current) {
        prevFileRef.current = curFile;
        getPlaylist().then((pl) => {
          if (pl?.items) {
            setItems(
              pl.items.map((it) => ({ path: it.path, format: it.format, name: pathToName(it.path) })),
              pl.currentIndex ?? -1
            );
          }
        }).catch((e) => logWarn('playlist sync', e));
      }

      // Crossfade tracking (Rust auto-triggers, frontend only observes)
      if (p.crossfadePending) {
        crossfadeActiveRef.current = true;
      }

      // EOF auto-next
      const dur = p.durationMs || 0;
      const pos = p.positionMs || 0;
      if (p.state === 'Playing' && dur > 1000 && pos > 0 && pos >= dur - EOF_THRESHOLD_MS) {
        const now = Date.now();
        if (!crossfadeActiveRef.current && now - eofAtRef.current > EOF_DEBOUNCE_MS) {
          eofAtRef.current = now;
          playNext().catch((e) => logWarn('auto-next', e));
        }
      }

      // Crossfade completed: clear flag, sync playlist
      if (p.state === 'Stopped' && crossfadeActiveRef.current) {
        crossfadeActiveRef.current = false;
        getPlaylist().then((pl) => {
          if (pl?.items) {
            setItems(
              pl.items.map((it) => ({ path: it.path, format: it.format, name: pathToName(it.path) })),
              pl.currentIndex ?? -1
            );
          }
        }).catch((e) => logWarn('playlist sync after crossfade', e));
        eofAtRef.current = Date.now();
        playNext().catch((e) => logWarn('crossfade next', e));
      }

      // Reset currentIndex only when playback actually stopped (i.e. there was
      // a file playing before). On initial Idle (no file ever played) we keep
      // the persisted currentIndex loaded above so the user sees which track
      // they were on last session.
      if ((p.state === 'Idle' || p.state === 'Stopped') && prevFileRef.current !== null) {
        if (!crossfadeActiveRef.current) {
          setCurrentIndex(-1);
        }
      }

      // Lyrics timing update (piggybacked on this event, no separate polling)
      if (p.lyrics) {
        setLyricsIndex(p.lyrics.index);
        setLyricsProgress(p.lyrics.progress);
      }
    }).then((fn) => { unlisten = fn; });

    return () => { unlisten?.(); };
  }, [applyEventPayload, setItems, setCurrentIndex, setLyricsIndex, setLyricsProgress]);
}
