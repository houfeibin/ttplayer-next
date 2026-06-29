import { useEffect, useState, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize, PhysicalSize, PhysicalPosition, currentMonitor, primaryMonitor } from '@tauri-apps/api/window';
import { createRoot } from 'react-dom/client';
import {
  skinGetCurrent, skinApply, themeGetMode,
  desktopLyricsGet, desktopLyricsSet,
  getCursorPosition,
  DESKTOP_LYRICS_FONT_MIN, DESKTOP_LYRICS_FONT_MAX,
  DESKTOP_LYRICS_FONT_DEFAULT, DESKTOP_LYRICS_FONT_FAMILY_DEFAULT, DESKTOP_LYRICS_FONT_COLOR_DEFAULT,
  DESKTOP_LYRICS_OPACITY_DEFAULT,
  type DesktopLyricsSettings,
} from '@/utils/ipc';

interface LyricsEvent {
  text: string;
  nextText?: string;
  /** In double-line mode, which side is currently being sung. */
  currentSide?: 'left' | 'right';
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
    // Use constructed stylesheet (CSSStyleSheet) instead of a <style> element:
    // constructed stylesheets bypass CSP `style-src` inline restrictions, so they
    // keep working even when Tauri/Vite injects a nonce in dev mode (which makes
    // 'unsafe-inline' be ignored per CSP spec).
    let skinSheet: CSSStyleSheet | null = null;
    let currentMode: ThemeMode = 'dark';

    const applyTheme = (mode: ThemeMode) => {
      const resolved = resolveTheme(mode);
      document.documentElement.setAttribute('data-theme', resolved);
    };

    const injectCss = (css: string) => {
      try {
        if (!skinSheet) {
          skinSheet = new CSSStyleSheet();
        }
        skinSheet.replaceSync(css);
        const sheets = document.adoptedStyleSheets;
        if (!sheets.includes(skinSheet)) {
          document.adoptedStyleSheets = [...sheets, skinSheet];
        }
      } catch (e) {
        console.error('[TTPlayer] desktop-lyrics injectCss failed:', e);
      }
    };

    (async () => {
      try {
        // Load and apply skin CSS
        const id = await skinGetCurrent();
        const css = await skinApply(id);
        injectCss(css);
        console.log('[TTPlayer] desktop-lyrics skin init ok, skinId =', id, 'css len =', css.length);

        // Load and apply theme mode
        const raw = await themeGetMode();
        currentMode = (raw === 'light' || raw === 'system') ? raw : 'dark';
        applyTheme(currentMode);
      } catch (e) {
        console.error('[TTPlayer] desktop-lyrics skin init error:', e);
        applyTheme('dark');
      }
    })();

    // Listen for skin changes from the main window
    const unlistenSkinP = listen<{ skinId: string; css: string }>('skin-changed', (event) => {
      console.log('[TTPlayer] desktop-lyrics received skin-changed, skinId =', event.payload.skinId, 'css len =', event.payload.css?.length);
      if (event.payload && typeof event.payload.css === 'string') {
        injectCss(event.payload.css);
      } else {
        console.warn('[TTPlayer] desktop-lyrics skin-changed payload missing css');
      }
    });
    unlistenSkinP.then(
      () => console.log('[TTPlayer] desktop-lyrics skin-changed listener registered'),
      (e) => console.error('[TTPlayer] desktop-lyrics skin-changed listener FAILED:', e)
    );

    // Listen for theme mode changes from the main window
    const unlistenThemeP = listen<{ mode: string }>('theme-changed', (event) => {
      const mode = event.payload.mode as ThemeMode;
      currentMode = mode;
      applyTheme(mode);
    });
    unlistenThemeP.then(
      () => console.log('[TTPlayer] desktop-lyrics theme-changed listener registered'),
      (e) => console.error('[TTPlayer] desktop-lyrics theme-changed listener FAILED:', e)
    );

