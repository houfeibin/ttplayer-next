import { useCallback, useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { usePlayerStore } from '@/stores/player';
import { usePlaylistStore, pathToName, type PlayMode } from '@/stores/playlist';
import {
  playIndex, removeTrack, playlistMoveItem, playlistGetPlayMode, playlistSetPlayMode,
} from '@/utils/ipc';
import { logWarn } from '@/utils/logger';
import styles from './PlaylistPanel.module.css';

const PLAY_MODES: { value: PlayMode; label: string; icon: string }[] = [
  { value: 'sequential', label: '顺序播放', icon: '→' },
  { value: 'loop', label: '列表循环', icon: '↻' },
  { value: 'loop_one', label: '单曲循环', icon: '⟲' },
  { value: 'random', label: '随机播放', icon: '🔀' },
  { value: 'single', label: '播完停止', icon: '■' },
];

/** Debounce (ms) for scrolling to the current track. Prevents UI flicker
 *  during rapid successive track changes (e.g. auto-skip through corrupt files). */
const SCROLL_DEBOUNCE_MS = 500;

export default function PlaylistPanel() {
  const { items, currentIndex, playMode, removeItem, moveItem, setPlayMode } = usePlaylistStore();
  const state = usePlayerStore((s) => s.state);
  const listRef = useRef<HTMLDivElement>(null);
  const scrollTimerRef = useRef<number | null>(null);

  // ─── 播放模式下拉菜单 ───
  const [modeMenuOpen, setModeMenuOpen] = useState(false);
  const modeBtnRef = useRef<HTMLButtonElement>(null);
  const [modeMenuPos, setModeMenuPos] = useState<{ top: number; left: number } | null>(null);

  // 当菜单打开时，计算按钮在视口中的位置，供 fixed 定位的菜单使用。
  useEffect(() => {
    if (!modeMenuOpen || !modeBtnRef.current) {
      setModeMenuPos(null);
      return;
    }
    const rect = modeBtnRef.current.getBoundingClientRect();
    // 菜单向下展开；若空间不足则向上展开
    const menuHeight = PLAY_MODES.length * 36 + 8;
    const spaceBelow = window.innerHeight - rect.bottom;
    const top = spaceBelow < menuHeight ? rect.top - menuHeight - 4 : rect.bottom + 4;
    setModeMenuPos({ top, left: rect.left });
  }, [modeMenuOpen]);

  // Load play mode on mount
  useEffect(() => {
    playlistGetPlayMode()
      .then((m) => setPlayMode(m as PlayMode))
      .catch(() => {});
  }, [setPlayMode]);

  // Scroll to the current track whenever currentIndex changes.
  // Debounced by SCROLL_DEBOUNCE_MS to prevent flicker during rapid track
  // changes (e.g. auto-skip through multiple corrupt files). The highlight
  // (CSS .active class) updates immediately — only the scroll is debounced.
  // Uses `behavior: 'smooth'` for a ~300ms ease-out animation.
  useEffect(() => {
    if (currentIndex < 0 || !listRef.current) return;
    // Clear any pending scroll timer (rapid change coalescing)
    if (scrollTimerRef.current !== null) {
      clearTimeout(scrollTimerRef.current);
    }
    scrollTimerRef.current = window.setTimeout(() => {
      scrollTimerRef.current = null;
      const row = listRef.current?.querySelector<HTMLElement>(
        `[data-index="${currentIndex}"]`
      );
      // `block: 'nearest'` avoids unnecessary scrolling when the row is
      // already visible, and only scrolls when the current track is off-screen.
      row?.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }, SCROLL_DEBOUNCE_MS);
    return () => {
      if (scrollTimerRef.current !== null) {
        clearTimeout(scrollTimerRef.current);
        scrollTimerRef.current = null;
      }
    };
  }, [currentIndex]);

  const handleClick = useCallback(async (index: number) => {
    if (index === currentIndex && (state === 'Playing' || state === 'Paused')) return;
    try {
      await playIndex(index);
    } catch (e) {
      logWarn('playIndex', e);
    }
  }, [currentIndex, state]);

  const handleRemove = useCallback(async (e: React.MouseEvent, index: number) => {
    e.stopPropagation();
    removeItem(index);
    await removeTrack(index);
  }, [removeItem]);

  const handleMoveUp = useCallback(async (e: React.MouseEvent, index: number) => {
    e.stopPropagation();
    if (index === 0) return;
    moveItem(index, index - 1);
    await playlistMoveItem(index, index - 1);
  }, [moveItem]);

  const handleMoveDown = useCallback(async (e: React.MouseEvent, index: number) => {
    e.stopPropagation();
    if (index >= items.length - 1) return;
    moveItem(index, index + 1);
    await playlistMoveItem(index, index + 1);
  }, [moveItem, items.length]);

  const handleModeSelect = useCallback(async (mode: PlayMode) => {
    setPlayMode(mode);
    setModeMenuOpen(false);
    try {
      await playlistSetPlayMode(mode);
    } catch (e) {
      logWarn('playlistSetPlayMode', e);
    }
  }, [setPlayMode]);

  // 当前播放模式的展示信息
  const currentMode = PLAY_MODES.find((m) => m.value === playMode) ?? PLAY_MODES[0];

  // 菜单项样式：跟随皮肤/主题
  const menuItemStyle: React.CSSProperties = {
    display: 'block',
    width: '100%',
    textAlign: 'left',
    background: 'transparent',
    border: 'none',
    color: 'var(--text-primary, rgba(255,255,255,0.85))',
    fontSize: 12,
    padding: '6px 12px',
    borderRadius: 6,
    cursor: 'pointer',
    transition: 'background 0.15s, color 0.15s',
  };
  // 悬停态：accent 色调半透明覆盖层
  const menuItemHoverStyle: React.CSSProperties = {
    background: 'rgba(var(--accent-rgb, 124, 108, 240), 0.12)',
    color: 'var(--accent-light, #C4B5FD)',
  };
  // 选中态：accent 色调更深覆盖层
  const menuItemActiveStyle: React.CSSProperties = {
    background: 'rgba(var(--accent-rgb, 124, 108, 240), 0.18)',
    color: 'var(--accent-light, #C4B5FD)',
    fontWeight: 600,
  };

  return (
    <div className={styles.section}>
      <div className={styles.header}>
        <span className={styles.headerTitle}>播放列表</span>
        <div className={styles.headerRight}>
          <button
            ref={modeBtnRef}
            className={`${styles.modeSelect} ${modeMenuOpen ? styles.modeSelectActive : ''}`}
            onClick={() => setModeMenuOpen((v) => !v)}
            title="播放模式"
            aria-haspopup="menu"
            aria-expanded={modeMenuOpen}
            type="button"
          >
            {currentMode.icon} {currentMode.label} ▾
          </button>
          <span className={styles.count}>{items.length} 首</span>
        </div>
      </div>
      {modeMenuOpen && modeMenuPos && createPortal(
        <>
          <div
            style={{ position: 'fixed', inset: 0, zIndex: 9998 }}
            onClick={() => setModeMenuOpen(false)}
          />
          <div
            role="menu"
            style={{
              position: 'fixed',
              top: modeMenuPos.top,
              left: modeMenuPos.left,
              minWidth: 140,
              background: 'var(--bg-tertiary, rgba(30, 30, 46, 0.96))',
              backdropFilter: 'blur(8px)',
              border: '1px solid var(--border-color, rgba(255,255,255,0.1))',
              borderRadius: 10,
              padding: 4,
              boxShadow: '0 8px 30px rgba(0,0,0,0.4)',
              zIndex: 9999,
            }}
          >
            {PLAY_MODES.map((m) => {
              const isActive = m.value === playMode;
              const baseStyle = isActive
                ? { ...menuItemStyle, ...menuItemActiveStyle }
                : menuItemStyle;
              return (
                <button
                  key={m.value}
                  style={baseStyle}
                  onMouseEnter={(e) => {
                    if (!isActive) Object.assign(e.currentTarget.style, menuItemHoverStyle);
                  }}
                  onMouseLeave={(e) => {
                    Object.assign(e.currentTarget.style, baseStyle);
                  }}
                  onClick={() => handleModeSelect(m.value)}
                  title={m.label}
                  type="button"
                >{m.icon} {m.label}</button>
              );
            })}
          </div>
        </>,
        document.body,
      )}

      {items.length === 0 ? (
        <div className={styles.empty}>
          <span>📋 播放列表为空</span>
          <span className={styles.emptyHint}>拖入音频文件或点击 📂 打开</span>
        </div>
      ) : (
        <div className={styles.list} ref={listRef}>
          {items.map((item, i) => {
            const name = pathToName(item.path);
            const isCurrent = i === currentIndex;
            return (
              <div
                key={item.path}
                data-index={i}
                className={`${styles.row} ${isCurrent ? styles.active : ''}`}
                onClick={() => handleClick(i)}
                title={item.path}
              >
                <div className={styles.rowInfo}>
                  <span className={styles.index}>{i + 1}</span>
                  <span className={styles.dotIndicator}></span>
                  <span className={styles.name}>{name}</span>
                  <span className={styles.formatTag}>{item.format}</span>
                </div>
                <div className={styles.actions}>
                  <button
                    className={styles.iconBtn}
                    onClick={(e) => handleMoveUp(e, i)}
                    disabled={i === 0}
                    title="上移"
                  >↑</button>
                  <button
                    className={styles.iconBtn}
                    onClick={(e) => handleMoveDown(e, i)}
                    disabled={i === items.length - 1}
                    title="下移"
                  >↓</button>
                  <button
                    className={styles.removeBtn}
                    onClick={(e) => handleRemove(e, i)}
                    title="移除"
                  >✕</button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
