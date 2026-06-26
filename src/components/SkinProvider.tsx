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
 * Inject or update the skin CSS variables <style> element in <head>.
 */
let styleEl: HTMLStyleElement | null = null;

function injectCss(css: string) {
  if (!styleEl) {
    styleEl = document.createElement('style');
    styleEl.id = 'ttplayer-skin-vars';
    document.head.appendChild(styleEl);
  }
  styleEl.textContent = css;
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
    void emitTo('lyrics-desktop', 'skin-changed', { skinId, css }).catch(() => {});
  } catch (e) {
    console.error('Failed to apply skin:', e);
  }
}
