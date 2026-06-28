import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { SongMetadata } from '@/stores/player';

export interface PlayerState {
  state: string;
  positionMs: number;
  durationMs: number;
  sampleRate: number;
  channels: number;
  volume: number;
  currentFile: string | null;
  metadata: SongMetadata | null;
  crossfadePending: boolean;
  spectrum: {
    bands: number[];
    peak: number;
  };
}

/** Playback error details emitted by the backend when decoding fails. */
export interface PlaybackError {
  /** Error category (snake_case): unknown_format, decoder_error, io_error, etc. */
  kind: string;
  /** Human-readable error message. */
  message: string;
  /** Path of the track that failed (null if no track was involved). */
  trackPath: string | null;
  /** Unix epoch timestamp (ms) when the error was recorded. */
  timestampMs: number;
}

/** Event payload shape emitted by backend every ~50ms */
export interface PlayerStateEvent {
  state: string;
  positionMs: number;
  durationMs: number;
  volume: number;
  currentFile: string | null;
  metadata: SongMetadata | null;
  crossfadePending: boolean;
  spectrum: {
    bands: number[];
    peak: number;
  };
  /** Lyrics timing update (piggybacked so frontend doesn't poll). */
  lyrics?: {
    index: number | null;
    text: string;
    progress: number;
    totalLines: number;
    changed: boolean;
  };
  /** Playback error (null when no error). Persisted while the error state
   *  is active so the frontend can log details and auto-skip. */
  error?: PlaybackError | null;
}

export interface PlaylistItem {
  path: string;
  format: string;
}

export interface PlaylistData {
  items: PlaylistItem[];
  currentIndex: number;
}

/** Open file dialog and return selected path */
export async function openFileDialog(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{
      name: 'Audio',
      extensions: [
        'flac', 'mp3', 'wav', 'ape', 'tak', 'ogg', 'opus',
        'm4a', 'aac', 'alac', 'wma', 'mpc', 'ac3', 'dts',
        'mod', 'xm', 's3m', 'it',
      ],
    }],
  });
  return selected as string | null;
}

