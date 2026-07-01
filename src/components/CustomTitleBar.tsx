import { useCallback, useEffect, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import styles from './CustomTitleBar.module.css';

interface Props {
  title: string;
}

/**
 * 自定义标题栏组件。
 *
 * - `data-tauri-drag-region` 使 Tauri 原生支持窗口拖拽
 * - 双击标题栏区域触发最大化/还原（Tauri 原生支持）
 * - 最小化/最大化/关闭按钮调用 Tauri Window API
 * - 视觉样式使用皮肤 CSS 变量（var(--bg-secondary) 等），
 *   随皮肤和主题切换实时更新
 */
export default function CustomTitleBar({ title }: Props) {
  const [maximized, setMaximized] = useState(false);

  const win = getCurrentWindow();

  // 初始化：读取当前最大化状态并监听变化
  useEffect(() => {
    win.isMaximized().then(setMaximized).catch(() => {});
    const unlistenP = win.onResized(() => {
      win.isMaximized().then(setMaximized).catch(() => {});
    });
    return () => { unlistenP.then((fn) => fn()); };
  }, [win]);

  const handleMinimize = useCallback(() => {
    win.minimize().catch(() => {});
  }, [win]);

  const handleToggleMaximize = useCallback(() => {
    win.toggleMaximize().catch(() => {});
  }, [win]);

  const handleClose = useCallback(() => {
    win.close().catch(() => {});
  }, [win]);

  return (
    <div className={styles.titleBar} data-tauri-drag-region>
      <span className={styles.titleText}>{title}</span>
      <div className={styles.controls}>
        {/* 最小化 */}
        <button
          className={styles.btn}
          onClick={handleMinimize}
          type="button"
          title="最小化"
          aria-label="最小化"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
            <line x1="2" y1="6" x2="10" y2="6" />
          </svg>
        </button>

        {/* 最大化/还原 */}
        <button
          className={styles.btn}
          onClick={handleToggleMaximize}
          type="button"
          title={maximized ? '还原' : '最大化'}
          aria-label={maximized ? '还原' : '最大化'}
        >
          {maximized ? (
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round">
              <rect x="2.5" y="4" width="5.5" height="5.5" rx="0.8" />
              <path d="M4 4 V2.5 H9.5 V8 H8" fill="none" />
            </svg>
          ) : (
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round">
              <rect x="2.5" y="2.5" width="7" height="7" rx="0.8" />
            </svg>
          )}
        </button>

        {/* 关闭 */}
        <button
          className={`${styles.btn} ${styles.closeBtn}`}
          onClick={handleClose}
          type="button"
          title="关闭"
          aria-label="关闭"
        >
          <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round">
            <line x1="3" y1="3" x2="9" y2="9" />
            <line x1="9" y1="3" x2="3" y2="9" />
          </svg>
        </button>
      </div>
    </div>
  );
}
