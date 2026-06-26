import { useState, useEffect, useRef, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { usePlayerStore } from '@/stores/player';
import { usePlaylistStore } from '@/stores/playlist';
import { usePlayerEvents } from '@/hooks/usePlayerEvents';
import { useHotkeys } from '@/hooks/useHotkeys';
import { usePlaybackActions } from '@/hooks/usePlaybackActions';
import { useTicker } from '@/hooks/useTicker';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import Equalizer from './Equalizer';
import LyricsPanel from './LyricsPanel';
import PlaylistPanel from './PlaylistPanel';
import SkinSelector from '../SkinSelector';
import MiniMode from '../MiniMode';
import SettingsPanel from '../SettingsPanel';
import FilePropertiesDialog from '../FilePropertiesDialog';
import TagEditor from '../TagEditor';
import FormatConverter from '../FormatConverter';
import styles from './MainPanel.module.css';

export default function MainPanel() {
  // ─── State ───
  const { state, positionMs, durationMs, volume, currentFile, metadata, channels } = usePlayerStore();
  const { items } = usePlaylistStore();

  // ─── Hooks ───
  usePlayerEvents();
  const { handlePlayPause, handleStop, handleNext, handlePrev } = useHotkeys();
  const {
    dragFiles, setDragFiles,
    handleOpenFile, handleOpenFiles, handleOpenFolder, handleProgressClick, handleDrop,
    handleVolumeChange,
    formatTime, progressPercent,
  } = usePlaybackActions();
  const { currentTicker } = useTicker();

  // ─── UI toggles ───
  const [miniMode, setMiniMode] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showFileProps, setShowFileProps] = useState(false);
  const [showTagEditor, setShowTagEditor] = useState(false);
  const [showConverter, setShowConverter] = useState(false);
  const [showSkinSelector, setShowSkinSelector] = useState(false);
  const [showEqualizer, setShowEqualizer] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const [addMenuOpen, setAddMenuOpen] = useState(false);
  const addBtnRef = useRef<HTMLButtonElement>(null);
  const [addMenuPos, setAddMenuPos] = useState<{ top: number; left: number } | null>(null);

  const menuItemStyle: React.CSSProperties = {
    display: 'block',
    width: '100%',
    textAlign: 'left',
    background: 'transparent',
    border: 'none',
    color: 'rgba(255,255,255,0.85)',
    fontSize: 13,
    padding: '8px 12px',
    borderRadius: 8,
    cursor: 'pointer',
    transition: 'background 0.15s, color 0.15s',
  };

  // 当菜单打开时，计算按钮在视口中的位置，供 fixed 定位的菜单使用。
  useEffect(() => {
    if (!addMenuOpen || !addBtnRef.current) {
      setAddMenuPos(null);
      return;
    }
    const rect = addBtnRef.current.getBoundingClientRect();
    setAddMenuPos({ top: rect.bottom + 4, left: rect.left });
  }, [addMenuOpen]);

  // Track window maximize state
  useEffect(() => {
    const win = getCurrentWindow();
    win.isMaximized().then(setIsMaximized).catch(() => {});
    const unlisten = win.onResized(() => {
      win.isMaximized().then(setIsMaximized).catch(() => {});
    });
    return () => { unlisten.then((fn: () => void) => fn()); };
  }, []);

  const handleMinimize = () => getCurrentWindow().minimize();
  const handleMaximize = () => getCurrentWindow().toggleMaximize();
  const handleClose = () => getCurrentWindow().close();

  // ─── Mini-mode window resize ───
  const normalSizeRef = useRef<{ w: number; h: number } | null>(null);
  const MINI_W = 460;
  const MINI_H = 72;

  const enterMiniMode = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      const size = await win.innerSize();
      normalSizeRef.current = { w: size.width, h: size.height };
      await win.setMinSize(new LogicalSize(MINI_W, MINI_H));
      await win.setSize(new LogicalSize(MINI_W, MINI_H));
      await win.setResizable(false);
      setMiniMode(true);
    } catch (e) {
      console.error('enterMiniMode failed:', e);
    }
  }, []);

  const exitMiniMode = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      await win.setResizable(true);
      await win.setMinSize(new LogicalSize(520, 480));
      const prev = normalSizeRef.current || { w: 820, h: 620 };
      await win.setSize(new LogicalSize(prev.w, prev.h));
      setMiniMode(false);
    } catch (e) {
      console.error('exitMiniMode failed:', e);
    }
  }, []);

  if (miniMode) return <MiniMode onExpand={exitMiniMode} />;

  // ─── Derived ───
  const fileName = currentFile ? (currentFile.split(/[/\\]/).pop() ?? currentFile) : null;
  const displayTitle = metadata?.title || fileName || 'TTPlayer-Next';
  const displayArtist = metadata?.artist || '';
  const audioBadge = channels ? (channels === 2 ? '立体声' : channels === 1 ? '单声道' : `${channels}ch`) : '';

  const stateText = (s: string) => {
    switch (s) { case 'Playing': return '播放中'; case 'Paused': return '已暂停'; case 'Stopped': return '已停止'; case 'Idle': return '就绪'; case 'Loading': return '加载中'; default: return s; }
  };

  const dotClass = state === 'Playing' ? styles.statusDot
    : state === 'Paused' ? `${styles.statusDot} ${styles.statusDotPaused}`
    : `${styles.statusDot} ${styles.statusDotIdle}`;

  return (
    <div className={styles.player}
      onDragOver={(e) => { e.preventDefault(); setDragFiles(true); }}
      onDragLeave={() => setDragFiles(false)}
      onDrop={handleDrop}
    >
      {dragFiles && (
        <div className={styles.dragOverlay}>
          <span>🎵 拖入音频文件播放</span>
        </div>
      )}

      {/* ─── 标题栏 ─── */}
      <header className={styles.titlebar}>
        <div className={styles.titlebarLeft} data-tauri-drag-region>
          <span className={styles.titlebarIcon}>🎵</span>
          <span className={styles.titlebarBrand}>
            千千静听
            <span className={styles.titlebarBrandSmall}>· Next</span>
          </span>
        </div>
        <div className={styles.titlebarActions}>
          <button className={styles.titlebarBtn} onClick={enterMiniMode} title="迷你模式" type="button">🔲</button>
          <button className={styles.titlebarBtn} onClick={() => setShowSkinSelector(true)} title="皮肤" type="button">🎨</button>
          <span className={styles.titlebarDivider} />
          <button className={styles.titlebarBtn} onClick={handleMinimize} title="最小化" type="button">─</button>
          <button className={styles.titlebarBtn} onClick={handleMaximize} title={isMaximized ? '还原' : '最大化'} type="button">
            {isMaximized ? '❐' : '□'}
          </button>
          <button className={`${styles.titlebarBtn} ${styles.titlebarCloseBtn}`} onClick={handleClose} title="关闭" type="button">✕</button>
        </div>
      </header>

      {/* ─── 主区域 ─── */}
      <div className={styles.mainVertical}>

        {/* 控制栏 */}
        <div className={styles.controlsBar}>
          {/* 歌曲信息 + 封面 */}
          <div className={styles.songInfo}>
            <div className={styles.songCover}>
              {metadata?.coverArt ? (
                <img src={metadata.coverArt} className={styles.songCoverImg} alt="cover" />
              ) : '♪'}
            </div>
            <div className={styles.songMeta}>
              <span className={styles.songTitle}>{displayTitle}</span>
              {displayArtist && <span className={styles.songArtist}>{displayArtist}</span>}
              {audioBadge && <span className={styles.songBadge}>{audioBadge}</span>}
            </div>
          </div>

          {/* 进度条 */}
          <div className={styles.progressArea}>
            <div className={styles.progressTrack} onClick={handleProgressClick}>
              <div className={styles.progressFill} style={{ width: `${progressPercent}%` }} />
            </div>
            <div className={styles.progressTime}>
              <span className={styles.timeCurrent}>{formatTime(positionMs)}</span>
              <span>{formatTime(durationMs)}</span>
            </div>
          </div>

          {/* 控制按钮 */}
          <div className={styles.controls}>
            <button className={styles.ctrlBtn} onClick={handlePrev} title="上一首" disabled={state === 'Idle'}>⏮</button>
            <button className={`${styles.ctrlBtn} ${styles.btnPlay}`} onClick={handlePlayPause} title="播放/暂停" disabled={state === 'Idle'}>
              {state === 'Playing' ? (
                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <rect x="6" y="5" width="4" height="14" rx="1" />
                  <rect x="14" y="5" width="4" height="14" rx="1" />
                </svg>
              ) : (
                <svg width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                  <path d="M7 5 L17 12 L7 19 Z" />
                </svg>
              )}
            </button>
            <button className={styles.ctrlBtn} onClick={handleNext} title="下一首" disabled={state === 'Idle'}>⏭</button>
          </div>

          {/* 工具按钮 */}
          <div className={styles.tools}>
            <button
              ref={addBtnRef}
              className={`${styles.toolBtn} ${addMenuOpen ? styles.toolBtnActive : ''}`}
              onClick={() => setAddMenuOpen((v) => !v)}
              title="添加文件 / 文件夹"
              aria-haspopup="menu"
              aria-expanded={addMenuOpen}
            >＋ 添加 ▾</button>
              {addMenuOpen && addMenuPos && createPortal(
                <>
                  <div
                    style={{ position: 'fixed', inset: 0, zIndex: 9998 }}
                    onClick={() => setAddMenuOpen(false)}
                  />
                  <div
                    role="menu"
                    style={{
                      position: 'fixed',
                      top: addMenuPos.top,
                      left: addMenuPos.left,
                      minWidth: 180,
                      background: 'rgba(30, 30, 46, 0.96)',
                      backdropFilter: 'blur(8px)',
                      border: '1px solid rgba(255,255,255,0.1)',
                      borderRadius: 10,
                      padding: 4,
                      boxShadow: '0 8px 30px rgba(0,0,0,0.4)',
                      zIndex: 9999,
                    }}
                  >
                    <button
                      style={menuItemStyle}
                      onClick={() => { setAddMenuOpen(false); handleOpenFile(); }}
                      title="选择单个音频文件"
                    >� 添加文件</button>
                    <button
                      style={menuItemStyle}
                      onClick={() => { setAddMenuOpen(false); handleOpenFiles(); }}
                      title="选择多个音频文件"
                    >📑 添加多个文件</button>
                    <button
                      style={menuItemStyle}
                      onClick={() => { setAddMenuOpen(false); handleOpenFolder(); }}
                      title="选择文件夹（递归扫描）"
                    >🗂 添加文件夹</button>
                  </div>
                </>,
                document.body,
              )}
            <button
              className={`${styles.toolBtn} ${showEqualizer ? styles.toolBtnActive : ''}`}
              onClick={() => setShowEqualizer(!showEqualizer)}
              title="均衡器"
            >🎛</button>
            <div className={styles.volWrap}>
              <span className={styles.volIcon}>🔊</span>
              <input
                type="range" min={0} max={100} value={volume}
                onChange={handleVolumeChange}
                className={styles.volSlider}
                title="音量"
              />
            </div>
          </div>
        </div>

        {/* 分割区域：播放列表（上） + 歌词（下） */}
        <div className={styles.splitArea}>
          <PlaylistPanel />
          <LyricsPanel />
        </div>
      </div>

      {/* ─── 底部状态栏 ─── */}
      <footer className={styles.statusbar}>
        <div className={styles.statusLeft}>
          <span><span className={dotClass}></span>{stateText(state)}</span>
          <span>{currentTicker}</span>
        </div>
        <div className={styles.statusRight}>
          {currentFile && <button className={styles.statusTag} onClick={() => setShowFileProps(true)}>ℹ️ 属性</button>}
          {currentFile && <button className={styles.statusTag} onClick={() => setShowTagEditor(true)}>🏷️ 标签</button>}
          <button className={styles.statusTag} onClick={() => setShowConverter(true)}>🔄 转换</button>
          <button className={styles.statusTag} onClick={() => setShowSettings(true)}>⚙️ 设置</button>
        </div>
      </footer>

      {/* ─── EQ 浮层 ─── */}
      {showEqualizer && (
        <div className={styles.eqOverlay} onClick={(e) => e.stopPropagation()}>
          <Equalizer />
        </div>
      )}

      {/* ─── 模态框 ─── */}
      {showSkinSelector && (
        <div className={styles.modalOverlay} onClick={() => setShowSkinSelector(false)}>
          <div className={styles.modalContent} onClick={(e) => e.stopPropagation()}><SkinSelector /></div>
        </div>
      )}
      {showSettings && <SettingsPanel onClose={() => setShowSettings(false)} />}
      {showFileProps && currentFile && <FilePropertiesDialog filePath={currentFile} onClose={() => setShowFileProps(false)} />}
      {showTagEditor && currentFile && <TagEditor filePath={currentFile} onClose={() => setShowTagEditor(false)} />}
      {showConverter && <FormatConverter onClose={() => setShowConverter(false)} />}
    </div>
  );
}
