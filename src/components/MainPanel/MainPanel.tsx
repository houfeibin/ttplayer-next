import { useState, useEffect, useRef, useCallback } from 'react';
import { createPortal, flushSync } from 'react-dom';
import { usePlayerStore } from '@/stores/player';
import { usePlaylistStore } from '@/stores/playlist';
import { usePlayerEvents } from '@/hooks/usePlayerEvents';
import { useHotkeys } from '@/hooks/useHotkeys';
import { usePlaybackActions } from '@/hooks/usePlaybackActions';
import { useTicker } from '@/hooks/useTicker';
import { useDesktopLyrics } from '@/hooks/useDesktopLyrics';
import { getCurrentWindow, LogicalSize, PhysicalPosition, availableMonitors } from '@tauri-apps/api/window';
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
  // 桌面歌词 hook 必须在 `if (miniMode) return` 之前调用：
  // 迷你模式下 LyricsPanel 会被卸载，若 hook 放在 LyricsPanel 中，
  // cleanup 会停止向桌面歌词窗口推送 lyrics-update，导致窗口冻结。
  // 提升到 MainPanel 后，迷你模式下 hook 保持活跃，桌面歌词持续更新。
  const { toggleDesktopLyrics, desktopLyricsActive } = useDesktopLyrics();

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
    // 跟随皮肤/主题的文本颜色
    color: 'var(--text-primary, rgba(255,255,255,0.85))',
    fontSize: 13,
    padding: '8px 12px',
    borderRadius: 8,
    cursor: 'pointer',
    transition: 'background 0.15s, color 0.15s',
  };

  // 悬停态：使用 accent 色调的半透明覆盖层，适配深浅主题
  const menuItemHoverStyle: React.CSSProperties = {
    background: 'rgba(var(--accent-rgb, 124, 108, 240), 0.12)',
    color: 'var(--accent-light, #C4B5FD)',
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
  // 保存进入迷你模式前的窗口尺寸与位置，退出时据此恢复
  // 使用物理坐标（PhysicalPosition）确保多 DPI 环境下位置零误差恢复
  const normalStateRef = useRef<{ w: number; h: number; x: number; y: number } | null>(null);
  const MINI_W = 460;
  const MINI_H = 72;
  // localStorage key：与 MiniMode.tsx 共享，持久化迷你模式窗口物理坐标
  const MINI_POS_KEY = 'ttplayer:mini-mode-pos';

  const enterMiniMode = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      const size = await win.innerSize();
      const pos = await win.outerPosition();
      normalStateRef.current = { w: size.width, h: size.height, x: pos.x, y: pos.y };
      // 先取消最大化（如果在最大化状态），否则后续 setSize 可能无效
      if (await win.isMaximized()) {
        await win.unmaximize();
        await new Promise<void>((r) => setTimeout(r, 50));
      }

      // 先读取保存的位置（在隐藏窗口前完成所有读取操作）
      let targetPos: { x: number; y: number } | null = null;
      const saved = localStorage.getItem(MINI_POS_KEY);
      if (saved) {
        try {
          const { x, y } = JSON.parse(saved);
          // 多显示器边界检查：确认保存的位置仍在某个可用显示器范围内
          // 若 availableMonitors 权限未配置或调用失败，降级为不检查直接恢复
          let shouldRestore = true;
          try {
            const monitors = await availableMonitors();
            shouldRestore = monitors.some((m) =>
              x >= m.position.x && x < m.position.x + m.size.width &&
              y >= m.position.y && y < m.position.y + m.size.height
            );
          } catch {
            // availableMonitors 不可用（权限缺失或 API 未就绪），跳过边界检查
          }
          if (shouldRestore) targetPos = { x, y };
        } catch {
          // JSON 解析失败时忽略，使用当前位置
        }
      }

      // 设置 minSize/maxSize 约束
      await win.setMinSize(new LogicalSize(MINI_W, MINI_H));
      await win.setMaxSize(new LogicalSize(MINI_W, MINI_H));

      if (targetPos) {
        // 关键：在 hide() 之前设置 opacity=0 并等待浏览器 paint 完成。
        // 这样 OS 缓存的最后 paint 帧是空白帧(opacity=0)，show() 时显示空白帧，
        // 而非旧的标准模式界面，从而消除闪现。
        // 若在 hide() 之后设置，窗口隐藏时浏览器停止 paint，DOM 修改不会反映到
        // OS 缓存帧，show() 仍会显示旧帧(MainPanel, opacity=1)造成闪现。
        document.body.style.opacity = '0';
        // 等待两次 rAF 确保浏览器完成 paint（第一帧提交，第二帧确认上屏）
        await new Promise<void>((r) => requestAnimationFrame(() => r()));
        await new Promise<void>((r) => requestAnimationFrame(() => r()));

        // 有保存的位置：隐藏窗口 → 设置尺寸和位置 → 渲染 MiniMode → 显示窗口
        // 确保用户只看到最终状态，无中间的缩小和跳跃过渡
        await win.hide();
        try {
          await win.setSize(new LogicalSize(MINI_W, MINI_H));
          await win.setPosition(new PhysicalPosition(targetPos.x, targetPos.y));
          // 同步渲染 MiniMode（窗口隐藏时 DOM 也会更新，但浏览器不 paint）
          flushSync(() => {
            setMiniMode(true);
          });
          // 显示窗口（OS 显示缓存的空白帧 opacity=0，无闪现）
          await win.show();
          await win.setFocus();
          // 等待浏览器 paint MiniMode(opacity=0) 后再恢复可见
          await new Promise<void>((r) => requestAnimationFrame(() => r()));
          await new Promise<void>((r) => requestAnimationFrame(() => r()));
        } finally {
          // 恢复内容可见
          document.body.style.opacity = '';
          // 确保窗口可见（即使上方步骤失败）
          await win.show().catch(() => {});
        }
      } else {
        // 无保存的位置：直接缩小，无需 hide/show
        await win.setSize(new LogicalSize(MINI_W, MINI_H));
        await new Promise<void>((r) => requestAnimationFrame(() => r()));
        flushSync(() => {
          setMiniMode(true);
        });
      }
    } catch (e) {
      console.error('enterMiniMode failed:', e);
    }
  }, []);

  const exitMiniMode = useCallback(async () => {
    try {
      const win = getCurrentWindow();
      await win.setResizable(true);
      // 先解除 max 约束，再将 min 设为很小值，避免当前窗口(460×72)小于新 min(520×480)
      // 触发 OS 强制扩容 → onResized → 再 setSize 的二次缩放链
      await win.setMaxSize(null);
      await win.setMinSize(null);
      const prev = normalStateRef.current || { w: 820, h: 620, x: 100, y: 100 };

      // 关键：在 hide() 之前设置 opacity=0 并等待浏览器 paint 完成。
      // 这样 OS 缓存的最后 paint 帧是空白帧(opacity=0)，show() 时显示空白帧，
      // 而非旧的 MiniMode 界面，从而消除闪现。
      // 若在 hide() 之后设置，窗口隐藏时浏览器停止 paint，DOM 修改不会反映到
      // OS 缓存帧，show() 仍会显示旧帧(MiniMode, opacity=1)造成闪现。
      document.body.style.opacity = '0';
      // 等待两次 rAF 确保浏览器完成 paint（第一帧提交，第二帧确认上屏）
      await new Promise<void>((r) => requestAnimationFrame(() => r()));
      await new Promise<void>((r) => requestAnimationFrame(() => r()));

      // 隐藏窗口 → 设置尺寸和位置 → 渲染 MainPanel → 显示窗口
      // 确保用户只看到最终状态，无中间的放大和跳跃过渡
      await win.hide();
      try {
        await win.setSize(new LogicalSize(prev.w, prev.h));
        await win.setPosition(new PhysicalPosition(prev.x, prev.y));
        // 恢复标准模式的最小尺寸约束（与 tauri.conf.json 一致）
        await win.setMinSize(new LogicalSize(520, 480));
        // 同步渲染 MainPanel（窗口隐藏时 DOM 也会更新，但浏览器不 paint）
        flushSync(() => {
          setMiniMode(false);
        });
        // 显示窗口（OS 显示缓存的空白帧 opacity=0，无闪现）
        await win.show();
        await win.setFocus();
        // 等待浏览器 paint MainPanel(opacity=0) 后再恢复可见
        await new Promise<void>((r) => requestAnimationFrame(() => r()));
        await new Promise<void>((r) => requestAnimationFrame(() => r()));
      } finally {
        // 恢复内容可见
        document.body.style.opacity = '';
        // 确保窗口可见（即使上方步骤失败）
        await win.show().catch(() => {});
      }
    } catch (e) {
      console.error('exitMiniMode failed:', e);
    }
  }, []);

  // ─── Derived ───
  const fileName = currentFile ? (currentFile.split(/[/\\]/).pop() ?? currentFile) : null;
  const displayTitle = metadata?.title || fileName || 'TTPlayer-Next';
  const displayArtist = metadata?.artist || '';
  const audioBadge = channels ? (channels === 2 ? '立体声' : channels === 1 ? '单声道' : `${channels}ch`) : '';

  // ─── Scrolling title: detect overflow and activate marquee ───
  // When the title (esp. filename fallback) exceeds the container width,
  // a smooth horizontal scroll animation ensures the full text is visible.
  // Measurement uses canvas measureText (independent of DOM state) so it
  // works correctly even while the animation is running. A ResizeObserver
  // re-measures when the container size changes (window resize, etc.).
  const titleRef = useRef<HTMLSpanElement>(null);
  const [titleScrolling, setTitleScrolling] = useState(false);

  useEffect(() => {
    const container = titleRef.current;
    if (!container || !displayTitle) {
      setTitleScrolling(false);
      return;
    }
    const measure = () => {
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      ctx.font = window.getComputedStyle(container).font;
      const textWidth = ctx.measureText(displayTitle).width;
      const containerWidth = container.clientWidth;
      const overflowing = textWidth > containerWidth;
      setTitleScrolling(overflowing);
      if (overflowing) {
        // Consistent scroll speed (~50px/s), clamped to 6–30s range
        const duration = Math.min(30, Math.max(6, textWidth / 50));
        container.style.setProperty('--title-scroll-duration', `${duration}s`);
      }
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(container);
    return () => ro.disconnect();
  }, [displayTitle]);

  // 所有 hooks 必须在此 early return 之前调用完毕
  // 否则 React 会抛出 "Rendered fewer hooks than expected" 错误
  if (miniMode) return <MiniMode onExpand={exitMiniMode} width={MINI_W} height={MINI_H} />;

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
              <span
                className={styles.songTitle}
                ref={titleRef}
                data-scrolling={titleScrolling || undefined}
              >
                <span className={styles.songTitleText} data-text={displayTitle}>
                  {displayTitle}
                </span>
              </span>
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
                      // 跟随皮肤/主题：使用 --bg-tertiary 作为菜单底色
                      background: 'var(--bg-tertiary, rgba(30, 30, 46, 0.96))',
                      backdropFilter: 'blur(8px)',
                      border: '1px solid var(--border-color, rgba(255,255,255,0.1))',
                      borderRadius: 10,
                      padding: 4,
                      boxShadow: '0 8px 30px rgba(0,0,0,0.4)',
                      zIndex: 9999,
                    }}
                  >
                    <button
                      style={menuItemStyle}
                      onMouseEnter={(e) => Object.assign(e.currentTarget.style, menuItemHoverStyle)}
                      onMouseLeave={(e) => Object.assign(e.currentTarget.style, menuItemStyle)}
                      onClick={() => { setAddMenuOpen(false); handleOpenFile(); }}
                      title="选择单个音频文件"
                    >� 添加文件</button>
                    <button
                      style={menuItemStyle}
                      onMouseEnter={(e) => Object.assign(e.currentTarget.style, menuItemHoverStyle)}
                      onMouseLeave={(e) => Object.assign(e.currentTarget.style, menuItemStyle)}
                      onClick={() => { setAddMenuOpen(false); handleOpenFiles(); }}
                      title="选择多个音频文件"
                    >📑 添加多个文件</button>
                    <button
                      style={menuItemStyle}
                      onMouseEnter={(e) => Object.assign(e.currentTarget.style, menuItemHoverStyle)}
                      onMouseLeave={(e) => Object.assign(e.currentTarget.style, menuItemStyle)}
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
          <LyricsPanel
            toggleDesktopLyrics={toggleDesktopLyrics}
            desktopLyricsActive={desktopLyricsActive}
          />
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
