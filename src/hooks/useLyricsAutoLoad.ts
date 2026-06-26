import { useEffect, useRef } from 'react';
import { useLyricsStore } from '@/stores/lyrics';
import { usePlayerStore } from '@/stores/player';
import { lyricsAutoLoad, lyricsGetLines } from '@/utils/ipc';

/**
 * Drives the lyrics lifecycle in response to playback:
 *  - when the current file changes, auto-load matching lyrics (.lrc sibling
 *    or embedded tags); on miss, clear the panel.
 *  - whenever the active line changes, smooth-scroll it into view.
 *
 * Extracted from LyricsPanel so the component body only describes rendering;
 * the file-change/scroll side-effects are independently testable.
 */
export function useLyricsAutoLoad(lineRefs: React.MutableRefObject<(HTMLDivElement | null)[]>) {
  const currentFile = usePlayerStore((s) => s.currentFile);
  const { lines, currentIndex, hasLyrics, setLines, setHasLyrics, clear } = useLyricsStore();
  const lastFileRef = useRef<string | null>(null);

  // Auto-load lyrics when file changes
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
