import { useCallback, useState } from 'react';
import { usePlayerStore } from '@/stores/player';
import {
  playFile, addFiles, openFileDialog, openFilesDialog, openFolderDialog,
  playlistAddFolder, seek, setVolume,
} from '@/utils/ipc';

/**
 * File open, drag-drop, progress seek, volume change, playlist double-click.
 */
export function usePlaybackActions() {
  const durationMs = usePlayerStore((s) => s.durationMs);
  const positionMs = usePlayerStore((s) => s.positionMs);
  const storeSetVolume = usePlayerStore((s) => s.setVolume);
  const [dragFiles, setDragFiles] = useState(false);

  const handleOpenFile = useCallback(async () => {
    const path = await openFileDialog();
    if (path) { await addFiles([path]); await playFile(path); }
  }, []);

  /** 打开多选文件对话框并加入播放列表，自动播放首个。 */
  const handleOpenFiles = useCallback(async () => {
    const paths = await openFilesDialog();
    if (paths.length === 0) return;
    await addFiles(paths);
    await playFile(paths[0]);
  }, []);

  /** 打开文件夹对话框，递归扫描音频并加入播放列表，自动播放首个。 */
  const handleOpenFolder = useCallback(async () => {
    const folder = await openFolderDialog();
    if (!folder) return;
    const count = await playlistAddFolder(folder);
    if (count > 0) {
      // 读回播放列表取第一个（folder 可能扫描出多个，取首项播放）
      const { getPlaylist } = await import('@/utils/ipc');
      const pl = await getPlaylist();
      const firstPath = pl.items[0]?.path;
      if (firstPath) await playFile(firstPath);
    }
  }, []);

  const handleProgressClick = useCallback(async (e: React.MouseEvent<HTMLDivElement>) => {
    if (durationMs <= 0) return;
    const rect = e.currentTarget.getBoundingClientRect();
    await seek(Math.round(((e.clientX - rect.left) / rect.width) * durationMs));
  }, [durationMs]);

  const handleDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault(); setDragFiles(false);
    const files = Array.from(e.dataTransfer.files)
      .map((f) => (f as unknown as { path: string }).path)
      .filter(Boolean);
    if (files.length === 0) return;
    const count = await addFiles(files);
    if (count > 0) await playFile(files[0]);
  }, []);

  const handleVolumeChange = useCallback(async (e: React.ChangeEvent<HTMLInputElement>) => {
    const vol = parseInt(e.target.value);
    storeSetVolume(vol);
    await setVolume(vol);
  }, [storeSetVolume]);

  const handlePlaylistDblClick = useCallback(async (path: string) => {
    await playFile(path);
  }, []);

  const formatTime = useCallback((ms: number) => {
    const s = Math.floor(ms / 1000);
    return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
  }, []);

  const progressPercent = durationMs > 0 ? Math.min(100, (positionMs / durationMs) * 100) : 0;

  return {
    dragFiles, setDragFiles,
    handleOpenFile, handleOpenFiles, handleOpenFolder,
    handleProgressClick, handleDrop,
    handleVolumeChange, handlePlaylistDblClick,
    formatTime, progressPercent,
  };
}
