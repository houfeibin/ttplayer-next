import { useEffect, useRef, useState, useCallback } from 'react';
import { emitTo } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { invoke } from '@tauri-apps/api/core';
import { useLyricsStore } from '@/stores/lyrics';

interface DesktopLyricsApi {
  /** Toggle the desktop (always-on-top) lyrics window open/closed. */
  toggleDesktopLyrics: () => Promise<void>;
  /** Whether the desktop lyrics window is currently open. */
  desktopLyricsActive: boolean;
  /** Ref mirroring `desktopLyricsActive` for use in effects without re-subscribing. */
  desktopActiveRef: React.MutableRefObject<boolean>;
}

/**
 * When a line has no native word-level timings (standard LRC), synthesize
 * evenly-distributed word timings by splitting the text into segments.
 *
 * CJK characters are split individually; Latin words are kept whole.
 * The `progress` from the engine still tracks the real playback position,
 * so the word highlighting remains synced to the music.
 */
function synthesizeWords(
  text: string,
  lineTimeMs: number,
  nextLineTimeMs: number,
): { timeMs: number; text: string }[] {
  const duration = nextLineTimeMs - lineTimeMs;
  if (duration <= 0 || !text) return [];

  const segments: string[] = [];
  let buf = '';
  for (const ch of text) {
    const isCJK = /[\u4e00-\u9fff\u3040-\u309f\u30a0-\u30ff\uac00-\ud7af]/.test(ch);
    if (isCJK) {
      if (buf) { segments.push(buf); buf = ''; }
      segments.push(ch);
    } else if (ch === ' ') {
      if (buf) { segments.push(buf); buf = ''; }
      segments.push(' ');
    } else {
      buf += ch;
    }
  }
  if (buf) segments.push(buf);

  if (segments.length === 0) return [];

  // 用 n+1 份分配，使最后一个字提前开始高亮，避免行尾来不及显示就跳行
  const step = duration / (segments.length + 1);
  return segments.map((seg, i) => ({
    timeMs: Math.round(lineTimeMs + i * step),
    text: seg,
  }));
}

/**
 * Build the lyrics-update payload for the desktop window.
 * Falls back to synthesized word timings when the LRC has no native
 * `<mm:ss.xx>` word-level tags, so karaoke mode works with any LRC file.
 *
 * **Double-line page-flip**: lines are grouped into pairs (0+1, 2+3, …).
 * The pair stays on screen until playback advances past both lines, then
 * flips to the next pair. Within a pair, the line being sung is highlighted
 * and the other is dimmed.
 */
function buildLyricsPayload(
  store: ReturnType<typeof useLyricsStore.getState>,
  idx: number | null,
  progress: number,
) {
  const currentLine = idx !== null ? store.lines[idx] : null;

  // Page-flip: group lines into pairs. pageStart is always even.
  // Even idx → current is left line; odd idx → current is right line.
  const pageStart = idx !== null ? Math.floor(idx / 2) * 2 : null;
  const leftLine = pageStart !== null ? store.lines[pageStart] : null;
  const rightLine = pageStart !== null ? store.lines[pageStart + 1] : null;
  const currentSide: 'left' | 'right' = idx !== null && idx % 2 === 1 ? 'right' : 'left';

  const text = leftLine?.text ?? '';
  const nextText = rightLine?.text ?? '';

  // Word timings always come from the line currently being sung
  let words = currentLine?.words;
  let isKaraoke = !!words;

  if (!words && currentLine && currentLine.text) {
    const nextIdx = (idx ?? 0) + 1;
    const nextTimeMs = store.lines[nextIdx]?.timeMs ?? currentLine.timeMs + 5000;
    const synthesized = synthesizeWords(currentLine.text, currentLine.timeMs, nextTimeMs);
    if (synthesized.length > 0) {
      words = synthesized;
      isKaraoke = true;
    }
  }

  return {
    text,           // left line text
    nextText,       // right line text
    currentSide,    // which side is currently being sung
    index: idx,
    progress,
    isKaraoke,
    words,
    lineTimeMs: currentLine?.timeMs,
  };
}

/**
 * Owns the desktop lyrics window lifecycle and the forwarding of lyrics timing
 * updates to that window via the `lyrics-update` event.
 *
 * **Cross-window event delivery**: Tauri 2's `emit()` only fires listeners in
 * the *current* window. To reach the separate `lyrics-desktop` renderer we use
 * `emitTo('lyrics-desktop', ...)` which targets that webview explicitly.
 */
