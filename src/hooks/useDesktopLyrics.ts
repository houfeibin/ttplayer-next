import { useEffect, useRef, useState, useCallback } from 'react';
import { emitTo } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useLyricsStore } from '@/stores/lyrics';

interface DesktopLyricsApi {
  /** Toggle the desktop (always-on-top) lyrics window open/closed. */
  toggleDesktopLyrics: () => Promise<void>;
  /** Whether the desktop lyrics window is currently open. */
  desktopLyricsActive: boolean;
  /** Ref mirroring `desktopLyricsActive` for use in effects without re-subscribing. */
  desktopActiveRef: React.MutableRefObject<boolean>;
}

/**
 * Owns the desktop lyrics window lifecycle and the forwarding of lyrics timing
 * updates to that window via the `lyrics-update` event.
 *
 * **Cross-window event delivery**: Tauri 2's `emit()` only fires listeners in
 * the *current* window. To reach the separate `lyrics-desktop` renderer we use
 * `emitTo('lyrics-desktop', ...)` which targets that webview explicitly.
 */
export function useDesktopLyrics(): DesktopLyricsApi {
  const { currentIndex, progress, hasLyrics } = useLyricsStore();
  const desktopActiveRef = useRef(false);
  const desktopWindowRef = useRef<any>(null);
  const [desktopLyricsActive, setDesktopLyricsActive] = useState(false);

  // Forward lyrics timing to the desktop window whenever it changes.
  useEffect(() => {
    if (!hasLyrics || !desktopActiveRef.current) return;

    const store = useLyricsStore.getState();
    const currentLine = currentIndex !== null ? store.lines[currentIndex] : null;
    void emitTo('lyrics-desktop', 'lyrics-update', {
      text: store.lines[currentIndex ?? -1]?.text ?? '',
      index: currentIndex,
      progress,
      isKaraoke: !!currentLine?.words,
      words: currentLine?.words,
      lineTimeMs: currentLine?.timeMs,
    }).catch((e: unknown) => console.warn('[TTPlayer] lyrics emit:', e));
  }, [hasLyrics, currentIndex, progress]);

  // Close the desktop lyrics window together with the main window. Without
  // this, closing the main window leaves the always-on-top lyrics window
  // orphaned on screen. We listen for the main window's close request and
  // tear down the child window in the same action.
  useEffect(() => {
    const mainWin = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    void mainWin.onCloseRequested(() => {
      if (desktopActiveRef.current && desktopWindowRef.current) {
        void desktopWindowRef.current.close().catch(() => {});
        desktopWindowRef.current = null;
        desktopActiveRef.current = false;
        setDesktopLyricsActive(false);
      }
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  const toggleDesktopLyrics = useCallback(async () => {
    if (desktopActiveRef.current && desktopWindowRef.current) {
      desktopWindowRef.current.close();
      desktopWindowRef.current = null;
      desktopActiveRef.current = false;
      setDesktopLyricsActive(false);
      return;
    }
    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      const win = new WebviewWindow('lyrics-desktop', {
        url: '/lyrics-desktop.html',
        title: 'TTPlayer 歌词',
        width: 600,
        height: 160,
        alwaysOnTop: true,
        transparent: true,
        decorations: false,
        resizable: true,
        skipTaskbar: true,
        center: true,
      });
      await win.once('tauri://created', () => {
        desktopWindowRef.current = win;
        desktopActiveRef.current = true;
        setDesktopLyricsActive(true);

        // Immediately push the current line so the desktop window shows
        // something right away instead of "暂无歌词" until the next tick.
        if (hasLyrics) {
          const store = useLyricsStore.getState();
          const idx = store.currentIndex;
          const currentLine = idx !== null ? store.lines[idx] : null;
          void emitTo('lyrics-desktop', 'lyrics-update', {
            text: store.lines[idx ?? -1]?.text ?? '',
            index: idx,
            progress: store.progress,
            isKaraoke: !!currentLine?.words,
            words: currentLine?.words,
            lineTimeMs: currentLine?.timeMs,
          }).catch(() => {});
        }
      });
      await win.once('tauri://error', () => {
        desktopActiveRef.current = false;
        desktopWindowRef.current = null;
        setDesktopLyricsActive(false);
      });
    } catch (e) {
      console.error('Failed to create desktop lyrics window:', e);
    }
  }, [hasLyrics]);

  return { toggleDesktopLyrics, desktopLyricsActive, desktopActiveRef };
}
