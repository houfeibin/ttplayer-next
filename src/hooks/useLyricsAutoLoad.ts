import { useEffect, useRef } from 'react';
import { useLyricsStore } from '@/stores/lyrics';
import { usePlayerStore } from '@/stores/player';
import { lyricsAutoLoad, lyricsGetLines } from '@/utils/ipc';

/**
 * Auto-load lyrics when the current file changes.
 *
 * This hook MUST be called from a component that stays mounted in both
 * standard and mini mode (i.e. before any early `return` that unmounts the
 * lyrics panel). Otherwise, switching tracks in mini mode would leave the
 * lyrics store holding the previous track's lines, causing the desktop
 * lyrics window to show stale text.
 *
 * On file change it tries to auto-load a matching `.lrc` sibling or embedded
 * tags; on a miss it clears the store so consumers show the empty state.
 */
export function useLyricsLoader() {
  const currentFile = usePlayerStore((s) => s.currentFile);
  const setLines = useLyricsStore((s) => s.setLines);
  const clear = useLyricsStore((s) => s.clear);
  const lastFileRef = useRef<string | null>(null);

  useEffect(() => {
    if (!currentFile || currentFile === lastFileRef.current) return;
    lastFileRef.current = currentFile;

    (async () => {
      try {
        const found = await lyricsAutoLoad(currentFile);
        if (found) {
          const loadedLines = await lyricsGetLines();
          setLines(loadedLines);
        } else {
          clear();
        }
      } catch (e) {
        console.warn('[TTPlayer] lyricsAutoLoad:', e);
        clear();
      }
    })();
  }, [currentFile, setLines, clear]);
}

/**
 * Smooth-scroll the active lyrics line into view whenever it changes.
 *
 * Pure UI concern — only relevant while the lyrics panel is mounted.
 */
export function useLyricsAutoLoad(lineRefs: React.MutableRefObject<(HTMLDivElement | null)[]>) {
  const { lines, currentIndex, hasLyrics } = useLyricsStore();

  // Auto-scroll to current line
  useEffect(() => {
    if (currentIndex !== null && lineRefs.current[currentIndex]) {
      lineRefs.current[currentIndex]?.scrollIntoView({
        behavior: 'smooth',
        block: 'center',
      });
    }
  }, [currentIndex, lineRefs]);

  return { lines, currentIndex, hasLyrics };
}