const GEOMETRY_KEY = 'ttplayer:desktop-lyrics-geometry';
const GEOMETRY_KEY_H = 'ttplayer:desktop-lyrics-geometry-horizontal';
const GEOMETRY_KEY_V = 'ttplayer:desktop-lyrics-geometry-vertical';

interface WindowGeometry { x: number; y: number; width: number; height: number; }

/**
 * 按方向读取持久化的窗口几何（逻辑坐标）。
 * 横屏和竖屏分别保存，避免互相覆盖。
 * 向后兼容：新 key 不存在时回退到旧版单一 key。
 */
function loadGeometry(direction: string): WindowGeometry | null {
  const key = direction === 'vertical' ? GEOMETRY_KEY_V : GEOMETRY_KEY_H;
  try {
    const raw = localStorage.getItem(key) ?? localStorage.getItem(GEOMETRY_KEY);
    if (!raw) return null;
    const g = JSON.parse(raw);
    if (typeof g.x === 'number' && typeof g.y === 'number' &&
        typeof g.width === 'number' && typeof g.height === 'number') {
      return g;
    }
  } catch { /* ignore */ }
  return null;
}

function saveGeometry(g: WindowGeometry) {
  try { localStorage.setItem(GEOMETRY_KEY, JSON.stringify(g)); } catch { /* ignore */ }
}

