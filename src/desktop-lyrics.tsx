import { useEffect, useState, useCallback } from 'react';
import { listen } from '@tauri-apps/api/event';
import { createRoot } from 'react-dom/client';
import {
  skinGetCurrent, skinApply, themeGetMode,
  desktopLyricsGet, desktopLyricsSet,
  DESKTOP_LYRICS_FONT_MIN, DESKTOP_LYRICS_FONT_MAX,
  DESKTOP_LYRICS_FONT_DEFAULT, DESKTOP_LYRICS_FONT_FAMILY_DEFAULT, DESKTOP_LYRICS_FONT_COLOR_DEFAULT,
  type DesktopLyricsSettings,
} from '@/utils/ipc';

interface LyricsEvent {
  text: string;
  index: number | null;
  progress: number;
  isKaraoke: boolean;
  words?: { timeMs: number; text: string }[];
  lineTimeMs?: number;
}

type ThemeMode = 'light' | 'dark' | 'system';

const FONT_STEP = 2;

function resolveTheme(mode: ThemeMode): 'light' | 'dark' {
  if (mode === 'system') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return mode;
}

/**
 * Inject the active skin's CSS variables into this window's :root so that
 * var(--accent) etc. resolve to the user's chosen theme colour.
 * Also syncs the `data-theme` attribute so light-mode overrides in the
 * skin CSS take effect (mirrors ThemeProvider in the main window).
 *
 * Listens for `skin-changed` events from the main window so that skin
 * switches are reflected in the desktop lyrics window without a restart.
 */
function useSkinCss() {
  useEffect(() => {
    let styleEl: HTMLStyleElement | null = null;
    let currentMode: ThemeMode = 'dark';

    const applyTheme = (mode: ThemeMode) => {
      const resolved = resolveTheme(mode);
      document.documentElement.setAttribute('data-theme', resolved);
    };

    const injectCss = (css: string) => {
      if (!styleEl) {
        styleEl = document.createElement('style');
        styleEl.id = 'ttplayer-skin-vars';
        document.head.appendChild(styleEl);
      }
      styleEl.textContent = css;
    };

    (async () => {
      try {
        // Load and apply skin CSS
        const id = await skinGetCurrent();
        const css = await skinApply(id);
        injectCss(css);

        // Load and apply theme mode
        const raw = await themeGetMode();
        currentMode = (raw === 'light' || raw === 'system') ? raw : 'dark';
        applyTheme(currentMode);
      } catch (e) {
        console.error('Desktop lyrics skin init error:', e);
        applyTheme('dark');
      }
    })();

    // Listen for skin changes from the main window
    const unlistenSkin = listen<{ skinId: string; css: string }>('skin-changed', (event) => {
      injectCss(event.payload.css);
    });

    // Listen for theme mode changes from the main window
    const unlistenTheme = listen<{ mode: string }>('theme-changed', (event) => {
      const mode = event.payload.mode as ThemeMode;
      currentMode = mode;
      applyTheme(mode);
    });

    // Listen for OS theme changes when mode === 'system'
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      if (currentMode === 'system') applyTheme('system');
    };
    mq.addEventListener('change', onChange);

    return () => {
      unlistenSkin.then(fn => fn());
      unlistenTheme.then(fn => fn());
      if (styleEl) document.head.removeChild(styleEl);
      mq.removeEventListener('change', onChange);
    };
  }, []);
}

/** 锁定/解锁图标（SVG，关于 viewBox 中心对称）。 */
function LockIcon({ locked }: { locked: boolean }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {locked ? (
        <>
          <rect x="5" y="11" width="14" height="9" rx="2" />
          <path d="M8 11V8a4 4 0 0 1 8 0v3" />
        </>
      ) : (
        <>
          <rect x="5" y="11" width="14" height="9" rx="2" />
          <path d="M8 11V8a4 4 0 0 1 7-2.6" />
        </>
      )}
    </svg>
  );
}

