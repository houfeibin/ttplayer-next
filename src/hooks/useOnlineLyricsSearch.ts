import { useCallback } from 'react';
import { useLyricsStore } from '@/stores/lyrics';
import { usePlayerStore } from '@/stores/player';
import type { LrcLine, LyricSearchResult } from '@/utils/ipc';
import { lyricsSearchOnline, lyricsLoadOnline, lyricsGetLines, lyricsSaveToFile } from '@/utils/ipc';

/**
 * Online lyrics search state + actions: keyword, results, in-flight flag, and
 * the load handler that applies a chosen result back into the lyrics store.
 *
 * After a result is loaded from online, the LRC is automatically persisted
 * as `{audio_stem}.lrc` in the same directory so the next play hits the
 * local cache without network round-trips.
 */
export function useOnlineLyricsSearch(
  currentTitle: string,
  currentArtist: string,
  onLoaded: (lines: LrcLine[]) => void,
  onError: (msg: string) => void,
) {
  const { setLines } = useLyricsStore();
  const currentFile = usePlayerStore((s) => s.currentFile);

  const handleSearch = useCallback(async (keyword: string) => {
    const kw = keyword.trim() || `${currentTitle || ''} ${currentArtist || ''}`.trim();
    if (!kw) return [];
    try {
      return await lyricsSearchOnline(kw);
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : (e?.message ?? '搜索失败');
      onError(msg);
      return [] as LyricSearchResult[];
    }
  }, [currentTitle, currentArtist, onError]);

  const handleLoadOnline = useCallback(async (result: LyricSearchResult) => {
    try {
      const ok = await lyricsLoadOnline(result.source, result.id);
      if (!ok) return false;

      const loadedLines = await lyricsGetLines();
      setLines(loadedLines);
      onLoaded(loadedLines);

      // Persist to local file so next auto-load picks it up immediately.
      if (currentFile) {
        lyricsSaveToFile(currentFile)
          .then((savedPath) => console.log('[Lyrics] Saved to', savedPath))
          .catch(() => {}); // best-effort, ignore save errors
      }
      return true;
    } catch (e: any) {
      const msg = typeof e === 'string' ? e : (e?.message ?? '加载失败');
      onError(msg);
      return false;
    }
  }, [setLines, onLoaded, onError, currentFile]);

  return { handleSearch, handleLoadOnline };
}