export function useDesktopLyrics(): DesktopLyricsApi {
  const { currentIndex, progress, hasLyrics } = useLyricsStore();
  const desktopActiveRef = useRef(false);
  const desktopWindowRef = useRef<any>(null);
  // 标记应用正在关闭：onCloseRequested 时置 true，防止 tauri://destroyed 回调
  // 将 visible 持久化为 false（应用关闭时桌面歌词应保持 visible=true 以便下次恢复）
  const isAppClosingRef = useRef(false);
  const [desktopLyricsActive, setDesktopLyricsActive] = useState(false);

  // Forward lyrics timing to the desktop window.
  // 节流策略：currentIndex 变化时强制推送（歌词行切换需即时反馈），
  // progress 变化时节流到 ~8Hz（120ms 间隔），避免 20Hz 无节流 emitTo 导致
  // 桌面窗口 IPC 反序列化 + React 渲染积压引发界面冻结。
  const lastPushTimeRef = useRef(0);
  const lastIndexRef = useRef<number | null>(null);

  useEffect(() => {
    if (!desktopActiveRef.current) return;

    // 新歌无歌词时发送空 payload，清空桌面歌词窗口残留的上一首歌词，
    // 触发桌面歌词组件的"♪ 暂无歌词 ♪"占位显示。
    if (!hasLyrics) {
      void emitTo('lyrics-desktop', 'lyrics-update', {
        text: '',
        nextText: '',
        currentSide: 'left',
        index: null,
        progress: 0,
        isKaraoke: false,
        words: undefined,
        lineTimeMs: undefined,
      }).catch(() => {});
      return;
    }

    // currentIndex 变化时强制推送，不受节流限制
    const indexChanged = lastIndexRef.current !== currentIndex;
    if (indexChanged) {
      lastIndexRef.current = currentIndex;
      lastPushTimeRef.current = Date.now();
      const store = useLyricsStore.getState();
      void emitTo('lyrics-desktop', 'lyrics-update', buildLyricsPayload(store, currentIndex, progress))
        .catch((e: unknown) => console.warn('[TTPlayer] lyrics emit:', e));
      return;
    }

    // progress 变化时节流推送（~8Hz = 120ms 间隔）
    const now = Date.now();
    if (now - lastPushTimeRef.current < 120) return;
    lastPushTimeRef.current = now;

    const store = useLyricsStore.getState();
    void emitTo('lyrics-desktop', 'lyrics-update', buildLyricsPayload(store, currentIndex, progress))
      .catch((e: unknown) => console.warn('[TTPlayer] lyrics emit:', e));
  }, [hasLyrics, currentIndex, progress]);

  // Close the desktop lyrics window together with the main window. Without
  // this, closing the main window leaves the always-on-top lyrics window
  // orphaned on screen. We listen for the main window's close request and
  // tear down the child window in the same action.
  useEffect(() => {
    const mainWin = getCurrentWindow();
    let unlisten: (() => void) | undefined;
    void mainWin.onCloseRequested(() => {
      // 标记应用正在关闭，使 tauri://destroyed 回调不持久化 visible:false，
      // 从而保持 visible:true，下次启动自动恢复桌面歌词窗口。
      isAppClosingRef.current = true;
      if (desktopActiveRef.current && desktopWindowRef.current) {
        void desktopWindowRef.current.close().catch(() => {});
        desktopWindowRef.current = null;
        desktopActiveRef.current = false;
        setDesktopLyricsActive(false);
      }
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  const toggleDesktopLyrics = useCallback(async () => {
    if (desktopActiveRef.current && desktopWindowRef.current) {
      desktopWindowRef.current.close();
      desktopWindowRef.current = null;
      desktopActiveRef.current = false;
      setDesktopLyricsActive(false);
      // 持久化关闭状态，下次启动不自动恢复
      void invoke('desktop_lyrics_set', { visible: false }).catch(() => {});
      return;
    }
    try {
      const { WebviewWindow } = await import('@tauri-apps/api/webviewWindow');
      // 读取后端方向设置，按方向选择对应的持久化几何
      let direction = 'horizontal';
      try {
        const s = await invoke<{ direction: string }>('desktop_lyrics_get');
        direction = s.direction || 'horizontal';
      } catch { /* 默认 horizontal */ }
      const saved = loadGeometry(direction);
      const win = new WebviewWindow('lyrics-desktop', {
        url: '/lyrics-desktop.html',
        title: 'TTPlayer 歌词',
        width: saved?.width ?? 600,
        height: saved?.height ?? 160,
        x: saved?.x,
        y: saved?.y,
        alwaysOnTop: true,
        transparent: true,
        decorations: false,
        shadow: false,
        resizable: true,
        skipTaskbar: true,
        center: !saved,
      });
      await win.once('tauri://created', () => {
        desktopWindowRef.current = win;
        desktopActiveRef.current = true;
        setDesktopLyricsActive(true);
        // 持久化开启状态，下次启动自动恢复
        void invoke('desktop_lyrics_set', { visible: true }).catch(() => {});

        // Immediately push the current line so the desktop window shows
        // something right away instead of "暂无歌词" until the next tick.
        if (hasLyrics) {
          const store = useLyricsStore.getState();
          const idx = store.currentIndex;
          void emitTo('lyrics-desktop', 'lyrics-update', buildLyricsPayload(store, idx, store.progress))
            .catch(() => {});
        }
      });
      // Listen for window close (close button, Alt+F4, etc.) to sync state
      await win.once('tauri://destroyed', () => {
        desktopWindowRef.current = null;
        desktopActiveRef.current = false;
        setDesktopLyricsActive(false);
        // 应用关闭导致的销毁不持久化 visible:false（保持 true 以便下次启动恢复）；
        // 仅用户主动关闭桌面歌词窗口时才持久化 false。
        if (!isAppClosingRef.current) {
          void invoke('desktop_lyrics_set', { visible: false }).catch(() => {});
        }
      });
      await win.once('tauri://error', () => {
        desktopActiveRef.current = false;
        desktopWindowRef.current = null;
        setDesktopLyricsActive(false);
      });
    } catch (e) {
      console.error('Failed to create desktop lyrics window:', e);
    }
  }, [hasLyrics]);

  // 应用启动时读取持久化的 visible 状态，若上次关闭时桌面歌词处于开启状态则自动恢复。
  // 使用 ref 存储 toggleDesktopLyrics 的最新引用，使 mount effect 只执行一次，
  // 不因 hasLyrics 变化而重复触发（避免每次歌词加载/清空都尝试恢复窗口）。
  const toggleRef = useRef(toggleDesktopLyrics);
  toggleRef.current = toggleDesktopLyrics;
  useEffect(() => {
    let cancelled = false;
    invoke<{ visible: boolean }>('desktop_lyrics_get')
      .then((settings) => {
        if (!cancelled && settings.visible && !desktopActiveRef.current) {
          void toggleRef.current();
        }
      })
      .catch(() => {});
    return () => { cancelled = true; };
  }, []);

  return { toggleDesktopLyrics, desktopLyricsActive, desktopActiveRef };
}
