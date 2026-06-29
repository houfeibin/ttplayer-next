import { useEffect } from 'react';
import { emitTo } from '@tauri-apps/api/event';
import { useSkinStore } from '@/stores/skin';
import { skinList, skinGetCurrent, skinApply } from '@/utils/ipc';

/**
 * SkinProvider — injects CSS variables from the active skin into :root.
 * Renders nothing; wraps children to provide skin context.
 */
export default function SkinProvider({ children }: { children: React.ReactNode }) {
  const setAvailableSkins = useSkinStore((s) => s.setAvailableSkins);
  const setCurrentSkinId = useSkinStore((s) => s.setCurrentSkinId);
  const setCssVariables = useSkinStore((s) => s.setCssVariables);

  // Load available skins + current on mount
  useEffect(() => {
    (async () => {
      try {
        const [skins, currentId] = await Promise.all([
          skinList(),
          skinGetCurrent(),
        ]);
        setAvailableSkins(skins);
        setCurrentSkinId(currentId);

        // Apply current skin CSS (images are already embedded as data URIs by Rust)
        const css = await skinApply(currentId);
        setCssVariables(css);
        injectCss(css);
      } catch (e) {
        console.error('Skin init error:', e);
      }
    })();
  }, [setAvailableSkins, setCurrentSkinId, setCssVariables]);

  return <>{children}</>;
}

/**
 * Inject or update the skin CSS variables using a constructed stylesheet.
 *
 * Constructed stylesheets (CSSStyleSheet) bypass CSP `style-src` inline
 * restrictions, so skin CSS keeps applying even when Tauri/Vite injects a
 * nonce in dev mode (which makes 'unsafe-inline' be ignored per CSP spec).
 */
let skinSheet: CSSStyleSheet | null = null;

function injectCss(css: string) {
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
    console.error('[TTPlayer] injectCss failed:', e);
  }
}

/**
 * Emit `skin-changed` to the desktop lyrics window with retry.
 *
 * Tauri 2's `emitTo` can occasionally fail if the target webview is still
 * initializing or the IPC channel is momentarily busy. We retry a few times
 * with backoff so that skin switches reliably reach the desktop lyrics window.
 */
async function emitSkinChanged(payload: { skinId: string; css: string }): Promise<void> {
  const MAX_ATTEMPTS = 3;
  const BASE_DELAY_MS = 80;
  let lastErr: unknown = null;
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
    try {
      await emitTo('lyrics-desktop', 'skin-changed', payload);
      return;
    } catch (e) {
      lastErr = e;
      if (attempt < MAX_ATTEMPTS) {
        await new Promise((r) => setTimeout(r, BASE_DELAY_MS * attempt));
      }
    }
  }
  console.error('[TTPlayer] skin-changed emit failed after retries:', lastErr);
}

/**
 * Public API: apply a skin from any component.
 *
 * Updates the main window's CSS and, if the desktop lyrics window is open,
 * forwards the new CSS to it via a `skin-changed` event so the separate
 * renderer stays in sync.
 */
export async function applySkin(skinId: string): Promise<void> {
  const { setCurrentSkinId, setCssVariables } = useSkinStore.getState();
  try {
    const css = await skinApply(skinId);
    setCurrentSkinId(skinId);
    setCssVariables(css);
    injectCss(css);

    // Notify the desktop lyrics window (if open) to re-inject the new CSS.
    void emitSkinChanged({ skinId, css });
  } catch (e) {
    console.error('Failed to apply skin:', e);
  }
}
