import { create } from 'zustand';
import type { LrcLine, LyricSearchResult } from '@/utils/ipc';

interface LyricsState {
  lines: LrcLine[];
  currentIndex: number | null;
  progress: number;
  hasLyrics: boolean;
  offset: number;
  onlineResults: LyricSearchResult[];
  isSearching: boolean;

  // Style config
  fontSize: number;
  lineHeight: number;
  activeColor: string;
  inactiveColor: string;

  setLines: (lines: LrcLine[]) => void;
  setCurrentIndex: (index: number | null) => void;
  setProgress: (progress: number) => void;
  setHasLyrics: (has: boolean) => void;
  setOffset: (offset: number) => void;
  addOffset: (delta: number) => void;
  setOnlineResults: (results: LyricSearchResult[]) => void;
  setIsSearching: (searching: boolean) => void;
  setFontSize: (size: number) => void;
  setLineHeight: (height: number) => void;
  setActiveColor: (color: string) => void;
  setInactiveColor: (color: string) => void;
  clear: () => void;
}

export const useLyricsStore = create<LyricsState>((set) => ({
  lines: [],
  currentIndex: null,
  progress: 0,
  hasLyrics: false,
  offset: 0,
  onlineResults: [],
  isSearching: false,

  // Default style
  fontSize: 14,
  lineHeight: 1.8,
  activeColor: 'var(--accent)',
  inactiveColor: '#666',

  setLines: (lines) => set({ lines, hasLyrics: lines.length > 0 }),
  setCurrentIndex: (currentIndex) => set({ currentIndex }),
  setProgress: (progress) => set({ progress }),
  setHasLyrics: (hasLyrics) => set({ hasLyrics }),
  setOffset: (offset) => set({ offset }),
  addOffset: (delta) => set((s) => ({ offset: s.offset + delta })),
  setOnlineResults: (onlineResults) => set({ onlineResults }),
  setIsSearching: (isSearching) => set({ isSearching }),
  setFontSize: (fontSize) => set({ fontSize }),
  setLineHeight: (lineHeight) => set({ lineHeight }),
  setActiveColor: (activeColor) => set({ activeColor }),
  setInactiveColor: (inactiveColor) => set({ inactiveColor }),
  clear: () => set({ lines: [], currentIndex: null, progress: 0, hasLyrics: false, onlineResults: [] }),
}));
