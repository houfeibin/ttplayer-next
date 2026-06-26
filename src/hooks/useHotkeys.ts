import { useCallback, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { usePlayerStore } from '@/stores/player';
import {
  togglePlayPause, stop, playNext, playPrev, setVolume,
} from '@/utils/ipc';

/**
 * Handles tray actions and global hotkeys:
 * play_pause, next, prev, stop, volume_up, volume_down.
 */
export function useHotkeys() {
  const state = usePlayerStore((s) => s.state);
  const volume = usePlayerStore((s) => s.volume);
  const storeSetVolume = usePlayerStore((s) => s.setVolume);

  const handlePlayPause = useCallback(async () => {
    if (state === 'Idle' || state === 'Stopped') return;
    await togglePlayPause();
  }, [state]);

  const handleStop = useCallback(async () => { await stop(); }, []);
  const handleNext = useCallback(async () => { await playNext(); }, []);
  const handlePrev = useCallback(async () => { await playPrev(); }, []);

  useEffect(() => {
    const handleAction = (action: string) => {
      switch (action) {
        case 'play_pause': handlePlayPause(); break;
        case 'next': handleNext(); break;
        case 'prev': handlePrev(); break;
        case 'stop': handleStop(); break;
        case 'volume_up':
          storeSetVolume(Math.min(100, volume + 5));
          setVolume(Math.min(100, volume + 5));
          break;
        case 'volume_down':
          storeSetVolume(Math.max(0, volume - 5));
          setVolume(Math.max(0, volume - 5));
          break;
      }
    };
    const u1 = listen<string>('tray-action', (e) => handleAction(e.payload));
    const u2 = listen<string>('global-hotkey', (e) => handleAction(e.payload));
    return () => { u1.then((fn) => fn()); u2.then((fn) => fn()); };
  }, [handlePlayPause, handleNext, handlePrev, handleStop, volume, storeSetVolume]);

  return { handlePlayPause, handleStop, handleNext, handlePrev };
}
