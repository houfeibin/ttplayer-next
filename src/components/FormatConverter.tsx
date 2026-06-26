import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import styles from './FormatConverter.module.css';

interface ConvertFormat {
  id: string;
  name: string;
  extension: string;
}

interface ConvertResult {
  input: string;
  output: string;
  success: boolean;
  error?: string;
}

interface Props {
  onClose: () => void;
}

export default function FormatConverter({ onClose }: Props) {
  const [formats, setFormats] = useState<ConvertFormat[]>([]);
  const [selectedFormat, setSelectedFormat] = useState('wav');
  const [bitDepth, setBitDepth] = useState(16);
  const [preserveTags, setPreserveTags] = useState(true);
  const [outputDir, setOutputDir] = useState<string | null>(null);
  const [files, setFiles] = useState<string[]>([]);
  const [results, setResults] = useState<ConvertResult[]>([]);
  const [converting, setConverting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<ConvertFormat[]>('convert_get_formats')
      .then(setFormats)
      .catch(console.error);
  }, []);

  const handleAddFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
        filters: [{
          name: 'Audio',
          extensions: ['mp3', 'flac', 'wav', 'aac', 'm4a', 'ogg', 'opus', 'wma', 'ape', 'ac3', 'eac3', 'mod', 'xm', 's3m', 'it'],
        }],
      });
      if (selected) {
        const newFiles = Array.isArray(selected) ? selected : [selected];
        setFiles((prev) => [...prev, ...newFiles]);
      }
    } catch (e) {
      console.error('Failed to open files:', e);
    }
  };

  const handleSelectOutputDir = async () => {
    try {
      const { open: openDir } = await import('@tauri-apps/plugin-dialog');
      const selected = await openDir({ directory: true });
      if (selected) {
        setOutputDir(selected as string);
      }
    } catch (e) {
      console.error('Failed to select directory:', e);
    }
  };

  const handleRemoveFile = (index: number) => {
    setFiles((prev) => prev.filter((_, i) => i !== index));
  };

  const handleConvert = async () => {
    if (files.length === 0) {
      setError('请先添加要转换的文件');
      return;
    }

    setConverting(true);
    setError(null);
    setResults([]);

    try {
      const res = await invoke<ConvertResult[]>('convert_files', {
        files,
        options: {
          output_format: selectedFormat,
          output_dir: outputDir,
          bit_depth: selectedFormat === 'wav' ? bitDepth : undefined,
          preserve_tags: preserveTags,
        },
      });
      setResults(res);

      const failures = res.filter((r) => !r.success);
      if (failures.length > 0) {
        setError(`${failures.length} 个文件转换失败`);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setConverting(false);
    }
  };

  const fileName = (path: string) => path.split(/[/\\]/).pop() ?? path;

  const successCount = results.filter((r) => r.success).length;
  const failCount = results.filter((r) => !r.success).length;

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>🔄 格式转换器</span>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>

        <div className={styles.content}>
          {/* File list */}
          <div className={styles.section}>
            <div className={styles.sectionHeader}>
              <span>📁 源文件 ({files.length})</span>
              <button className={styles.addBtn} onClick={handleAddFiles}>+ 添加文件</button>
            </div>
            <div className={styles.fileList}>
              {files.length === 0 && (
                <div className={styles.empty}>点击"添加文件"选择要转换的音频文件</div>
              )}
              {files.map((f, i) => (
                <div key={i} className={styles.fileItem}>
                  <span className={styles.fileName}>{fileName(f)}</span>
                  <button className={styles.removeBtn} onClick={() => handleRemoveFile(i)}>✕</button>
                </div>
              ))}
            </div>
          </div>

          {/* Output settings */}
          <div className={styles.section}>
            <span className={styles.sectionTitle}>⚙️ 输出设置</span>

            <div className={styles.row}>
              <label className={styles.label}>输出格式</label>
              <div className={styles.formatBtns}>
                {formats.map((f) => (
                  <button
                    key={f.id}
                    className={`${styles.formatBtn} ${selectedFormat === f.id ? styles.formatBtnActive : ''}`}
                    onClick={() => setSelectedFormat(f.id)}
                  >
                    {f.name}
                  </button>
                ))}
              </div>
            </div>

            {selectedFormat === 'wav' && (
              <div className={styles.row}>
                <label className={styles.label}>位深度</label>
                <div className={styles.formatBtns}>
                  <button
                    className={`${styles.formatBtn} ${bitDepth === 16 ? styles.formatBtnActive : ''}`}
                    onClick={() => setBitDepth(16)}
                  >
                    16-bit
                  </button>
                  <button
                    className={`${styles.formatBtn} ${bitDepth === 24 ? styles.formatBtnActive : ''}`}
                    onClick={() => setBitDepth(24)}
                  >
                    24-bit
                  </button>
                </div>
              </div>
            )}

            <div className={styles.row}>
              <label className={styles.label}>输出目录</label>
              <div className={styles.dirRow}>
                <span className={styles.dirPath}>{outputDir ?? '与源文件相同目录'}</span>
                <button className={styles.dirBtn} onClick={handleSelectOutputDir}>选择</button>
              </div>
            </div>

            <div className={styles.row}>
              <label className={styles.checkboxLabel}>
                <input
                  type="checkbox"
                  checked={preserveTags}
                  onChange={(e) => setPreserveTags(e.target.checked)}
                />
                保留标签信息
              </label>
            </div>
          </div>

          {/* Error */}
          {error && <div className={styles.error}>{error}</div>}

          {/* Results */}
          {results.length > 0 && (
            <div className={styles.section}>
              <span className={styles.sectionTitle}>
                ✅ 转换结果 ({successCount} 成功{failCount > 0 ? `, ${failCount} 失败` : ''})
              </span>
              <div className={styles.resultList}>
                {results.map((r, i) => (
                  <div key={i} className={`${styles.resultItem} ${r.success ? styles.resultSuccess : styles.resultFail}`}>
                    <span className={styles.resultName}>{fileName(r.input)}</span>
                    {r.success ? (
                      <span className={styles.resultArrow}>→ {fileName(r.output)}</span>
                    ) : (
                      <span className={styles.resultError}>{r.error}</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>

        <div className={styles.footer}>
          <button className={styles.cancelBtn} onClick={onClose}>关闭</button>
          <button
            className={styles.convertBtn}
            onClick={handleConvert}
            disabled={converting || files.length === 0}
          >
            {converting ? '转换中...' : `🔄 转换 (${files.length})`}
          </button>
        </div>
      </div>
    </div>
  );
}
