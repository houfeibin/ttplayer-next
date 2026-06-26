import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import styles from './TagEditor.module.css';

interface TagData {
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  year?: number;
  track?: number;
  genre: string;
  comment: string;
}

interface Props {
  filePath: string;
  onClose: () => void;
}

export default function TagEditor({ filePath, onClose }: Props) {
  const [tags, setTags] = useState<TagData>({
    title: '', artist: '', album: '', albumArtist: '',
    genre: '', comment: '',
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    setLoading(true);
    invoke<any>('tags_read', { path: filePath })
      .then((data) => {
        setTags({
          title: data.title || '',
          artist: data.artist || '',
          album: data.album || '',
          albumArtist: data.albumArtist || '',
          year: data.year,
          track: data.track,
          genre: data.genre || '',
          comment: data.comment || '',
        });
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [filePath]);

  const handleSave = async () => {
    setSaving(true);
    setError(null);
    setSaved(false);

    const updates: Record<string, string> = {};
    if (tags.title) updates.title = tags.title;
    if (tags.artist) updates.artist = tags.artist;
    if (tags.album) updates.album = tags.album;
    if (tags.albumArtist) updates.album_artist = tags.albumArtist;
    if (tags.year) updates.year = String(tags.year);
    if (tags.track) updates.track = String(tags.track);
    if (tags.genre) updates.genre = tags.genre;
    if (tags.comment) updates.comment = tags.comment;

    try {
      await invoke('tags_write', { path: filePath, updates });
      setSaved(true);
      setTimeout(() => setSaved(false), 2000);
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const handleChange = (field: keyof TagData, value: string | number) => {
    setTags((prev) => ({ ...prev, [field]: value }));
  };

  const fileName = filePath.split(/[/\\]/).pop() ?? filePath;

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>🏷️ 标签编辑器</span>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>

        <div className={styles.fileName}>{fileName}</div>

        <div className={styles.content}>
          {loading && <div className={styles.loading}>加载中...</div>}
          {error && <div className={styles.error}>{error}</div>}

          {!loading && (
            <div className={styles.form}>
              <div className={styles.row}>
                <label className={styles.label}>标题</label>
                <input
                  className={styles.input}
                  value={tags.title}
                  onChange={(e) => handleChange('title', e.target.value)}
                  placeholder="歌曲标题"
                />
              </div>
              <div className={styles.row}>
                <label className={styles.label}>艺术家</label>
                <input
                  className={styles.input}
                  value={tags.artist}
                  onChange={(e) => handleChange('artist', e.target.value)}
                  placeholder="艺术家"
                />
              </div>
              <div className={styles.row}>
                <label className={styles.label}>专辑</label>
                <input
                  className={styles.input}
                  value={tags.album}
                  onChange={(e) => handleChange('album', e.target.value)}
                  placeholder="专辑名称"
                />
              </div>
              <div className={styles.row}>
                <label className={styles.label}>专辑艺术家</label>
                <input
                  className={styles.input}
                  value={tags.albumArtist}
                  onChange={(e) => handleChange('albumArtist', e.target.value)}
                  placeholder="专辑艺术家"
                />
              </div>
              <div className={styles.rowGroup}>
                <div className={styles.rowHalf}>
                  <label className={styles.label}>年份</label>
                  <input
                    className={styles.input}
                    type="number"
                    value={tags.year ?? ''}
                    onChange={(e) => handleChange('year', e.target.value ? parseInt(e.target.value) : 0)}
                    placeholder="年份"
                  />
                </div>
                <div className={styles.rowHalf}>
                  <label className={styles.label}>音轨号</label>
                  <input
                    className={styles.input}
                    type="number"
                    value={tags.track ?? ''}
                    onChange={(e) => handleChange('track', e.target.value ? parseInt(e.target.value) : 0)}
                    placeholder="音轨号"
                  />
                </div>
              </div>
              <div className={styles.row}>
                <label className={styles.label}>流派</label>
                <input
                  className={styles.input}
                  value={tags.genre}
                  onChange={(e) => handleChange('genre', e.target.value)}
                  placeholder="流派"
                />
              </div>
              <div className={styles.row}>
                <label className={styles.label}>注释</label>
                <textarea
                  className={styles.textarea}
                  value={tags.comment}
                  onChange={(e) => handleChange('comment', e.target.value)}
                  placeholder="注释"
                  rows={3}
                />
              </div>
            </div>
          )}
        </div>

        <div className={styles.footer}>
          {saved && <span className={styles.saved}>✅ 已保存</span>}
          <button className={styles.cancelBtn} onClick={onClose}>取消</button>
          <button
            className={styles.saveBtn}
            onClick={handleSave}
            disabled={saving || loading}
          >
            {saving ? '保存中...' : '💾 保存'}
          </button>
        </div>
      </div>
    </div>
  );
}
