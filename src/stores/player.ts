import { create } from 'zustand';

import type { PlayerStateEvent } from '@/utils/ipc';

// Matches Rust PlaybackState enum
export type PlaybackState =
  | 'Idle'
  | 'Loading'
  | 'Playing'
  | 'Paused'
  | 'Stopped';

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

  setState: (state: PlaybackState) => void;
  setPosition: (ms: number) => void;
  setDuration: (ms: number) => void;
  setVolume: (vol: number) => void;
  setAudioInfo: (sampleRate: number, channels: number) => void;
  setCurrentFile: (path: string | null) => void;
  setMetadata: (meta: SongMetadata) => void;
  setSpectrum: (bands: number[], peak: number) => void;
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

  setState: (state) => set({ state }),
  setPosition: (ms) => set({ positionMs: ms }),
  setDuration: (ms) => set({ durationMs: ms }),
  setVolume: (vol) => set({ volume: vol }),
  setAudioInfo: (sampleRate, channels) => set({ sampleRate, channels }),
  setCurrentFile: (path) => set({ currentFile: path }),
  setMetadata: (meta) => set({ metadata: meta }),
  setSpectrum: (bands, peak) => set({ spectrum: bands, spectrumPeak: peak }),
  applyEventPayload: (p) => {
    const stateMap: Record<string, PlaybackState> = {
      Playing: 'Playing', Paused: 'Paused', Stopped: 'Stopped', Idle: 'Idle', Loading: 'Loading',
    };
    set({
      state: stateMap[p.state] ?? 'Idle',
      positionMs: p.positionMs,
      durationMs: p.durationMs,
      volume: p.volume,
      currentFile: p.currentFile,
      metadata: p.metadata ?? get().metadata,
      spectrum: p.spectrum?.bands ?? get().spectrum,
      spectrumPeak: p.spectrum?.peak ?? 0,
    });
  },
}));
