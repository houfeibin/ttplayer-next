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
  /** Timestamp of the last handled playback error — prevents duplicate
   *  auto-skip calls for the same error (the backend emits Error state on
   *  every 50ms tick until a new track starts). */
  const lastErrorTimestampRef = useRef(0);

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

      // Crossfade tracking (Rust auto-triggers, frontend only observes).
      // The backend sets `crossfadePending=true` when the decode thread enters
      // the crossfade window, and clears it (transitioning state to Stopped)
      // once the mix is complete. Mirror that flag here so EOF auto-next is
      // suppressed only while a crossfade is actually in progress.
      if (p.crossfadePending) {
        crossfadeActiveRef.current = true;
      } else if (crossfadeActiveRef.current && p.state !== 'Stopped') {
        // Backend cleared the flag without going through Stopped (e.g. the
        // crossfade was aborted by a manual track change). Release the
        // suppression so normal EOF auto-next can resume.
        crossfadeActiveRef.current = false;
      }

      // Error handling: when the backend reports an Error state (file corrupt,
      // unsupported format, decode failure, etc.), log full details and
      // auto-skip to the next track. Each error is identified by its timestamp
      // to ensure exactly one skip per error — the backend emits Error state
      // on every 50ms tick until a new track starts, so without this guard we
      // would fire dozens of overlapping playNext() calls.
      if (p.state === 'Error' && p.error) {
        // If an error interrupted an in-progress crossfade, release the
        // crossfade flag so the completion handler below doesn't also fire.
        if (crossfadeActiveRef.current) {
          crossfadeActiveRef.current = false;
        }
        if (lastErrorTimestampRef.current !== p.error.timestampMs) {
          lastErrorTimestampRef.current = p.error.timestampMs;
          // Log structured error details for debugging: timestamp, track ID
          // (file name), error kind, and message.
          const trackName = p.error.trackPath
            ? (p.error.trackPath.split(/[/\\]/).pop() ?? p.error.trackPath)
            : '(unknown)';
          const timeStr = new Date(p.error.timestampMs).toISOString();
          console.error(
            `[TTPlayer] Playback error | ${timeStr} | track="${trackName}" | ` +
            `kind="${p.error.kind}" | ${p.error.message}`
          );
          // Auto-skip — the backend's open_and_play clears the error state.
          // If there's no next track (Sequential/Single at end), playNext()
          // returns null and the player stays in Error state (user sees
          // which track failed and can manually navigate).
          playNext().catch((e) => logWarn('error auto-skip', e));
        }
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

      // Reset currentIndex only when playback actually stopped (i.e. there was
      // a file playing before). On initial Idle (no file ever played) we keep
      // the persisted currentIndex loaded above so the user sees which track
      // they were on last session.
      //
      // This runs BEFORE the crossfade-completion handler below so that when
      // a crossfade just finished (state=Stopped, crossfadeActiveRef=true),
      // we skip the reset — the upcoming `playNext()` will set the new index.
      if ((p.state === 'Idle' || p.state === 'Stopped') && prevFileRef.current !== null) {
        if (!crossfadeActiveRef.current) {
          setCurrentIndex(-1);
        }
      }

      // Crossfade completed: clear flag, sync playlist, advance to next track.
      // The backend sets state=Stopped and clears crossfadePending when the
      // mix finishes; this handler responds by opening the next track.
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

      // Lyrics timing update (piggybacked on this event, no separate polling)
      if (p.lyrics) {
        setLyricsIndex(p.lyrics.index);
        setLyricsProgress(p.lyrics.progress);
      }
    }).then((fn) => { unlisten = fn; });

    return () => { unlisten?.(); };
  }, [applyEventPayload, setItems, setCurrentIndex, setLyricsIndex, setLyricsProgress]);
}
