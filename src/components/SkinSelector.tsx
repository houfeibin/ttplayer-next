import { useState, useEffect } from 'react';
import { useSkinStore } from '@/stores/skin';
import { applySkin } from '@/components/SkinProvider';
import { skinGetDir, skinDelete, skinList, skinOpenDir } from '@/utils/ipc';
import styles from './SkinSelector.module.css';

const SKIN_COLORS: Record<string, string> = {
  'default': '#7c3aed',
  'ttplayer-blue': '#4a8db7',
  'green': '#22c55e',
  'rose': '#e11d48',
};

export default function SkinSelector() {
  const currentSkinId = useSkinStore((s) => s.currentSkinId);
  const setAvailableSkins = useSkinStore((s) => s.setAvailableSkins);
  const availableSkins = useSkinStore((s) => s.availableSkins);
  const loading = useSkinStore((s) => s.loading);
  const [switching, setSwitching] = useState<string | null>(null);
  const [skinDir, setSkinDir] = useState('');
  const [deleting, setDeleting] = useState<string | null>(null);

  useEffect(() => {
    skinGetDir().then(setSkinDir).catch(() => {});
  }, []);

  const refreshList = async () => {
    try {
      const skins = await skinList();
      setAvailableSkins(skins);
    } catch { /* ignore */ }
  };

  const handleApply = async (skinId: string) => {
    if (skinId === currentSkinId || switching) return;
    setSwitching(skinId);
    try {
      await applySkin(skinId);
    } finally {
      setSwitching(null);
    }
  };

  const handleDelete = async (e: React.MouseEvent, skinId: string) => {
    e.stopPropagation();
    if (deleting) return;
    if (!confirm(`确定要删除皮肤「${availableSkins.find(s => s.id === skinId)?.name ?? skinId}」吗？`)) return;
    setDeleting(skinId);
    try {
      await skinDelete(skinId);
      await refreshList();
      // If we deleted the current skin, switch to default
      if (skinId === currentSkinId) {
        await applySkin('default');
      }
    } catch (err) {
      alert(`删除失败: ${err}`);
    } finally {
      setDeleting(null);
    }
  };

  const handleOpenDir = async () => {
    try {
      await skinOpenDir();
    } catch (err) {
      alert(`无法打开文件夹: ${err}`);
    }
  };

  return (
    <div className={styles.container}>
      <h3 className={styles.title}>🎨 皮肤</h3>
      <div className={styles.grid}>
        {availableSkins.map((skin) => {
          const isActive = skin.id === currentSkinId;
          const isSwitching = switching === skin.id;
          const isDeleting = deleting === skin.id;
          const accentColor = SKIN_COLORS[skin.id] || '#888';
          const canDelete = skin.id !== 'default';

          return (
            <button
              key={skin.id}
              className={`${styles.card} ${isActive ? styles.active : ''}`}
              onClick={() => handleApply(skin.id)}
              disabled={isSwitching || loading || isDeleting}
              style={{ '--skin-accent': accentColor } as React.CSSProperties}
            >
              <div className={styles.preview}>
                <div className={styles.previewBar} />
                <div className={styles.previewDot} />
              </div>
              <div className={styles.info}>
                <span className={styles.name}>
                  {isSwitching ? '切换中...' : isDeleting ? '删除中...' : skin.name}
                </span>
                <span className={styles.desc}>{skin.description}</span>
              </div>
              {isActive && <span className={styles.badge}>✓</span>}
              {skin.hasImages && <span className={styles.imgBadge}>🖼️</span>}
              {canDelete && (
                <span
                  className={styles.deleteBtn}
                  onClick={(e) => handleDelete(e, skin.id)}
                  title="删除皮肤"
                >
                  ✕
                </span>
              )}
            </button>
          );
        })}
      </div>
      {skinDir && (
        <div className={styles.dirHint}>
          <span className={styles.dirLabel}>📁 皮肤文件夹:</span>
          <code className={styles.dirPath} title={skinDir}>{skinDir}</code>
          <button className={styles.openDirBtn} onClick={handleOpenDir} type="button">打开</button>
        </div>
      )}
    </div>
  );
}
