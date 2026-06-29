import { create } from 'zustand';

import type { PlayerStateEvent, PlaybackError } from '@/utils/ipc';

// Matches Rust PlaybackState enum
export type PlaybackState =
  | 'Idle'
  | 'Loading'
  | 'Playing'
  | 'Paused'
  | 'Stopped'
  | 'Error';

export type PlayMode =
  | 'Single'
  | 'LoopOne'
  | 'Sequential'
  | 'Loop'
  | 'Random';

export interface SongMetadata {
  title: string;
  artist: string;
  album: string;
  albumArtist: string;
  year: number | null;
  track: number | null;
  genre: string;
  comment: string;
  durationMs: number | null;
  bitRate: number | null;
  sampleRate: number | null;
  channels: number | null;
  bitDepth: number | null;
  coverArt: string | null;
}

/**
 * Active seek guard. Set immediately on user-initiated seek (optimistic UI),
 * cleared once the backend reports a position matching the target — or after
 * a safety timeout. While active, `applyEventPayload` ignores stale position
 * updates from the event-push thread (which may still report the pre-seek
 * position for a few ticks before the backend finishes processing the seek).
 * This prevents the progress bar from briefly snapping back to the old
 * position before jumping to the clicked target.
 */
export interface SeekingTo {
  target: number;
  at: number;
}

interface PlayerStore {
  state: PlaybackState;
  positionMs: number;
  durationMs: number;
  volume: number; // 0-100
  sampleRate: number;
  channels: number;
  currentFile: string | null;
  metadata: SongMetadata;
  spectrum: number[];      // 256 log-spaced magnitude bands 0..1
  spectrumPeak: number;
  /** Last playback error (null when no error). Set when backend reports Error state. */
  error: PlaybackError | null;
  /** Seek guard (null when not seeking). See SeekingTo docs. */
  seekingTo: SeekingTo | null;

  setState: (state: PlaybackState) => void;
  setPosition: (ms: number) => void;
  setDuration: (ms: number) => void;
  setVolume: (vol: number) => void;
  setAudioInfo: (sampleRate: number, channels: number) => void;
  setCurrentFile: (path: string | null) => void;
  setMetadata: (meta: SongMetadata) => void;
  setSpectrum: (bands: number[], peak: number) => void;
  /** Set/clear the seek guard. Pass null to clear. */
  setSeekingTo: (val: SeekingTo | null) => void;
  /** Batch-apply the entire event payload from backend (replaces all per-field setters) */
  applyEventPayload: (payload: PlayerStateEvent) => void;
}

export const usePlayerStore = create<PlayerStore>((set, get) => ({
  state: 'Idle',
  positionMs: 0,
  durationMs: 0,
  volume: 80,
  sampleRate: 0,
  channels: 0,
  currentFile: null,
  metadata: {
    title: '',
    artist: '',
    album: '',
    albumArtist: '',
    year: null,
    track: null,
    genre: '',
    comment: '',
    durationMs: null,
    bitRate: null,
    sampleRate: null,
    channels: null,
    bitDepth: null,
    coverArt: null,
  },
  spectrum: Array(256).fill(0),
  spectrumPeak: 0,
  error: null,
  seekingTo: null,

  setState: (state) => set({ state }),
  setPosition: (ms) => set({ positionMs: ms }),
  setDuration: (ms) => set({ durationMs: ms }),
  setVolume: (vol) => set({ volume: vol }),
  setAudioInfo: (sampleRate, channels) => set({ sampleRate, channels }),
  setCurrentFile: (path) => set({ currentFile: path }),
  setMetadata: (meta) => set({ metadata: meta }),
  setSpectrum: (bands, peak) => set({ spectrum: bands, spectrumPeak: peak }),
  setSeekingTo: (val) => set({ seekingTo: val }),
  applyEventPayload: (p) => {
    const stateMap: Record<string, PlaybackState> = {
      Playing: 'Playing', Paused: 'Paused', Stopped: 'Stopped',
      Idle: 'Idle', Loading: 'Loading', Error: 'Error',
    };
    const cur = get();
    let positionMs = p.positionMs;
    let seekingTo = cur.seekingTo;

    // If the file changed (new track started), invalidate any active seek
    // guard so the new track's position updates aren't filtered.
    const fileChanged = p.currentFile !== cur.currentFile;
    if (fileChanged) {
      seekingTo = null;
    }

    // Seek guard: filter stale position updates. The event-push thread (50ms
    // tick) may still report the pre-seek position for a few ticks after the
    // user clicks, because the backend hasn't finished processing the seek
    // yet. While the guard is active, ignore positions that are far from the
    // seek target; accept (and clear the guard) once the backend reports a
    // position close to the target. A 1s safety timeout ensures the guard
    // can't get stuck if the seek fails.
    if (seekingTo !== null) {
      const SEEK_TOLERANCE_MS = 2000;
      const SEEK_SAFETY_TIMEOUT_MS = 1000;
      const elapsed = Date.now() - seekingTo.at;
      const caughtUp = Math.abs(p.positionMs - seekingTo.target) < SEEK_TOLERANCE_MS;
      if (caughtUp || elapsed > SEEK_SAFETY_TIMEOUT_MS) {
        seekingTo = null;
      } else {
        // Stale position — keep the optimistic value
        positionMs = cur.positionMs;
      }
    }

    set({
      state: stateMap[p.state] ?? 'Idle',
      positionMs,
      durationMs: p.durationMs,
      volume: p.volume,
      currentFile: p.currentFile,
      // 后端 transport.rs 的 is_current 检查已保证 player.metadata() 永远是
      // 当前文件（current_file == tags_path 时才写入）或上一个文件的陈旧 metadata。
      // 因此 payload 中的 metadata 可直接采用：
      //   - 若是新文件已读取的新 metadata → 正确应用（修复切歌竞态：当异步标签
      //     读取在新歌首 tick 前完成时，file_changed 与 metadata_changed 同时为真，
      //     后端只发送一次新 metadata，原 fileChanged 守卫会丢弃它且永不再重发）。
      //   - 若是上一个文件的陈旧 metadata → 与当前 cur.metadata 相同，无副作用。
      metadata: p.metadata ?? cur.metadata,
      spectrum: p.spectrum?.bands ?? cur.spectrum,
      spectrumPeak: p.spectrum?.peak ?? 0,
      // Mirror the backend's error field. When the backend clears the error
      // (on new track start), p.error will be null, clearing the frontend state.
      error: p.error ?? null,
      seekingTo,
    });
  },
}));
