import React from 'react';
import ReactDOM from 'react-dom/client';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useState, useEffect } from 'react';
import BatchTagEditor from '@/components/BatchTagEditor';
import CustomTitleBar from '@/components/CustomTitleBar';
import { skinGetCurrent, skinApply, themeGetMode } from '@/utils/ipc';
import '@/styles/global.css';

type ThemeMode = 'light' | 'dark' | 'system';

function resolveTheme(mode: ThemeMode): 'light' | 'dark' {
  if (mode === 'system') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return mode;
}

/**
 * 皮肤 CSS 同步 hook（与 desktop-lyrics.tsx 的 useSkinCss 一致）。
 *
 * 使用 CSSStyleSheet 构造样式表注入，绕过 CSP inline 限制。
 * 监听 `skin-changed` 和 `theme-changed` 事件实时同步主窗口的皮肤和主题。
 */
function useSkinCss() {
  useEffect(() => {
    let skinSheet: CSSStyleSheet | null = null;
    let currentMode: ThemeMode = 'dark';

    const applyTheme = (mode: ThemeMode) => {
      const resolved = resolveTheme(mode);
      document.documentElement.setAttribute('data-theme', resolved);
    };

    const injectCss = (css: string) => {
      try {
        if (!skinSheet) skinSheet = new CSSStyleSheet();
        skinSheet.replaceSync(css);
        const sheets = document.adoptedStyleSheets;
        if (!sheets.includes(skinSheet)) {
          document.adoptedStyleSheets = [...sheets, skinSheet];
        }
      } catch (e) {
        console.error('[TTPlayer] batch-editor injectCss failed:', e);
      }
    };

    (async () => {
      try {
        const id = await skinGetCurrent();
        const css = await skinApply(id);
        injectCss(css);

        const raw = await themeGetMode();
        currentMode = (raw === 'light' || raw === 'system') ? raw : 'dark';
        applyTheme(currentMode);
      } catch (e) {
        console.error('[TTPlayer] batch-editor skin init error:', e);
        applyTheme('dark');
      }
    })();

    const unlistenSkinP = listen<{ skinId: string; css: string }>('skin-changed', (event) => {
      if (event.payload && typeof event.payload.css === 'string') {
        injectCss(event.payload.css);
      }
    });

    const unlistenThemeP = listen<{ mode: string }>('theme-changed', (event) => {
      currentMode = event.payload.mode as ThemeMode;
      applyTheme(currentMode);
    });

    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      if (currentMode === 'system') applyTheme('system');
    };
    mq.addEventListener('change', onChange);

    return () => {
      unlistenSkinP.then((fn) => fn());
      unlistenThemeP.then((fn) => fn());
      if (skinSheet) {
        document.adoptedStyleSheets = document.adoptedStyleSheets.filter((s) => s !== skinSheet);
      }
      mq.removeEventListener('change', onChange);
    };
  }, []);
}

function BatchEditorWindow() {
  const [paths, setPaths] = useState<string[] | null>(null);

  // 初始化皮肤和主题同步
  useSkinCss();

  useEffect(() => {
    // 接收主窗口传递的文件路径列表
    const unlistenP = listen<string[]>('batch-edit-paths', (event) => {
      setPaths(event.payload);
    });

    // 通知主窗口：批量编辑窗口已准备好接收 paths
    void getCurrentWindow().emit('batch-editor-ready', {});

    return () => {
      unlistenP.then((fn) => fn());
    };
  }, []);

  const handleClose = async () => {
    await getCurrentWindow().close();
  };

  if (!paths) {
    return (
      <>
        <CustomTitleBar title="批量标签编辑" />
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: 'calc(100% - 36px)', color: 'var(--text-secondary)' }}>
          等待文件列表...
        </div>
      </>
    );
  }

  return (
    <>
      <CustomTitleBar title="批量标签编辑" />
      <BatchTagEditor paths={paths} onClose={handleClose} fullscreen />
    </>
  );
}

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <BatchEditorWindow />
  </React.StrictMode>,
);