/** Open file dialog with multiple selection enabled. */
export async function openFilesDialog(): Promise<string[]> {
  const selected = await open({
    multiple: true,
    filters: [{
      name: 'Audio',
      extensions: [
        'flac', 'mp3', 'wav', 'ape', 'tak', 'ogg', 'opus',
        'm4a', 'aac', 'alac', 'wma', 'mpc', 'ac3', 'dts',
        'mod', 'xm', 's3m', 'it',
      ],
    }],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected as string[] : [selected as string];
}

/** Open folder dialog and return selected folder path (or null). */
export async function openFolderDialog(): Promise<string | null> {
  const selected = await open({ directory: true, multiple: false });
  return (selected as string | null) ?? null;
}

export async function playFile(path: string): Promise<void> {
  return invoke('player_play', { path });
}

export async function togglePlayPause(): Promise<void> {
  return invoke('player_toggle');
}

export async function pause(): Promise<void> {
  return invoke('player_pause');
}

export async function stop(): Promise<void> {
  return invoke('player_stop');
}

export async function getState(): Promise<PlayerState> {
  return invoke('player_get_state');
}

export async function seek(positionMs: number): Promise<void> {
  return invoke('player_seek', { positionMs });
}

export async function setVolume(volume: number): Promise<void> {
  return invoke('player_set_volume', { volume });
}

export async function addFiles(paths: string[]): Promise<number> {
  return invoke('playlist_add_files', { paths });
}

export async function getPlaylist(): Promise<PlaylistData> {
  return invoke('playlist_get_items');
}

export async function playNext(): Promise<string | null> {
  return invoke('playlist_next');
}

export async function playPrev(): Promise<string | null> {
  return invoke('playlist_prev');
}

export async function playIndex(index: number): Promise<void> {
  return invoke('playlist_play_index', { index });
}

export async function clearPlaylist(): Promise<void> {
  return invoke('playlist_clear');
}

export async function removeTrack(index: number): Promise<void> {
  return invoke('playlist_remove', { index });
}

/** Move a track from `from` index to `to` index (reorder). */
export async function playlistMoveItem(from: number, to: number): Promise<void> {
  return invoke('playlist_move_item', { from, to });
}

/** Recursively scan a folder and add all audio files. Returns count added. */
export async function playlistAddFolder(folder: string): Promise<number> {
  return invoke('playlist_add_folder', { folder });
}

/** Get the current play mode (single/sequential/loop/loop_one/random). */
export async function playlistGetPlayMode(): Promise<string> {
  return invoke('playlist_get_play_mode');
}

/** Set the play mode. */
export async function playlistSetPlayMode(mode: string): Promise<void> {
  return invoke('playlist_set_play_mode', { mode });
}

export async function readTags(path: string): Promise<Record<string, unknown>> {
  return invoke('tags_read', { path });
}

// --- EQ ---

export async function eqGetBands(): Promise<number[]> {
  return invoke('eq_get_bands');
}

export async function eqSetBand(band: number, gainDb: number): Promise<void> {
  return invoke('eq_set_band', { band, gainDb });
}

export async function eqGetPreamp(): Promise<number> {
  return invoke('eq_get_preamp');
}

export async function eqSetPreamp(gainDb: number): Promise<void> {
  return invoke('eq_set_preamp', { gainDb });
}

export async function eqReset(): Promise<void> {
  return invoke('eq_reset');
}

// --- Surround ---

export async function surroundSetWidth(width: number): Promise<void> {
  return invoke('surround_set_width', { width });
}

export async function surroundGetWidth(): Promise<number> {
  return invoke('surround_get_width');
}

// --- Crossfade ---

export async function crossfadeSetDuration(durationMs: number): Promise<void> {
  return invoke('crossfade_set_duration', { durationMs });
}

export async function crossfadeGetDuration(): Promise<number> {
  return invoke('crossfade_get_duration');
}

export async function crossfadeIsPending(): Promise<boolean> {
  return invoke('crossfade_is_pending');
}

// --- Lyrics ---

export async function lyricsLoad(path: string): Promise<boolean> {
  return invoke('lyrics_load', { path });
}

export async function lyricsSearch(audioPath: string): Promise<string[]> {
  return invoke('lyrics_search', { audioPath });
}

export async function lyricsAutoLoad(audioPath: string): Promise<boolean> {
  return invoke('lyrics_auto_load', { audioPath });
}

export interface LyricsUpdate {
  index: number | null;
  text: string;
  progress: number;
  totalLines: number;
  changed: boolean;
}

export async function lyricsUpdate(positionMs: number): Promise<LyricsUpdate> {
  return invoke('lyrics_update', { positionMs });
}

export interface LrcLine {
  timeMs: number;
  text: string;
  words?: { timeMs: number; text: string }[];
}

export async function lyricsGetLines(): Promise<LrcLine[]> {
  return invoke('lyrics_get_lines');
}

export async function lyricsClear(): Promise<void> {
  return invoke('lyrics_clear');
}

export async function lyricsGetMetadata(): Promise<{
  hasLyrics: boolean;
  totalLines: number;
  currentIndex: number | null;
}> {
  return invoke('lyrics_get_metadata');
}

export interface LyricSearchResult {
  id: string;
  title: string;
  artist: string;
  album?: string;
  durationMs?: number;
  source: string;
}

export async function lyricsSearchOnline(keyword: string): Promise<LyricSearchResult[]> {
  return invoke('lyrics_search_online', { keyword });
}

export async function lyricsLoadOnline(source: string, id: string): Promise<boolean> {
  return invoke('lyrics_load_online', { source, id });
}

/** Get the list of configured lyrics server URLs (in priority order). */
export async function lyricsGetServers(): Promise<string[]> {
  return invoke('lyrics_get_servers');
}

/**
 * Replace the lyrics server list.
 * Each entry must be a TTPlayer-compatible LRC server base URL.
 * Servers are queried in order with failover.
 * Returns the resulting (deduplicated, non-empty) server list.
 */
export async function lyricsSetServers(urls: string[]): Promise<string[]> {
  return invoke('lyrics_set_servers', { urls });
}

/** Save the currently loaded lyrics to `{audio_stem}.lrc` in the same directory. */
export async function lyricsSaveToFile(audioPath: string): Promise<string> {
  return invoke('lyrics_save_to_file', { audioPath });
}

// ── Token management ──────────────────────────────────────────────────────

/** Set the API token for online lyrics search. Validates and persists the token. */
export async function lyricsSetToken(token: string): Promise<string> {
  return invoke('lyrics_set_token', { token });
}

/** Get the current API token (empty string if not set). */
export async function lyricsGetToken(): Promise<string> {
  return invoke('lyrics_get_token');
}

/** Check if a token is currently configured. */
export async function lyricsHasToken(): Promise<boolean> {
  return invoke('lyrics_has_token');
}

// ============================================================
// Skin IPC
// ============================================================

export interface SkinInfo {
  id: string;
  name: string;
  version: string;
  author: string;
  description: string;
  isBuiltin: boolean;
  hasImages: boolean;
}

export async function skinList(): Promise<SkinInfo[]> {
  return invoke('skin_list');
}

export async function skinGetCurrent(): Promise<string> {
  return invoke('skin_get_current');
}

export async function skinApply(skinId: string): Promise<string> {
  return invoke('skin_apply', { skinId });
}

export async function skinInstall(path: string): Promise<SkinInfo> {
  return invoke('skin_install', { path });
}

export async function skinGetDir(): Promise<string> {
  return invoke('skin_get_dir');
}

export async function skinDelete(skinId: string): Promise<void> {
  return invoke('skin_delete', { skinId });
}

export async function skinOpenDir(): Promise<void> {
  return invoke('skin_open_dir');
}

// ============================================================
// Theme IPC
// ============================================================

export async function themeGetMode(): Promise<string> {
  return invoke('theme_get_mode');
}

export async function themeSetMode(mode: string): Promise<void> {
  return invoke('theme_set_mode', { mode });
}

// ============================================================
// Desktop Lyrics Settings IPC
// ============================================================

/** 桌面歌词字号范围（与后端 desktop_lyrics.rs 常量保持一致） */
export const DESKTOP_LYRICS_FONT_MIN = 12;
export const DESKTOP_LYRICS_FONT_MAX = 48;
export const DESKTOP_LYRICS_FONT_DEFAULT = 28;
export const DESKTOP_LYRICS_FONT_FAMILY_DEFAULT = 'system-ui, sans-serif';
export const DESKTOP_LYRICS_FONT_COLOR_DEFAULT = '#a78bfa';
/** 窗口不透明度范围与默认值（与后端常量一致） */
export const DESKTOP_LYRICS_OPACITY_MIN = 0.1;
export const DESKTOP_LYRICS_OPACITY_MAX = 1.0;
export const DESKTOP_LYRICS_OPACITY_DEFAULT = 1.0;

export interface DesktopLyricsSettings {
  font_size: number;
  locked: boolean;
  font_family: string;
  bold: boolean;
  italic: boolean;
  font_color: string;
  /** 卡拉OK逐字播放模式 */
  karaoke: boolean;
  /** 显示行数：1=单行，2=双行 */
  line_count: number;
  /** 显示方向："horizontal" 或 "vertical" */
  direction: string;
  /** 窗口不透明度（0.1~1.0） */
  opacity: number;
}

/** 读取桌面歌词设置。 */
export async function desktopLyricsGet(): Promise<DesktopLyricsSettings> {
  return invoke('desktop_lyrics_get');
}

/**
 * 更新桌面歌词设置。任一参数为 `undefined` 时保持原值。后端会持久化并向
 * 所有窗口广播 `desktop-lyrics-settings-changed` 事件，调用方无需自行 emit。
 *
 * 注意：Tauri 2 会把 invoke 参数 key 做 camelCase→snake_case 转换以匹配
 * Rust 命令参数名（如 `fontSize` → `font_size`）。因此这里接收与
 * `DesktopLyricsSettings` 一致的 snake_case 字段，内部映射成 camelCase
 * 后再传给 invoke，否则后端参数匹配不到、变更静默丢失。
 */
export async function desktopLyricsSet(params: {
  font_size?: number;
  locked?: boolean;
  font_family?: string;
  bold?: boolean;
  italic?: boolean;
  font_color?: string;
  karaoke?: boolean;
  line_count?: number;
  direction?: string;
  opacity?: number;
}): Promise<DesktopLyricsSettings> {
  return invoke('desktop_lyrics_set', {
    fontSize: params.font_size,
    locked: params.locked,
    fontFamily: params.font_family,
    bold: params.bold,
    italic: params.italic,
    fontColor: params.font_color,
    karaoke: params.karaoke,
    lineCount: params.line_count,
    direction: params.direction,
    opacity: params.opacity,
  });
}

/** 恢复所有桌面歌词设置到默认值。 */
export async function desktopLyricsReset(): Promise<DesktopLyricsSettings> {
  return invoke('desktop_lyrics_reset');
}

/**
 * 获取鼠标在屏幕上的物理坐标（像素）。
 * 用于桌面歌词锁定后轮询检测鼠标是否在右上角解锁按钮区域，
 * 以便动态切换 `setIgnoreCursorEvents` 实现穿透 + 可交互共存。
 */
export async function getCursorPosition(): Promise<[number, number]> {
  return invoke<[number, number]>('get_cursor_position');
}

// ============================================================
// File Properties IPC
// ============================================================

export interface FileProperties {
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

export async function fileGetProperties(path: string): Promise<FileProperties> {
  return invoke('file_get_properties', { path });
}

// ============================================================
// Tag Write IPC
// ============================================================

export async function tagsWrite(path: string, updates: Record<string, string>): Promise<void> {
  return invoke('tags_write', { path, updates });
}
