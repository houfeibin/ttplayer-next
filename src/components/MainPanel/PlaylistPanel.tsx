import { useCallback, useEffect } from 'react';
import { usePlayerStore } from '@/stores/player';
import { usePlaylistStore, pathToName, type PlayMode } from '@/stores/playlist';
import {
  playIndex, removeTrack, playlistMoveItem, playlistGetPlayMode, playlistSetPlayMode,
} from '@/utils/ipc';
import styles from './PlaylistPanel.module.css';

const PLAY_MODES: { value: PlayMode; label: string; icon: string }[] = [
  { value: 'sequential', label: '顺序播放', icon: '→' },
  { value: 'loop', label: '列表循环', icon: '↻' },
  { value: 'loop_one', label: '单曲循环', icon: '⟲' },
  { value: 'random', label: '随机播放', icon: '🔀' },
  { value: 'single', label: '播完停止', icon: '■' },
];

export default function PlaylistPanel() {
  const { items, currentIndex, playMode, removeItem, moveItem, setPlayMode } = usePlaylistStore();
  const state = usePlayerStore((s) => s.state);

  // Load play mode on mount
  useEffect(() => {
    playlistGetPlayMode()
      .then((m) => setPlayMode(m as PlayMode))
      .catch(() => {});
  }, [setPlayMode]);

  const handleClick = useCallback(async (index: number) => {
    if (index === currentIndex && (state === 'Playing' || state === 'Paused')) return;
    await playIndex(index);
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

  const handleModeChange = useCallback(async (e: React.ChangeEvent<HTMLSelectElement>) => {
    const mode = e.target.value as PlayMode;
    setPlayMode(mode);
    await playlistSetPlayMode(mode);
  }, [setPlayMode]);

  return (
    <div className={styles.section}>
      <div className={styles.header}>
        <span className={styles.headerTitle}>播放列表</span>
        <div className={styles.headerRight}>
          <select
            className={styles.modeSelect}
            value={playMode}
            onChange={handleModeChange}
            title="播放模式"
          >
            {PLAY_MODES.map((m) => (
              <option key={m.value} value={m.value}>{m.icon} {m.label}</option>
            ))}
          </select>
          <span className={styles.count}>{items.length} 首</span>
        </div>
      </div>

      {items.length === 0 ? (
        <div className={styles.empty}>
          <span>📋 播放列表为空</span>
          <span className={styles.emptyHint}>拖入音频文件或点击 📂 打开</span>
        </div>
      ) : (
        <div className={styles.list}>
          {items.map((item, i) => {
            const name = pathToName(item.path);
            const isCurrent = i === currentIndex;
            return (
              <div
                key={item.path}
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