    // Listen for OS theme changes when mode === 'system'
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      if (currentMode === 'system') applyTheme('system');
    };
    mq.addEventListener('change', onChange);

    return () => {
      unlistenSkinP.then(fn => fn());
      unlistenThemeP.then(fn => fn());
      if (skinSheet) {
        document.adoptedStyleSheets = document.adoptedStyleSheets.filter(s => s !== skinSheet);
      }
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

// ── 文本宽度测量（基于 canvas measureText，避免 DOM 回流）──
// 用于自动调整桌面歌词窗口尺寸，确保完整显示歌词文本且无截断或溢出。
let _measureCanvas: HTMLCanvasElement | null = null;
function getTextWidth(text: string, font: string): number {
  if (!_measureCanvas) _measureCanvas = document.createElement('canvas');
  const ctx = _measureCanvas.getContext('2d');
  if (!ctx) return text.length * 16; // 回退估算
  ctx.font = font;
  return Math.ceil(ctx.measureText(text).width);
}

// ── 按方向读取持久化几何（逻辑坐标）──
// 横屏/竖屏分别保存，避免互相覆盖。向后兼容旧版单一 key。
function loadDirectionalGeometry(dir: 'horizontal' | 'vertical') {
  const key = `ttplayer:desktop-lyrics-geometry-${dir}`;
  try {
    const raw = localStorage.getItem(key) ?? localStorage.getItem('ttplayer:desktop-lyrics-geometry');
    if (!raw) return null;
    const g = JSON.parse(raw);
    if (typeof g.x === 'number' && typeof g.y === 'number' &&
        typeof g.width === 'number' && typeof g.height === 'number') {
      return g as { x: number; y: number; width: number; height: number };
    }
  } catch { /* ignore */ }
  return null;
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
    karaoke: true,
    line_count: 1,
    direction: 'horizontal',
    opacity: DESKTOP_LYRICS_OPACITY_DEFAULT,
  });
  const [hovered, setHovered] = useState(false);
  const [cornerHovered, setCornerHovered] = useState(false);
  // 标记后端设置是否已加载，避免在默认值→加载值变化时触发方向切换逻辑
  const settingsLoadedRef = useRef(false);

  useSkinCss();

  // 加载持久化设置
  useEffect(() => {
    desktopLyricsGet()
      .then((s) => {
        setSettings(s);
        // 标记设置已加载，同步 displayDirection 和 prevIsVerticalRef，
        // 避免把"默认值→加载值"的变化误识别为方向切换
        settingsLoadedRef.current = true;
        setDisplayDirection(s.direction);
        prevIsVerticalRef.current = s.direction === 'vertical';
      })
      .catch((e) => console.warn('[DesktopLyrics] load settings:', e));
  }, []);

  // 监听设置变更（来自主窗口设置面板或本窗口操作的后端广播），保持双向同步
  useEffect(() => {
    const unlisten = listen<DesktopLyricsSettings>('desktop-lyrics-settings-changed', (event) => {
      setSettings(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // 锁定状态：OS 级鼠标穿透 + 轮询检测角落区域
  //
  // CSS pointer-events:none 仅影响 DOM 事件分发，不阻止 OS 窗口捕获鼠标。
  // 必须使用 Tauri 的 setIgnoreCursorEvents(true) 才能让鼠标穿透到下层应用。
  // 但该 API 是窗口级的——开启后整个窗口都不接收事件，解锁按钮也无法点击。
  //
  // 解决方案：锁定时每 100ms 轮询全局鼠标坐标（Rust 命令 GetCursorPos），
  // 判断鼠标是否在右上角 56×56 区域内：
  //   - 是 → setIgnoreCursorEvents(false)，显示解锁按钮，可点击
  //   - 否 → setIgnoreCursorEvents(true)，鼠标穿透到下层应用
  useEffect(() => {
    const win = getCurrentWindow();

    if (!settings.locked) {
      // 解锁态：恢复拖动 + 正常事件接收
      document.body.style.setProperty('-webkit-app-region', 'drag');
      document.documentElement.style.setProperty('-webkit-app-region', 'drag');
      document.body.style.pointerEvents = 'auto';
      void win.setIgnoreCursorEvents(false);
      // 不重置 cornerHovered：若鼠标正在角落区域内（刚点击解锁），
      // 保持按钮可见，由 onMouseLeave 自然隐藏
      return;
    }

    // 锁定态：禁止拖动 + 启动轮询
    document.body.style.setProperty('-webkit-app-region', 'no-drag');
    document.documentElement.style.setProperty('-webkit-app-region', 'no-drag');
    document.body.style.pointerEvents = 'none';
    void win.setIgnoreCursorEvents(true);

    let polling = true;
    const ZONE_LOGICAL = 56; // 右上角悬停区域边长（逻辑像素）
    // 缓存上次状态，仅在变化时调用 IPC/setState，避免 10Hz 无意义的窗口样式变更和渲染调度
    let lastIgnoreCursor: boolean | null = null;
    let lastCornerHovered: boolean | null = null;
    // 缓存窗口几何（锁定态下窗口不移动，无需每 100ms 查询）
    let cachedPos: { x: number; y: number } | null = null;
    let cachedSize: { width: number; height: number } | null = null;
    let cachedSf: number | null = null;

    const poll = async () => {
      while (polling) {
        try {
          const [cx, cy] = await getCursorPosition();
          // 窗口几何只在首次或缓存失效时查询，减少 IPC 调用
          if (!cachedPos || !cachedSize || cachedSf === null) {
            cachedPos = await win.outerPosition();
            cachedSize = await win.outerSize();
            cachedSf = await win.scaleFactor();
          }
          const pos = cachedPos;
          const size = cachedSize;
          const sf = cachedSf;
          const zoneW = ZONE_LOGICAL * sf;
          const zoneH = ZONE_LOGICAL * sf;
          const zoneLeft = pos.x + size.width - zoneW;
          const zoneRight = pos.x + size.width;
          const zoneTop = pos.y;
          const zoneBottom = pos.y + zoneH;
          const over = cx >= zoneLeft && cx <= zoneRight && cy >= zoneTop && cy <= zoneBottom;
          // 仅在状态变化时调用 IPC 和 setState
          const wantIgnore = !over;
          if (lastIgnoreCursor !== wantIgnore) {
            await win.setIgnoreCursorEvents(wantIgnore);
            lastIgnoreCursor = wantIgnore;
          }
          if (lastCornerHovered !== over) {
            setCornerHovered(over);
            lastCornerHovered = over;
          }
        } catch { /* ignore */ }
        await new Promise(r => setTimeout(r, 100));
      }
    };
    void poll();

    return () => {
      polling = false;
      void win.setIgnoreCursorEvents(false);
    };
  }, [settings.locked]);

  // 窗口位置/大小持久化：移动或缩放时保存几何信息，下次打开时恢复
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const win = getCurrentWindow();
    const save = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(async () => {
        try {
          const pos = await win.outerPosition();
          const size = await win.outerSize();
          const scaleFactor = await win.scaleFactor();
          const x = Math.round(pos.x / scaleFactor);
          const y = Math.round(pos.y / scaleFactor);
          const width = Math.round(size.width / scaleFactor);
          const height = Math.round(size.height / scaleFactor);
          // 按当前显示方向选择 key，横屏/竖屏分别保存，避免互相覆盖
          const geoKey = `ttplayer:desktop-lyrics-geometry-${displayIsVerticalRef.current ? 'vertical' : 'horizontal'}`;
          localStorage.setItem(geoKey,
            JSON.stringify({ x, y, width, height }));
        } catch { /* ignore */ }
      }, 300);
    };
    // 使用 Promise.then(fn => fn()) 模式确保 cleanup 竞态安全：
    // 若组件在 onMoved/onResized 的 Promise resolve 前卸载，
    // 局部变量赋值模式（unlistenMove = fn）会导致 cleanup 命中 undefined 永远泄漏。
    // Promise.then 模式即使 cleanup 先执行，resolve 后仍会调用 fn() 取消订阅。
    const unlistenMoveP = win.onMoved(() => save());
    const unlistenResizeP = win.onResized(() => save());
    return () => {
      if (timer) clearTimeout(timer);
      unlistenMoveP.then((fn: () => void) => fn());
      unlistenResizeP.then((fn: () => void) => fn());
    };
  }, []);

  // 歌词更新监听
  useEffect(() => {
    const unlisten = listen<LyricsEvent>('lyrics-update', (event) => {
      setLyrics(event.payload);
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // ── 丝滑逐字动画：requestAnimationFrame 插值 ──
  // 后端每 ~50ms 推送一次 progress，直接用会产生阶梯感。
  // 用 RAF 每帧向 target 平滑插值，换行时立即吸附。
  // 优化：收敛后停止 RAF 循环，target 变化时重启，避免暂停/无歌词时 60Hz 空转。
  const [displayProgress, setDisplayProgress] = useState(0);
  const displayProgressRef = useRef(0);
  const targetProgressRef = useRef(0);
  const lastIndexRef = useRef<number | null>(null);
  const rafActiveRef = useRef(false);
  const rafIdRef = useRef<number | null>(null);

  const setDisplayProgressBoth = (v: number) => {
    displayProgressRef.current = v;
    setDisplayProgress(v);
  };

  const startRAF = () => {
    if (rafActiveRef.current) return;
    rafActiveRef.current = true;
    const animate = () => {
      const target = targetProgressRef.current;
      const prev = displayProgressRef.current;
      const diff = target - prev;
      if (Math.abs(diff) < 0.03) {
        // 收敛：吸附到 target 并停止循环
        if (prev !== target) setDisplayProgressBoth(target);
        rafActiveRef.current = false;
        return;
      }
      setDisplayProgressBoth(prev + diff * 0.5);
      rafIdRef.current = requestAnimationFrame(animate);
    };
    rafIdRef.current = requestAnimationFrame(animate);
  };

  useEffect(() => {
    if (lyrics.index !== lastIndexRef.current) {
      // 换行：立即吸附
      lastIndexRef.current = lyrics.index;
      setDisplayProgressBoth(lyrics.progress);
      targetProgressRef.current = lyrics.progress;
    } else {
      targetProgressRef.current = lyrics.progress;
    }
    // target 变化时重启 RAF（如果已停止）
    startRAF();
  }, [lyrics.index, lyrics.progress]);

  useEffect(() => {
    return () => {
      rafActiveRef.current = false;
      if (rafIdRef.current !== null) cancelAnimationFrame(rafIdRef.current);
    };
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

  const { font_size: fontSize, locked, font_family: fontFamily, bold, italic, font_color: fontColor, karaoke, line_count: lineCount, direction, opacity } = settings;

  // 歌词主色：未自定义时跟随皮肤强调色（var(--accent)），自定义时使用用户颜色
  const isDefaultFontColor = fontColor === DESKTOP_LYRICS_FONT_COLOR_DEFAULT;
  const activeColor = isDefaultFontColor ? 'var(--accent)' : fontColor;
  const activeGlow60 = isDefaultFontColor ? 'rgba(var(--accent-rgb), 0.6)' : `${fontColor}99`;
  const activeGlow40 = isDefaultFontColor ? 'rgba(var(--accent-rgb), 0.4)' : `${fontColor}66`;

  const isVertical = direction === 'vertical';
  const isDoubleLine = lineCount === 2;

  // displayDirection 滞后于 settings.direction：窗口尺寸/位置调整完成后才更新 CSS 方向，
  // 避免歌词文本先变为竖向、窗口后切换的分步闪烁问题。
  const [displayDirection, setDisplayDirection] = useState(direction);
  const displayIsVertical = displayDirection === 'vertical';
  // ref 镜像 displayIsVertical，供 useEffect([]) 中的 save() 闭包读取最新方向，
  // 避免 save() 捕获首次渲染的过时值导致横屏/竖屏几何写入同一个 key 互相覆盖。
  const displayIsVerticalRef = useRef(displayIsVertical);
  displayIsVerticalRef.current = displayIsVertical;

  // ── 自动调整窗口尺寸 + 方向切换位置管理 ──
  // 仅在方向/字号/字体/行数变化时触发（不含歌词文本变化，避免翻页时窗口跳动）。
  //
  // 方向切换策略：
  //   横→竖：保存当前横屏物理几何（位置+尺寸），将窗口移至当前显示器右侧居中
  //          （窗口垂直中心 = 显示器垂直中心，窗口右边缘距显示器右边缘 MARGIN 逻辑像素）
  //   竖→横：精准恢复切换前的横屏物理几何（位置+尺寸，误差 0 物理像素）
  //
  // 多显示器：使用 win.currentMonitor() 获取窗口所在显示器的边界参数。
  // 精准恢复：保存/恢复均使用物理坐标（PhysicalSize/PhysicalPosition），避免
  //          逻辑↔物理转换的舍入误差。
  //
  // 布局规则（用于计算竖屏/横屏目标尺寸）：
  //   横屏单行：宽 = 文本宽 + 2*padH，高 = fontSize + 2*padV
  //   横屏双行：宽 = max(行1, 行2) + 2*padH，高 = 2*(fontSize + 2*padV)
  //   竖屏单行：宽 = fontSize + 2*padH，高 = 文本宽 + 2*padV
  //   竖屏双行：宽 = 2*(fontSize + 2*padH)，高 = max(行1, 行2) + 2*padV
  // 双行模式下 padH=0（文字顶到窗口左右边缘），单行模式 padH=20。
  const lyricsTextRef = useRef(lyrics.text);
  const lyricsNextTextRef = useRef(lyrics.nextText);
  lyricsTextRef.current = lyrics.text;
  lyricsNextTextRef.current = lyrics.nextText;

  // 记录上次已处理的方向，用于检测方向切换；双向保存几何用于精准恢复
  const prevIsVerticalRef = useRef(isVertical);
  const savedHorizontalGeometryRef = useRef<{ x: number; y: number; width: number; height: number } | null>(null);
  const savedVerticalGeometryRef = useRef<{ x: number; y: number; width: number; height: number } | null>(null);
  // 首次挂载跳过自动 resize：使用 localStorage 中保存的几何作为默认尺寸，
  // 确保用户已调整的尺寸参数在重启后仍能保持。
  const isFirstRunRef = useRef(true);

  // 竖屏右侧边距（逻辑像素）
  const VERTICAL_RIGHT_MARGIN = 20;

  useEffect(() => {
    // 设置未加载前不执行（避免默认值→加载值的变化触发方向切换）
    if (!settingsLoadedRef.current) return;
    // 首次（设置加载后）：跳过自动 resize，保留 localStorage 中恢复的初始尺寸
    if (isFirstRunRef.current) {
      isFirstRunRef.current = false;
      prevIsVerticalRef.current = isVertical;
      return;
    }

    const win = getCurrentWindow();
    // 检测方向切换（ref 在 timeout 回调中更新，兼容 React StrictMode 双调用）
    const directionChanged = prevIsVerticalRef.current !== isVertical;
    const switchedToVertical = directionChanged && isVertical;
    const switchedToHorizontal = directionChanged && !isVertical;

    const padH = isDoubleLine ? 0 : 20;
    const padV = Math.max(8, fontSize * 0.4);
    const font = `${italic ? 'italic ' : ''}${bold ? 700 : 400} ${fontSize}px ${fontFamily}`;

    // 无歌词时使用占位文本测量，保证窗口不缩成 0
    const text1 = lyricsTextRef.current || '♪ 暂无歌词 ♪';
    const text2 = lyricsNextTextRef.current || '';
    const hasLine2 = isDoubleLine && !!text2;

    const w1 = getTextWidth(text1, font);
    const w2 = hasLine2 ? getTextWidth(text2, font) : 0;

    let width: number;
    let height: number;

    if (isVertical) {
      // 竖屏：writing-mode vertical-rl，每行宽度 ≈ fontSize
      const lineCount = hasLine2 ? 2 : 1;
      width = lineCount * (fontSize + 2 * padH);
      height = Math.max(w1, w2) + 2 * padV;
    } else {
      // 横屏：每行高度 ≈ fontSize
      const lineCount = hasLine2 ? 2 : 1;
      // 安全余量：canvas measureText 与 WebView 实际文本渲染宽度存在子像素差异
      // （生产环境字体 hinting/抗锯齿可能让实际宽度略大于测量值），单行模式下
      // padH=20 已有空间，但仍需额外余量防止左侧文字被截断。余量按字号比例缩放。
      const safetyPad = Math.max(8, fontSize * 0.25);
      width = Math.max(w1, w2) + 2 * padH + safetyPad;
      height = lineCount * (fontSize + 2 * padV);
    }

    // 最小尺寸：确保右上角 56×56 悬停区域有空间
    width = Math.max(width, 60);
    height = Math.max(height, 60);

    // 方向切换时立即执行（0ms），其他变化（字号/字体/行数）用 300ms 防抖
    const delay = directionChanged ? 0 : 300;

    const timer = setTimeout(async () => {
      try {
        if (switchedToVertical) {
          // ── 横→竖：保存横屏物理几何 ──
          const pos = await win.outerPosition();
          const size = await win.outerSize();
          savedHorizontalGeometryRef.current = {
            x: pos.x, y: pos.y, width: size.width, height: size.height,
          };

          // 优先精准恢复上次竖屏几何；无保存则从 localStorage 跨会话恢复；均无则计算右侧居中
          const savedV = savedVerticalGeometryRef.current;
          if (savedV) {
            // 同会话：物理坐标精准恢复（0 像素误差）
            await win.setSize(new PhysicalSize(savedV.width, savedV.height));
            await win.setPosition(new PhysicalPosition(savedV.x, savedV.y));
          } else {
            const sf = await win.scaleFactor();
            // 跨会话：从 localStorage 读取逻辑坐标，转换为物理坐标恢复
            const savedVLog = loadDirectionalGeometry('vertical');
            if (savedVLog) {
              await win.setSize(new PhysicalSize(
                Math.round(savedVLog.width * sf), Math.round(savedVLog.height * sf)));
              await win.setPosition(new PhysicalPosition(
                Math.round(savedVLog.x * sf), Math.round(savedVLog.y * sf)));
            } else {
              // 无保存几何，计算右侧居中位置
              const winWidthPhys = Math.round(width * sf);
              const winHeightPhys = Math.round(height * sf);

              // 获取窗口所在显示器边界（物理坐标）
              // currentMonitor() 在某些时序下可能返回 null，用 primaryMonitor() 兜底
              const monitor = await currentMonitor().catch(() => null)
                ?? await primaryMonitor().catch(() => null);
              if (monitor) {
                const monRightPhys = monitor.position.x + monitor.size.width;
                const monTopPhys = monitor.position.y;
                const monBottomPhys = monitor.position.y + monitor.size.height;
                const marginPhys = Math.round(VERTICAL_RIGHT_MARGIN * sf);
                // 右边缘距显示器右边缘 MARGIN；垂直居中
                const xPhys = monRightPhys - winWidthPhys - marginPhys;
                const yPhys = monTopPhys + Math.round((monBottomPhys - monTopPhys - winHeightPhys) / 2);
                await win.setSize(new PhysicalSize(winWidthPhys, winHeightPhys));
                await win.setPosition(new PhysicalPosition(xPhys, yPhys));
              } else {
                await win.setSize(new LogicalSize(Math.round(width), Math.round(height)));
              }
            }
          }
        } else if (switchedToHorizontal) {
          // ── 竖→横：保存竖屏物理几何 ──
          const pos = await win.outerPosition();
          const size = await win.outerSize();
          savedVerticalGeometryRef.current = {
            x: pos.x, y: pos.y, width: size.width, height: size.height,
          };

          // 精准恢复横屏几何：同会话用物理坐标（0 像素误差），跨会话从 localStorage 恢复
          const saved = savedHorizontalGeometryRef.current;
          if (saved) {
            await win.setSize(new PhysicalSize(saved.width, saved.height));
            await win.setPosition(new PhysicalPosition(saved.x, saved.y));
          } else {
            // 跨会话：从 localStorage 读取逻辑坐标，转换为物理坐标恢复
            const sf = await win.scaleFactor();
            const savedHLog = loadDirectionalGeometry('horizontal');
            if (savedHLog) {
              await win.setSize(new PhysicalSize(
                Math.round(savedHLog.width * sf), Math.round(savedHLog.height * sf)));
              await win.setPosition(new PhysicalPosition(
                Math.round(savedHLog.x * sf), Math.round(savedHLog.y * sf)));
            } else {
              // 无保存几何，使用计算尺寸
              await win.setSize(new LogicalSize(Math.round(width), Math.round(height)));
            }
          }
        } else {
          // ── 非方向切换（字号/字体/行数变化）：仅调整尺寸 ──
          await win.setSize(new LogicalSize(Math.round(width), Math.round(height)));
        }
        // 标记当前方向已处理
        prevIsVerticalRef.current = isVertical;
        // 窗口尺寸/位置就位后，同步 CSS 方向，避免文本先变竖向、窗口后切换
        if (directionChanged) {
          setDisplayDirection(direction);
        }
      } catch (e) {
        console.warn('[DesktopLyrics] resize/position:', e);
      }
    }, delay);

    return () => clearTimeout(timer);
    // 故意排除 lyrics.text / lyrics.nextText / isDoubleLine：
    // - 翻页不应触发 resize
    // - 单双行切换时保持窗口原始尺寸不变（不拉伸/压缩/重排），
    //   单行模式文本在现有区域内水平居中
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fontSize, fontFamily, bold, italic, isVertical]);

  const lyricTextStyle = {
    fontSize: `${fontSize}px`,
    fontFamily,
    fontWeight: bold ? 700 : 400,
    fontStyle: italic ? 'italic' : 'normal',
    textAlign: 'center',
    padding: `${Math.max(8, fontSize * 0.4)}px 20px`,
    // 解锁时整个页面均可拖动窗口（body 为 drag），歌词区域跟随继承即可
    // 锁定时必须为 no-drag，否则 -webkit-app-region:drag 会在窗口层级
    // 捕获鼠标事件，导致 body 的 pointer-events:none 穿透失效
    WebkitAppRegion: locked ? 'no-drag' : 'drag',
    transition: 'font-size 0.15s ease, padding 0.2s ease, color 0.2s ease',
    // 使用 displayIsVertical（滞后于 settings.direction）：
    // 窗口尺寸/位置调整完成后才切换 CSS 方向，避免分步闪烁
    ...(displayIsVertical ? { writingMode: 'vertical-rl' as const } : {}),
  } as React.CSSProperties;

  /** Render a single line — either karaoke word-by-word or whole-line. */
  const renderLine = (
    lineText: string,
    isCurrent: boolean,
    options?: {
      textAlign?: 'left' | 'right' | 'center';
      alignSelf?: 'flex-start' | 'flex-end' | 'center' | 'auto';
    },
  ) => {
    const align = options?.textAlign ?? 'center';
    // 双行模式下去掉水平 padding，让文字顶到窗口边缘
    const hPad = options?.textAlign !== undefined ? '0px' : '20px';
    const alignSelf = options?.alignSelf ?? 'auto';

    // Karaoke word-by-word mode: only for current line when enabled and words available
    if (isCurrent && karaoke && lyrics.isKaraoke && lyrics.words && lyrics.lineTimeMs !== undefined) {
      const lineDuration = lyrics.words.length > 1
        ? lyrics.words[lyrics.words.length - 1].timeMs - lyrics.lineTimeMs
        : 5000;

      return (
        <div style={{
          ...lyricTextStyle,
          textAlign: align,
          alignSelf,
          padding: `${Math.max(8, fontSize * 0.4)}px ${hPad}`,
          textShadow: '0 2px 8px rgba(0,0,0,0.5)',
        }}>
          {lyrics.words.map((word, i) => {
            const wordProgress = lineDuration > 0
              ? (word.timeMs - lyrics.lineTimeMs!) / lineDuration
              : i / Math.max(lyrics.words!.length - 1, 1);
            const isSung = displayProgress >= wordProgress;
            return (
              <span
                key={i}
                style={{
                  color: isSung ? activeColor : 'var(--text-secondary, rgba(255,255,255,0.6))',
                  textShadow: isSung ? `0 0 12px ${activeGlow60}` : 'none',
                  transition: 'color 0.08s linear, text-shadow 0.08s linear',
                }}
              >
                {word.text}
              </span>
            );
          })}
        </div>
      );
    }

    // Whole-line mode (non-karaoke, or next line in double-line mode)
    return (
      <div style={{
        ...lyricTextStyle,
        textAlign: align,
        alignSelf,
        padding: `${Math.max(8, fontSize * 0.4)}px ${hPad}`,
        color: isCurrent ? activeColor : 'var(--text-secondary, rgba(255,255,255,0.5))',
        textShadow: isCurrent
          ? `0 0 12px ${activeGlow40}, 0 2px 8px rgba(0,0,0,0.5)`
          : '0 2px 8px rgba(0,0,0,0.5)',
        opacity: isCurrent ? 1 : 0.7,
      }}>
        {lineText}
      </div>
    );
  };

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

    if (isDoubleLine) {
      // Page-flip double-line: lines grouped in pairs (0+1, 2+3, …).
      // The pair stays until both lines are sung, then flips to the next pair.
      // currentSide indicates which line is being sung — the other is dimmed.
      //
      // 横屏：双行上下排列，第一行左对齐，第二行右对齐
      // 竖屏：双行左右排列（row-reverse，第一行在右），第一行顶部对齐，
      //       第二行底部对齐（通过 alignSelf 控制）
      const leftIsCurrent = lyrics.currentSide !== 'right';

      const containerStyle: React.CSSProperties = displayIsVertical
        ? {
            display: 'flex',
            flexDirection: 'row-reverse',
            alignItems: 'stretch',
            justifyContent: 'center',
            height: '100%',
            transition: 'padding 0.2s ease',
          }
        : {
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'stretch',
            justifyContent: 'center',
            width: '100%',
            transition: 'padding 0.2s ease',
          };

      return (
        <div style={containerStyle}>
          {renderLine(lyrics.text, leftIsCurrent,
            displayIsVertical ? { alignSelf: 'flex-start' } : { textAlign: 'left' })}
          {lyrics.nextText && renderLine(lyrics.nextText, !leftIsCurrent,
            displayIsVertical ? { alignSelf: 'flex-end' } : { textAlign: 'right' })}
        </div>
      );
    }

    // Single-line mode
    return renderLine(lyrics.text, true);
  };

  // 控制栏：未锁定且悬停时可见
  const controlBarStyle = {
    position: 'fixed',
    bottom: 4,
    left: '50%',
    transform: 'translateX(-50%)',
    display: 'flex',
    alignItems: 'center',
    gap: 4,
    padding: '4px 10px',
    background: 'var(--panel-bg)',
    backdropFilter: hovered ? 'blur(8px)' : 'none',
    borderRadius: 12,
    border: hovered ? `1px solid var(--border-color)` : 'none',
    WebkitAppRegion: 'no-drag',
    opacity: hovered ? 1 : 0,
    pointerEvents: hovered ? 'auto' : 'none',
    transition: 'opacity 0.2s ease',
    zIndex: 10,
    userSelect: 'none',
  } as React.CSSProperties;

  const btnStyle: React.CSSProperties = {
    background: 'transparent',
    border: 'none',
    color: locked ? 'var(--accent)' : 'var(--text-primary)',
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
    color: 'var(--text-primary)',
    fontSize: 12,
    minWidth: 34,
    textAlign: 'center',
    fontVariantNumeric: 'tabular-nums',
  };

  const handleClose = useCallback(() => {
    void getCurrentWindow().close();
  }, []);

  // 右上角悬停区域：包含锁定/解锁 + 关闭按钮，默认隐藏，鼠标移入显示
  // 区域尺寸 56x56，锁定后轮询检测鼠标进入此区域时显示解锁按钮。
  const cornerZoneStyle = {
    position: 'fixed',
    top: 0,
    right: 0,
    display: 'flex',
    alignItems: 'flex-start',
    gap: 2,
    padding: 8,
    minWidth: 56,
    minHeight: 56,
    pointerEvents: 'auto' as const,
    WebkitAppRegion: 'no-drag',
    zIndex: 20,
  } as React.CSSProperties;

  const cornerBtnBase = {
    width: 28,
    height: 28,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    borderRadius: 6,
    border: 'none',
    background: 'var(--button-bg)',
    backdropFilter: 'blur(8px)',
    color: 'var(--text-primary)',
    cursor: 'pointer',
    padding: 0,
    WebkitAppRegion: 'no-drag',
    opacity: cornerHovered ? 1 : 0,
    transition: 'opacity 0.2s ease, background 0.15s, color 0.15s',
    pointerEvents: cornerHovered ? 'auto' : 'none',
    fontSize: 14,
    fontWeight: 700,
  } as React.CSSProperties;

  // 边框可见性：未锁定且悬停时显示
  const borderVisible = !locked && hovered;

  return (
    <div
      style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        boxSizing: 'border-box',
        border: borderVisible
          ? '1px solid var(--border-color)'
          : 'none',
        borderRadius: borderVisible ? 8 : 0,
        outline: 'none',
        transition: 'border 0.2s ease, border-radius 0.2s ease',
      }}
      onMouseEnter={() => { if (!locked) setHovered(true); }}
      onMouseLeave={() => setHovered(false)}
    >
      {/* 歌词内容：应用用户设置的不透明度；控制栏/角落按钮保持全不透明以保证可交互性 */}
      <div style={{
        width: '100%',
        height: '100%',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        opacity,
        transition: 'opacity 0.2s ease',
      }}>
        {renderLyrics()}
      </div>

      {/* 右上角悬停区域：锁定/解锁 + 关闭按钮 */}
      <div
        style={cornerZoneStyle}
        onMouseEnter={() => setCornerHovered(true)}
        onMouseLeave={() => setCornerHovered(false)}
      >
        {/* 锁定/解锁按钮 */}
        <button
          style={cornerBtnBase}
          onClick={toggleLock}
          title={locked ? '点击解锁窗口' : '锁定窗口位置（锁定后鼠标穿透）'}
          aria-label={locked ? '解锁窗口位置' : '锁定窗口位置'}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--accent-hover)';
            e.currentTarget.style.color = 'var(--text-primary)';
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'var(--button-bg)';
            e.currentTarget.style.color = 'var(--text-primary)';
          }}
        >
          <LockIcon locked={locked} />
        </button>

        {/* 关闭按钮（仅未锁定时显示） */}
        {!locked && (
          <button
            style={cornerBtnBase}
            onClick={handleClose}
            title="关闭桌面歌词"
            aria-label="关闭桌面歌词"
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'rgba(239, 68, 68, 0.85)';
              e.currentTarget.style.color = 'var(--text-primary)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'var(--button-bg)';
              e.currentTarget.style.color = 'var(--text-primary)';
            }}
          >
            ✕
          </button>
        )}
      </div>

      {/* 未锁定态：控制栏（字号调节） */}
      {!locked && (
        <div style={controlBarStyle}>
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
      )}
    </div>
  );
}

createRoot(document.getElementById('root')!).render(<DesktopLyrics />);
