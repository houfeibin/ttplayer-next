import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import styles from './FilePropertiesDialog.module.css';

interface FileProperties {
  fileName: string;
  filePath: string;
  fileSize: number;
  fileSizeStr: string;
  format: string;
  formatExt: string;
  sampleRate?: number;
  channels?: number;
  bitDepth?: number;
  bitrate?: number;
  durationMs?: number;
  durationStr?: string;
  title?: string;
  artist?: string;
  album?: string;
  albumArtist?: string;
  year?: number;
  track?: number;
  genre?: string;
  comment?: string;
  hasCover: boolean;
  rgTrackGain?: number;
  rgAlbumGain?: number;
}

interface Props {
  filePath: string;
  onClose: () => void;
}

export default function FilePropertiesDialog({ filePath, onClose }: Props) {
  const [props, setProps] = useState<FileProperties | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<'info' | 'tags' | 'technical'>('info');

  useEffect(() => {
    setLoading(true);
    setError(null);
    invoke<FileProperties>('file_get_properties', { path: filePath })
      .then(setProps)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [filePath]);

  const renderRow = (label: string, value: string | number | undefined | null) => {
    if (value === undefined || value === null || value === '') return null;
    return (
      <div className={styles.row} key={label}>
        <span className={styles.label}>{label}</span>
        <span className={styles.value}>{String(value)}</span>
      </div>
    );
  };

  return (
    <div className={styles.overlay} onClick={onClose}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <div className={styles.header}>
          <span className={styles.title}>📄 文件属性</span>
          <button className={styles.closeBtn} onClick={onClose}>✕</button>
        </div>

        <div className={styles.tabs}>
          <button
            className={`${styles.tab} ${activeTab === 'info' ? styles.activeTab : ''}`}
            onClick={() => setActiveTab('info')}
          >🎵 基本信息</button>
          <button
            className={`${styles.tab} ${activeTab === 'tags' ? styles.activeTab : ''}`}
            onClick={() => setActiveTab('tags')}
          >🏷️ 标签</button>
          <button
            className={`${styles.tab} ${activeTab === 'technical' ? styles.activeTab : ''}`}
            onClick={() => setActiveTab('technical')}
          >⚙️ 技术</button>
        </div>

        <div className={styles.content}>
          {loading && <div className={styles.loading}>加载中...</div>}
          {error && <div className={styles.error}>{error}</div>}
          {props && !loading && (
            <>
              {activeTab === 'info' && (
                <div className={styles.section}>
                  {renderRow('文件名', props.fileName)}
                  {renderRow('路径', props.filePath)}
                  {renderRow('格式', `${props.format} (.${props.formatExt})`)}
                  {renderRow('大小', props.fileSizeStr)}
                  {renderRow('时长', props.durationStr)}
                  {renderRow('标题', props.title)}
                  {renderRow('艺术家', props.artist)}
                  {renderRow('专辑', props.album)}
                  {renderRow('专辑艺术家', props.albumArtist)}
                  {renderRow('年份', props.year)}
                  {renderRow('音轨', props.track)}
                  {renderRow('流派', props.genre)}
                  {renderRow('封面', props.hasCover ? '✅ 有' : '❌ 无')}
                </div>
              )}

              {activeTab === 'tags' && (
                <div className={styles.section}>
                  {renderRow('标题 (Title)', props.title)}
                  {renderRow('艺术家 (Artist)', props.artist)}
                  {renderRow('专辑 (Album)', props.album)}
                  {renderRow('专辑艺术家 (Album Artist)', props.albumArtist)}
                  {renderRow('年份 (Year)', props.year)}
                  {renderRow('音轨号 (Track)', props.track)}
                  {renderRow('流派 (Genre)', props.genre)}
                  {renderRow('注释 (Comment)', props.comment)}
                  {renderRow('ReplayGain (Track)', props.rgTrackGain?.toFixed(2) ? `${props.rgTrackGain!.toFixed(2)} dB` : null)}
                  {renderRow('ReplayGain (Album)', props.rgAlbumGain?.toFixed(2) ? `${props.rgAlbumGain!.toFixed(2)} dB` : null)}
                </div>
              )}

              {activeTab === 'technical' && (
                <div className={styles.section}>
                  {renderRow('编码格式', props.format)}
                  {renderRow('采样率', props.sampleRate ? `${props.sampleRate} Hz` : null)}
                  {renderRow('声道数', props.channels)}
                  {renderRow('位深度', props.bitDepth ? `${props.bitDepth} bit` : null)}
                  {renderRow('比特率', props.bitrate ? `${props.bitrate} kbps` : null)}
                  {renderRow('文件大小 (bytes)', props.fileSize.toLocaleString())}
                  {renderRow('文件路径', props.filePath)}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