function DesktopLyrics() {
  const [lyrics, setLyrics] = useState<LyricsEvent>({
    text: '',
    index: null,
    progress: 0,
    isKaraoke: false,
  });
  const [settings, setSettings] = useState<DesktopLyricsSettings>({
    font_size: DESKTOP_LYRICS_FONT_DEFAULT,
    locked: false,
    font_family: DESKTOP_LYRICS_FONT_FAMILY_DEFAULT,
    bold: true,
    italic: false,
    font_color: DESKTOP_LYRICS_FONT_COLOR_DEFAULT,
  });
  const [hovered, setHovered] = useState(false);

  useSkinCss();

  // 加载持久化设置
  useEffect(() => {
    desktopLyricsGet()
      .then(setSettings)
      .catch((e) => console.warn('[DesktopLyrics] load settings:', e));
  }, []);

  // 监听设置变更（来自主窗口设置面板或本窗口操作的后端广播），保持双向同步
  useEffect(() => {
    const unlisten = listen<DesktopLyricsSettings>('desktop-lyrics-settings-changed', (event) => {
      setSettings(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // 锁定状态控制整窗拖动：锁定时禁用拖动（no-drag），解锁时恢复拖动（drag）。
  // 注意 html, body 默认为 drag（见 lyrics-desktop.html），这里通过内联样式覆盖。
  useEffect(() => {
    const region = settings.locked ? 'no-drag' : 'drag';
    document.body.style.setProperty('-webkit-app-region', region);
    document.documentElement.style.setProperty('-webkit-app-region', region);
  }, [settings.locked]);

  // 歌词更新监听
  useEffect(() => {
    const unlisten = listen<LyricsEvent>('lyrics-update', (event) => {
      setLyrics(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  const changeFontSize = useCallback(async (delta: number) => {
    setSettings((prev) => {
      const next = Math.max(DESKTOP_LYRICS_FONT_MIN, Math.min(DESKTOP_LYRICS_FONT_MAX, prev.font_size + delta));
      void desktopLyricsSet({ font_size: next }).catch((e) => console.warn('[DesktopLyrics] set font size:', e));
      return { ...prev, font_size: next };
    });
  }, []);

  const toggleLock = useCallback(async () => {
    setSettings((prev) => {
      const next = !prev.locked;
      void desktopLyricsSet({ locked: next }).catch((e) => console.warn('[DesktopLyrics] set locked:', e));
      return { ...prev, locked: next };
    });
  }, []);

  const { font_size: fontSize, locked, font_family: fontFamily, bold, italic, font_color: fontColor } = settings;

  const lyricTextStyle = {
    fontSize: `${fontSize}px`,
    fontFamily,
    fontWeight: bold ? 700 : 400,
    fontStyle: italic ? 'italic' : 'normal',
    textAlign: 'center',
    padding: `${Math.max(8, fontSize * 0.4)}px 20px`,
    WebkitAppRegion: 'no-drag',
    transition: 'font-size 0.15s ease',
  } as React.CSSProperties;

  const renderLyrics = () => {
    if (!lyrics.text) {
      return (
        <div style={{
          ...lyricTextStyle,
          color: 'var(--text-secondary, rgba(255,255,255,0.5))',
        }}>
          ♪ 暂无歌词 ♪
        </div>
      );
    }

    if (lyrics.isKaraoke && lyrics.words && lyrics.lineTimeMs !== undefined) {
      const lineDuration = lyrics.words.length > 1
        ? lyrics.words[lyrics.words.length - 1].timeMs - lyrics.lineTimeMs
        : 5000;

      return (
        <div style={{
          ...lyricTextStyle,
          textShadow: '0 2px 8px rgba(0,0,0,0.5)',
        }}>
          {lyrics.words.map((word, i) => {
            const wordProgress = lineDuration > 0
              ? (word.timeMs - lyrics.lineTimeMs!) / lineDuration
              : i / Math.max(lyrics.words!.length - 1, 1);
            const isSung = lyrics.progress >= wordProgress;
            return (
              <span
                key={i}
                style={{
                  color: isSung ? fontColor : 'var(--text-secondary, rgba(255,255,255,0.6))',
                  textShadow: isSung ? `0 0 12px ${fontColor}99` : 'none',
                  transition: 'color 0.15s, text-shadow 0.15s',
                }}
              >
                {word.text}
              </span>
            );
          })}
        </div>
      );
    }

    return (
      <div style={{
        ...lyricTextStyle,
        color: fontColor,
        textShadow: `0 0 12px ${fontColor}66, 0 2px 8px rgba(0,0,0,0.5)`,
      }}>
        {lyrics.text}
      </div>
    );
  };

  // 控制栏始终 no-drag，确保锁定时按钮仍可点击（解除锁定）
  const controlBarStyle = {
    position: 'fixed',
    bottom: 4,
    left: '50%',
    transform: 'translateX(-50%)',
    display: 'flex',
    alignItems: 'center',
    gap: 4,
    padding: '4px 10px',
    background: 'rgba(0, 0, 0, 0.5)',
    backdropFilter: 'blur(8px)',
    borderRadius: 12,
    border: '1px solid rgba(255, 255, 255, 0.1)',
    WebkitAppRegion: 'no-drag',
    opacity: hovered ? 1 : 0.35,
    transition: 'opacity 0.2s ease',
    zIndex: 10,
    userSelect: 'none',
  } as React.CSSProperties;

  const btnStyle: React.CSSProperties = {
    background: 'transparent',
    border: 'none',
    color: locked ? 'var(--accent, #a78bfa)' : 'rgba(255, 255, 255, 0.85)',
    cursor: 'pointer',
    width: 28,
    height: 28,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    borderRadius: 8,
    transition: 'background 0.15s, color 0.15s',
    fontSize: 14,
    fontWeight: 700,
  };

  const sizeLabelStyle: React.CSSProperties = {
    color: 'rgba(255, 255, 255, 0.9)',
    fontSize: 12,
    minWidth: 34,
    textAlign: 'center',
    fontVariantNumeric: 'tabular-nums',
  };

  return (
    <div
      style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center' }}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      {renderLyrics()}

      {/* 控制栏：锁定切换 + 字号调节 */}
      <div style={controlBarStyle}>
        <button
          style={btnStyle}
          onClick={toggleLock}
          title={locked ? '已锁定位置（点击解锁可拖动）' : '位置未锁定（点击锁定）'}
          aria-label={locked ? '解锁窗口位置' : '锁定窗口位置'}
        >
          <LockIcon locked={locked} />
        </button>
        <span style={{ width: 1, height: 16, background: 'rgba(255,255,255,0.15)' }} />
        <button
          style={btnStyle}
          onClick={() => changeFontSize(-FONT_STEP)}
          disabled={fontSize <= DESKTOP_LYRICS_FONT_MIN}
          title="缩小字号"
          aria-label="缩小字号"
        >
          －
        </button>
        <span style={sizeLabelStyle} title="当前字号（像素）">{fontSize}px</span>
        <button
          style={btnStyle}
          onClick={() => changeFontSize(FONT_STEP)}
          disabled={fontSize >= DESKTOP_LYRICS_FONT_MAX}
          title="放大字号"
          aria-label="放大字号"
        >
          ＋
        </button>
      </div>
    </div>
  );
}

createRoot(document.getElementById('root')!).render(<DesktopLyrics />);
