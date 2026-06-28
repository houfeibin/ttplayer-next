import { useEffect, useRef, useState } from 'react';
import { usePlayerStore } from '@/stores/player';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  togglePlayPause, playNext, playPrev,
} from '@/utils/ipc';

interface Props {
  onExpand: () => void;
  width: number;
  height: number;
}

// localStorage key：持久化迷你模式窗口物理坐标，供下次进入迷你模式或重启后恢复
const MINI_POS_KEY = 'ttplayer:mini-mode-pos';

export default function MiniMode({ onExpand, width, height }: Props) {
  const { state, positionMs, durationMs, metadata, currentFile } = usePlayerStore();
  const [isMaximized, setIsMaximized] = useState(false);
  // 使用 ref 稳定 window 实例，避免每次渲染都创建新实例导致 useEffect 重跑
  // getCurrentWindow() 在 Tauri 2 中每次调用返回 new Window()，若放在渲染作用域
  // 会导致 useEffect([win]) 每 50ms（positionMs 更新）重新订阅/取消订阅 onResized
  const winRef = useRef(getCurrentWindow());

  useEffect(() => {
    const win = winRef.current;
    win.isMaximized().then(setIsMaximized).catch(() => {});
    const unlistenResized = win.onResized(() => {
      win.isMaximized().then(setIsMaximized).catch(() => {});
    });
    // 监听窗口移动，持久化物理坐标到 localStorage
    // 使用 300ms debounce 避免拖拽过程中高频写入
    // 关键：exitMiniMode 中的 setPosition 也会触发 onMoved，但 MiniMode
    // 卸载时 cleanup 会清除未触发的定时器，防止标准模式位置覆盖迷你模式位置
    let moveTimer: number | null = null;
    const unlistenMoved = win.onMoved(({ payload }) => {
      if (moveTimer !== null) clearTimeout(moveTimer);
      moveTimer = window.setTimeout(() => {
        try {
          localStorage.setItem(MINI_POS_KEY, JSON.stringify({ x: payload.x, y: payload.y }));
        } catch {
          // localStorage 写入失败时忽略，不影响窗口正常使用
        }
      }, 300);
    });
    return () => {
      // 清除未触发的定时器，防止组件卸载后写入错误位置
      if (moveTimer !== null) clearTimeout(moveTimer);
      unlistenResized.then((fn: () => void) => fn());
      unlistenMoved.then((fn: () => void) => fn());
    };
  }, []);

  const handlePlayPause = async (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
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

  // 按钮基础样式：内联样式确保不受 CSS 模块加载影响
  // 注意：不使用 -webkit-app-region，Tauri 2 会自动将按钮从 data-tauri-drag-region 中排除
  const btnBase: React.CSSProperties = {
    background: 'transparent',
    border: 'none',
    cursor: 'pointer',
    padding: 0,
    margin: 0,
    display: 'inline-flex',
    alignItems: 'center',
    justifyContent: 'center',
    borderRadius: 6,
    transition: 'background 0.15s',
    flexShrink: 0,
    position: 'relative',
    zIndex: 1,
  };

  return (
    <div
      data-tauri-drag-region="deep"
      style={{
        width: `${width}px`,
        height: `${height}px`,
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
        background: 'linear-gradient(180deg, #1a1025 0%, #0f0a1a 100%)',
        borderRadius: 12,
        userSelect: 'none',
        cursor: 'grab',
      }}
    >
      {/* 进度条 */}
      <div
        style={{
          width: '100%',
          height: 3,
          background: 'rgba(255, 255, 255, 0.15)',
          flexShrink: 0,
        }}
      >
        <div
          style={{
            width: `${progress}%`,
            height: '100%',
            background: 'linear-gradient(90deg, #8b5cf6, #a78bfa)',
            transition: 'width 0.3s linear',
          }}
        />
      </div>

      {/* 主体内容 */}
      <div
        style={{
          flex: 1,
          display: 'flex',
          alignItems: 'center',
          padding: '6px 10px',
          gap: 10,
          minWidth: 0,
          minHeight: 0,
        }}
      >
        {/* 歌曲信息 */}
        <div
          style={{
            flex: 1,
            minWidth: 0,
            display: 'flex',
            alignItems: 'baseline',
            gap: 8,
            overflow: 'hidden',
          }}
        >
          <span
            style={{
              fontSize: 13,
              fontWeight: 600,
              color: '#e0d4ff',
              whiteSpace: 'nowrap',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              maxWidth: 180,
            }}
          >
            {title}
          </span>
          {artist && (
            <span
              style={{
                fontSize: 11,
                color: '#a78bfa',
                whiteSpace: 'nowrap',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                maxWidth: 120,
              }}
            >
              {artist}
            </span>
          )}
          <span
            style={{
              fontSize: 10,
              color: '#a090c0',
              whiteSpace: 'nowrap',
              flexShrink: 0,
              fontVariantNumeric: 'tabular-nums',
              marginLeft: 'auto',
            }}
          >
            {formatTime(positionMs)} / {formatTime(durationMs)}
          </span>
        </div>

        {/* 播放控制按钮 */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 2,
            flexShrink: 0,
          }}
        >
          <button
            style={{ ...btnBase, width: 28, height: 28, fontSize: 13, color: '#c4b5fd' }}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); playPrev(); }}
            title="上一首"
            type="button"
          >
            ⏮
          </button>
          <button
            style={{
              ...btnBase,
              width: 30,
              height: 30,
              fontSize: 14,
              color: '#c4b5fd',
              borderRadius: '50%',
              background: 'rgba(139, 92, 246, 0.2)',
              border: '1px solid rgba(139, 92, 246, 0.3)',
            }}
            onClick={handlePlayPause}
            title="播放/暂停"
            type="button"
          >
            {state === 'Playing' ? '⏸' : '▶'}
          </button>
          <button
            style={{ ...btnBase, width: 28, height: 28, fontSize: 13, color: '#c4b5fd' }}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); playNext(); }}
            title="下一首"
            type="button"
          >
            ⏭
          </button>
        </div>

        {/* 窗口控制按钮 */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 2,
            flexShrink: 0,
          }}
        >
          <button
            style={{ ...btnBase, width: 24, height: 24, fontSize: 12, color: '#a090c0' }}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); onExpand(); }}
            title="展开"
            type="button"
          >
            ⬜
          </button>
          <button
            style={{ ...btnBase, width: 24, height: 24, fontSize: 12, color: '#a090c0' }}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); winRef.current.minimize(); }}
            title="最小化"
            type="button"
          >
            ─
          </button>
          <button
            style={{ ...btnBase, width: 24, height: 24, fontSize: 12, color: '#a090c0' }}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); winRef.current.toggleMaximize(); }}
            title={isMaximized ? '还原' : '最大化'}
            type="button"
          >
            {isMaximized ? '❐' : '□'}
          </button>
          <button
            style={{ ...btnBase, width: 24, height: 24, fontSize: 12, color: '#a090c0' }}
            onClick={(e) => { e.preventDefault(); e.stopPropagation(); winRef.current.close(); }}
            title="关闭"
            type="button"
            onMouseEnter={(e) => { e.currentTarget.style.background = '#E94560'; e.currentTarget.style.color = '#fff'; }}
            onMouseLeave={(e) => { e.currentTarget.style.background = 'transparent'; e.currentTarget.style.color = '#a090c0'; }}
          >
            ✕
          </button>
        </div>
      </div>
    </div>
  );
}
