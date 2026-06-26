import { useEffect } from 'react';
import { themeGetMode } from '@/utils/ipc';

type ThemeMode = 'light' | 'dark' | 'system';

function resolveTheme(mode: ThemeMode): 'light' | 'dark' {
  if (mode === 'system') {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }
  return mode;
}

export default function ThemeProvider({ children }: { children: React.ReactNode }) {
  useEffect(() => {
    let currentMode: ThemeMode = 'dark';

    // Apply the resolved theme to <html data-theme>
    const apply = (mode: ThemeMode) => {
      const resolved = resolveTheme(mode);
      document.documentElement.setAttribute('data-theme', resolved);
    };

    // Load persisted mode from backend
    (async () => {
      try {
        const raw = await themeGetMode();
        currentMode = (raw === 'light' || raw === 'system') ? raw : 'dark';
        apply(currentMode);
      } catch {
        // Fall back to dark
        apply('dark');
      }
    })();

    // Listen for OS theme changes (only relevant when mode === 'system')
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const onChange = () => {
      if (currentMode === 'system') {
        apply('system');
      }
    };
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  return <>{children}</>;
}
