import { useEffect, useState } from 'react';
import { usePlayerStore } from '@/stores/player';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  togglePlayPause, playNext, playPrev,
} from '@/utils/ipc';
import styles from './MiniMode.module.css';

interface Props {
  onExpand: () => void;
}

export default function MiniMode({ onExpand }: Props) {
  const { state, positionMs, durationMs, metadata, currentFile } = usePlayerStore();
  const [hover, setHover] = useState(false);
  const [isMaximized, setIsMaximized] = useState(false);
  const win = getCurrentWindow();

  useEffect(() => {
    win.isMaximized().then(setIsMaximized).catch(() => {});
    const unlisten = win.onResized(() => {
      win.isMaximized().then(setIsMaximized).catch(() => {});
    });
    return () => { unlisten.then((fn: () => void) => fn()); };
  }, [win]);

  const handlePlayPause = async (e: React.MouseEvent) => {
    e.preventDefault();
    if (state === 'Idle') return;
    await togglePlayPause();
  };

  const formatTime = (ms: number) => {
    const totalSec = Math.floor(ms / 1000);
    const min = Math.floor(totalSec / 60);
    const sec = totalSec % 60;
    return `${min}:${sec.toString().padStart(2, '0')}`;
  };

  const fileName = currentFile
    ? (currentFile.split(/[/\\]/).pop() ?? currentFile)
    : 'TTPlayer-Next';
  const title = metadata?.title || fileName;
  const artist = metadata?.artist || '';
  const progress = durationMs > 0 ? (positionMs / durationMs) * 100 : 0;

  return (
    <div
      className={styles.mini}
      onMouseEnter={() => setHover(true)}
      onMouseLeave={() => setHover(false)}
    >
      {/* Progress bar */}
      <div className={styles.progressTrack}>
        <div className={styles.progressFill} style={{ width: `${progress}%` }} />
      </div>

      <div className={styles.body}>
        {/* Track info + time */}
        <div className={styles.info}>
          <span className={styles.title}>{title}</span>
          {artist && <span className={styles.artist}>{artist}</span>}
          <span className={styles.time}>
            {formatTime(positionMs)} / {formatTime(durationMs)}
          </span>
        </div>

        {/* Controls */}
        <div className={styles.controls} style={{ opacity: hover ? 1 : 0.4, transition: 'opacity 0.2s' }}>
          <button className={styles.btn} onMouseDown={(e) => { e.preventDefault(); playPrev(); }} title="上一首" type="button">⏮</button>
          <button className={`${styles.btn} ${styles.btnPlay}`} onMouseDown={handlePlayPause} title="播放/暂停" type="button">
            {state === 'Playing' ? '⏸' : '▶'}
          </button>
          <button className={styles.btn} onMouseDown={(e) => { e.preventDefault(); playNext(); }} title="下一首" type="button">⏭</button>
        </div>

        {/* Window controls */}
        <div className={styles.winCtrls}>
          <button className={styles.winBtn} onMouseDown={(e) => { e.preventDefault(); onExpand(); }} title="展开" type="button">⬜</button>
          <button className={styles.winBtn} onMouseDown={(e) => { e.preventDefault(); win.minimize(); }} title="最小化" type="button">─</button>
          <button className={styles.winBtn} onMouseDown={(e) => { e.preventDefault(); win.toggleMaximize(); }} title={isMaximized ? '还原' : '最大化'} type="button">
            {isMaximized ? '❐' : '□'}
          </button>
          <button className={`${styles.winBtn} ${styles.winCloseBtn}`} onMouseDown={(e) => { e.preventDefault(); win.close(); }} title="关闭" type="button">✕</button>
        </div>
      </div>
    </div>
  );
}
