import { useRef, useState, useCallback, useEffect } from 'react';
import { useLyricsStore } from '@/stores/lyrics';
import { usePlayerStore } from '@/stores/player';
import { seek } from '@/utils/ipc';
import type { LrcLine, LyricSearchResult } from '@/utils/ipc';
import Spectrum from './Spectrum';
import { KaraokeLine } from './KaraokeLine';
import { useLyricsAutoLoad } from '@/hooks/useLyricsAutoLoad';
import { useOnlineLyricsSearch } from '@/hooks/useOnlineLyricsSearch';
import styles from './LyricsPanel.module.css';

interface LyricsPanelProps {
  /** 切换桌面歌词窗口开关（由 MainPanel 提升，迷你模式下保持活跃） */
  toggleDesktopLyrics: () => Promise<void>;
  /** 桌面歌词窗口是否开启 */
  desktopLyricsActive: boolean;
}

export default function LyricsPanel({
  toggleDesktopLyrics,
  desktopLyricsActive,
}: LyricsPanelProps) {
  const {
    currentIndex, progress, hasLyrics, offset,
    fontSize, lineHeight, activeColor, inactiveColor,
    fontFamily, textAlign,
    addOffset,
  } = useLyricsStore();

  const currentTitle = usePlayerStore((s) => s.metadata.title);
  const currentArtist = usePlayerStore((s) => s.metadata.artist);
  const currentFile = usePlayerStore((s) => s.currentFile);

  const lineRefs = useRef<(HTMLDivElement | null)[]>([]);
  const linesContainerRef = useRef<HTMLDivElement>(null);
  const { lines } = useLyricsAutoLoad(lineRefs);

  // 歌曲切换时重置歌词滚动位置到顶部，确保新歌词从第一句开始显示。
  // 不依赖 currentIndex 变化（可能未变化导致 effect 不触发），
  // 而是直接监听 currentFile 变化，立即（无动画）将 scrollTop 归零。
  useEffect(() => {
    if (linesContainerRef.current) {
      linesContainerRef.current.scrollTop = 0;
    }
  }, [currentFile]);

  const [showSearch, setShowSearch] = useState(false);
  const [searchKeyword, setSearchKeyword] = useState('');
  const [onlineResults, setOnlineResults] = useState<LyricSearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [lyricsError, setLyricsError] = useState('');

  const { handleSearch, handleLoadOnline } = useOnlineLyricsSearch(
    currentTitle, currentArtist,
    () => { setShowSearch(false); setOnlineResults([]); },
    (msg: string) => { setLyricsError(msg); },
  );

  const onSearch = useCallback(async () => {
    setLyricsError('');
    setIsSearching(true);
    try {
      setOnlineResults(await handleSearch(searchKeyword));
    } finally {
      setIsSearching(false);
    }
  }, [handleSearch, searchKeyword]);

  const onLoadOnline = useCallback(async (result: LyricSearchResult) => {
    await handleLoadOnline(result);
  }, [handleLoadOnline]);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    if (!hasLyrics) return;
    e.preventDefault();
    addOffset(e.deltaY > 0 ? -500 : 500); // 0.5s per wheel tick
  }, [hasLyrics, addOffset]);

  const handleLineClick = useCallback((timeMs: number) => {
    void seek(timeMs);
  }, []);

  const panelStyle = {
    '--lyrics-font-size': `${fontSize}px`,
    '--lyrics-line-height': lineHeight,
    '--lyrics-active-color': activeColor,
    '--lyrics-inactive-color': inactiveColor,
    '--lyrics-font-family': fontFamily,
    '--lyrics-text-align': textAlign,
  } as React.CSSProperties;

  // --- Empty state ---
  if (!hasLyrics && !showSearch) {
    return (
      <div className={styles.section} style={panelStyle}>
        <div className={styles.spectrumBg}><Spectrum /></div>
        <Header
          title="歌词"
          desktopLyricsActive={desktopLyricsActive}
          onSearchClick={() => setShowSearch(true)}
          onDesktopToggle={toggleDesktopLyrics}
        />
        <div className={styles.empty}>
          <span className={styles.emptyIcon}>🎵</span>
          <span className={styles.emptyText}>暂无歌词</span>
          <span className={styles.emptyHint}>将 .lrc 文件放在音频同目录下，或在线搜索</span>
        </div>
      </div>
    );
  }

  // --- Search state ---
  if (showSearch) {
    return (
      <div className={styles.section} style={panelStyle}>
        <div className={styles.spectrumBg}><Spectrum /></div>
        <div className={styles.header}>
          <span className={styles.headerTitle}>🔍 在线搜索歌词</span>
          <button className={styles.btn} onClick={() => setShowSearch(false)}>✕</button>
        </div>
        <div className={styles.searchBox}>
          <input
            className={styles.searchInput}
            value={searchKeyword}
            onChange={(e) => setSearchKeyword(e.target.value)}
            onKeyDown={(e) => e.key === 'Enter' && onSearch()}
            placeholder={`${currentTitle || ''} ${currentArtist || ''}`.trim() || '输入歌名或歌手'}
          />
          <button className={styles.btn} onClick={onSearch} disabled={isSearching}>
            {isSearching ? '搜索中...' : '搜索'}
          </button>
        </div>
        <div className={styles.searchResults}>
          {lyricsError && (
            <div className={styles.emptyHint} style={{ color: '#f87171' }}>{lyricsError}</div>
          )}
          {onlineResults.length === 0 && !isSearching && !lyricsError && (
            <div className={styles.emptyHint}>输入关键词搜索在线歌词</div>
          )}
          {onlineResults.map((r) => (
            <div
              key={`${r.source}-${r.id}`}
              className={styles.searchResult}
              onClick={() => onLoadOnline(r)}
            >
              <span className={styles.resultTitle}>{r.title}</span>
              <span className={styles.resultArtist}>{r.artist}</span>
              {r.album && <span className={styles.resultAlbum}>{r.album}</span>}
            </div>
          ))}
        </div>
      </div>
    );
  }

  // --- Lyrics view ---
  return (
    <div className={styles.section} onWheel={handleWheel} style={panelStyle}>
      <div className={styles.spectrumBg}><Spectrum /></div>
      <div className={styles.header}>
        <span className={styles.headerTitle}>歌词</span>
        <div className={styles.actions}>
          {offset !== 0 && (
            <span className={styles.offsetBadge}>
              偏移: {offset > 0 ? '+' : ''}{(offset / 1000).toFixed(1)}s
            </span>
          )}
          <button className={styles.btn} onClick={() => setShowSearch(true)} title="在线搜索歌词">🔍</button>
          <button
            className={`${styles.btn} ${desktopLyricsActive ? styles.btnActive : ''}`}
            onClick={toggleDesktopLyrics}
            title="桌面歌词"
          >
            🖥️
          </button>
        </div>
      </div>
      <div className={styles.lines} ref={linesContainerRef}>
        {lines.map((line: LrcLine, i: number) => {
          const isActive = i === currentIndex;
          const isPast = currentIndex !== null && i < currentIndex;
          return (
            <div
              key={`${i}-${line.timeMs}`}
              ref={(el) => { lineRefs.current[i] = el; }}
              className={`${styles.line} ${isActive ? styles.active : ''} ${isPast ? styles.past : ''}`}
              onClick={() => handleLineClick(line.timeMs)}
            >
              {isActive && line.words ? (
                <KaraokeLine words={line.words} progress={progress} lineTimeMs={line.timeMs} />
              ) : (
                <span className={styles.lineText}>{line.text || '♪ ♪ ♪'}</span>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}

/// Header row shared by empty + lyrics views (search/action buttons).
function Header({
  title, desktopLyricsActive, onSearchClick, onDesktopToggle,
}: {
  title: string;
  desktopLyricsActive: boolean;
  onSearchClick: () => void;
  onDesktopToggle: () => void;
}) {
  return (
    <div className={styles.header}>
      <span className={styles.headerTitle}>{title}</span>
      <div className={styles.actions}>
        <button className={styles.btn} onClick={onSearchClick} title="在线搜索歌词">🔍</button>
        <button
          className={`${styles.btn} ${desktopLyricsActive ? styles.btnActive : ''}`}
          onClick={onDesktopToggle}
          title="桌面歌词"
        >
          🖥️
        </button>
      </div>
    </div>
  );
}
