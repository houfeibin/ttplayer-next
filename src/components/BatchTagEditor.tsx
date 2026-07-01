import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  tagsReadBatch,
  tagsWriteBatch,
  type BatchTagResult,
  type BatchTagView,
  type BatchTagEdit,
} from '@/utils/ipc';
import styles from './BatchTagEditor.module.css';

/** Editable fields. Keys mirror the backend `write` keys (album_artist →
 * "album_artist"). `year`/`track` are stringified before sending. */
type FieldKey =
  | 'title'
  | 'artist'
  | 'album'
  | 'albumArtist'
  | 'year'
  | 'track'
  | 'genre'
  | 'comment';

const FIELDS: { key: FieldKey; label: string; backendKey: string; placeholder: string }[] = [
  { key: 'title', label: '标题', backendKey: 'title', placeholder: '歌曲标题' },
  { key: 'artist', label: '艺术家', backendKey: 'artist', placeholder: '艺术家' },
  { key: 'album', label: '专辑', backendKey: 'album', placeholder: '专辑名称' },
  { key: 'albumArtist', label: '专辑艺术家', backendKey: 'album_artist', placeholder: '专辑艺术家' },
  { key: 'year', label: '年份', backendKey: 'year', placeholder: '年份' },
  { key: 'track', label: '音轨号', backendKey: 'track', placeholder: '音轨号' },
  { key: 'genre', label: '流派', backendKey: 'genre', placeholder: '流派' },
  { key: 'comment', label: '注释', backendKey: 'comment', placeholder: '注释' },
];

type WriteMode = 'overwrite' | 'fill_empty';

interface Props {
  paths: string[];
  onClose: () => void;
  /** 独立窗口模式：不渲染 overlay 遮罩，直接全屏铺满。 */
  fullscreen?: boolean;
}

export default function BatchTagEditor({ paths, onClose, fullscreen = false }: Props) {
  const [results, setResults] = useState<BatchTagResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // 多选：用 Set 存 path，默认全选
  const [selected, setSelected] = useState<Set<string>>(new Set(paths));
  // 每个字段的"是否应用"开关 + 当前编辑值
  const [apply, setApply] = useState<Record<FieldKey, boolean>>({
    title: false, artist: false, album: false, albumArtist: false,
    year: false, track: false, genre: false, comment: false,
  });
  const [values, setValues] = useState<Record<FieldKey, string>>({
    title: '', artist: '', album: '', albumArtist: '',
    year: '', track: '', genre: '', comment: '',
  });
  const [writeMode, setWriteMode] = useState<WriteMode>('overwrite');
  const [saveSummary, setSaveSummary] = useState<{ ok: number; err: number; errors: string[] } | null>(null);

  const lastPathsRef = useRef<string>(paths.join('\u0000'));

  // 首次加载：批量读取标签
  useEffect(() => {
    if (paths.length === 0) {
      setLoading(false);
      return;
    }
    const sig = paths.join('\u0000');
    if (sig === lastPathsRef.current && results.length > 0) {
      return; // 已经加载过
    }
    lastPathsRef.current = sig;
    setLoading(true);
    setError(null);
    tagsReadBatch(paths)
      .then((res) => {
        setResults(res);
        setSelected(new Set(paths));
      })
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [paths, results.length]);

  // 切换某文件选中
  const toggleSelect = useCallback((path: string) => {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }, []);

  const selectAll = useCallback(() => setSelected(new Set(paths)), [paths]);
  const selectNone = useCallback(() => setSelected(new Set()), []);

  // 切换字段"应用"开关时，自动用第一个选中文件的原值预填（便捷）
  const toggleApply = useCallback((key: FieldKey) => {
    setApply((prev) => {
      const next = !prev[key];
      if (next && !values[key]) {
        // 首次开启：用第一个选中文件的已有值预填
        const firstSel = paths.find((p) => selected.has(p));
        const r = results.find((x) => x.ok.path === firstSel);
        if (r) {
          const v = r.ok;
          const prefilled =
            key === 'year' ? (v.year ? String(v.year) : '')
            : key === 'track' ? (v.track ? String(v.track) : '')
            : (v as any)[key] ?? '';
          setValues((pv) => ({ ...pv, [key]: prefilled }));
        }
      }
      return { ...prev, [key]: next };
    });
  }, [paths, results, selected, values]);

  const handleChange = useCallback((key: FieldKey, value: string) => {
    setValues((prev) => ({ ...prev, [key]: value }));
  }, []);

  const selectedCount = selected.size;
  const appliedCount = useMemo(() => Object.values(apply).filter(Boolean).length, [apply]);

  const handleSave = useCallback(async () => {
    if (selectedCount === 0 || appliedCount === 0) return;
    setSaving(true);
    setError(null);
    setSaveSummary(null);

    const edits: BatchTagEdit[] = [];
    for (const r of results) {
      if (!selected.has(r.ok.path)) continue;
      if (r.err) continue; // 跳过读取失败的文件
      const updates: Record<string, string> = {};
      const v = r.ok;
      for (const f of FIELDS) {
        if (!apply[f.key]) continue;
        const newVal = values[f.key];
        const curVal =
          f.key === 'year' ? (v.year ? String(v.year) : '')
          : f.key === 'track' ? (v.track ? String(v.track) : '')
          : (v as any)[f.key] ?? '';
        if (writeMode === 'fill_empty' && curVal) continue; // 仅填充空字段
        // 空值也写入（用于清空字段）
        updates[f.backendKey] = newVal;
      }
      if (Object.keys(updates).length > 0) {
        edits.push({ path: r.ok.path, updates });
      }
    }

    if (edits.length === 0) {
      setError('没有需要写入的更改（所选文件均已存在相同值，或"仅填充空字段"模式下无空字段）');
      setSaving(false);
      return;
    }

    try {
      const res = await tagsWriteBatch(edits);
      // 更新本地状态为写入后的最新标签
      setResults((prev) => {
        const map = new Map(res.map((x) => [x.ok.path, x]));
        return prev.map((p) => map.get(p.ok.path) ?? p);
      });
      const okCount = res.filter((r) => !r.err).length;
      const errCount = res.length - okCount;
      const errors = res.filter((r) => r.err).map((r) => `${shortName(r.ok.path)}: ${r.err}`);
      setSaveSummary({ ok: okCount, err: errCount, errors });
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }, [selectedCount, appliedCount, results, selected, apply, values, writeMode]);

  const shortName = (p: string) => p.split(/[/\\]/).pop() ?? p;

  const containerClass = fullscreen ? styles.fullscreenRoot : styles.overlay;
  const dialogClass = fullscreen ? styles.fullscreenDialog : styles.dialog;

  return (
    <div className={containerClass} onClick={fullscreen ? undefined : onClose}>
      <div className={dialogClass} onClick={(e) => e.stopPropagation()}>
        {!fullscreen && (
          <div className={styles.header} data-tauri-drag-region>
            <span className={styles.title}>🏷️ 批量标签编辑</span>
            <button className={styles.closeBtn} onClick={onClose} type="button">✕</button>
          </div>
        )}

        <div className={styles.toolbar}>
          <button className={styles.toolBtn} onClick={selectAll} type="button">全选</button>
          <button className={styles.toolBtn} onClick={selectNone} type="button">取消全选</button>
          <span className={styles.summary}>
            {selectedCount}/{paths.length} 已选 · {appliedCount} 个字段待应用
          </span>
        </div>

        {error && <div className={styles.error} style={{ padding: '8px 16px' }}>{error}</div>}

        <div className={styles.body}>
          {/* 左侧文件列表 */}
          <div className={styles.fileList}>
            <div className={styles.fileListHeader}>
              <span>文件 ({paths.length})</span>
            </div>
            <div className={styles.fileListScroll}>
              {loading && <div className={styles.loading}>加载中...</div>}
              {!loading && results.map((r) => {
                const isSel = selected.has(r.ok.path);
                const hasErr = !!r.err;
                const name = shortName(r.ok.path);
                const v = r.ok;
                const tagSummary = [v.artist, v.album].filter(Boolean).join(' - ');
                return (
                  <div
                    key={r.ok.path}
                    className={`${styles.fileRow} ${isSel ? styles.fileRowSelected : ''} ${hasErr ? styles.fileRowError : ''}`}
                    onClick={() => toggleSelect(r.ok.path)}
                  >
                    <input
                      type="checkbox"
                      className={styles.checkbox}
                      checked={isSel}
                      onChange={() => toggleSelect(r.ok.path)}
                      onClick={(e) => e.stopPropagation()}
                    />
                    <div className={styles.fileInfo}>
                      <span className={styles.fileName} title={r.ok.path}>{name}</span>
                      <span className={styles.fileTags} title={tagSummary}>
                        {hasErr ? `⚠ ${r.err}` : (tagSummary || '（无标签）')}
                      </span>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>

          {/* 右侧编辑面板 */}
          <div className={styles.editPanel}>
            <div className={styles.editHeader}>
              编辑面板 — 勾选要应用的字段，填写新值后点击保存
            </div>
            <div className={styles.editContent}>
              <div className={styles.form}>
                {FIELDS.map((f) => (
                  <div key={f.key} className={styles.row}>
                    <div className={styles.rowHead}>
                      <input
                        type="checkbox"
                        className={styles.applyCheck}
                        checked={apply[f.key]}
                        onChange={() => toggleApply(f.key)}
                      />
                      <label className={styles.label}>{f.label}</label>
                    </div>
                    {f.key === 'comment' ? (
                      <textarea
                        className={`${styles.textarea} ${!apply[f.key] ? styles.inputDisabled : ''}`}
                        value={values[f.key]}
                        onChange={(e) => handleChange(f.key, e.target.value)}
                        placeholder={f.placeholder}
                        rows={2}
                        disabled={!apply[f.key]}
                      />
                    ) : (
                      <input
                        className={`${styles.input} ${!apply[f.key] ? styles.inputDisabled : ''}`}
                        value={values[f.key]}
                        onChange={(e) => handleChange(f.key, e.target.value)}
                        placeholder={f.placeholder}
                        type={f.key === 'year' || f.key === 'track' ? 'number' : 'text'}
                        disabled={!apply[f.key]}
                      />
                    )}
                  </div>
                ))}

                <div className={styles.modeRow}>
                  <label className={styles.modeLabel}>
                    <input
                      type="radio"
                      className={styles.modeRadio}
                      checked={writeMode === 'overwrite'}
                      onChange={() => setWriteMode('overwrite')}
                    />
                    覆盖写入
                  </label>
                  <label className={styles.modeLabel}>
                    <input
                      type="radio"
                      className={styles.modeRadio}
                      checked={writeMode === 'fill_empty'}
                      onChange={() => setWriteMode('fill_empty')}
                    />
                    仅填充空字段（保留已有值）
                  </label>
                </div>
              </div>
            </div>
          </div>
        </div>

        <div className={styles.footer}>
          {saving && <span className={styles.progress}>写入中...</span>}
          {saveSummary && (
            <span className={styles.progress}>
              <span className={saveSummary.err === 0 ? styles.progressOk : styles.progressErr}>
                ✅ 成功 {saveSummary.ok} · {saveSummary.err > 0 ? `❌ 失败 ${saveSummary.err}` : ''}
              </span>
            </span>
          )}
          <button className={styles.cancelBtn} onClick={onClose} type="button" disabled={saving}>
            关闭
          </button>
          <button
            className={styles.saveBtn}
            onClick={handleSave}
            disabled={saving || selectedCount === 0 || appliedCount === 0}
            type="button"
          >
            {saving ? '保存中...' : `💾 应用到 ${selectedCount} 个文件`}
          </button>
        </div>

        {saveSummary && saveSummary.errors.length > 0 && (
          <div style={{ padding: '0 16px 12px' }}>
            <div className={styles.errorList}>
              {saveSummary.errors.map((e, i) => (
                <div key={i} className={styles.errorItem}>{e}</div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
