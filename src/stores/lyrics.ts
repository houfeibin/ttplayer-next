import { create } from 'zustand';
import { persist } from 'zustand/middleware';
import type { LrcLine, LyricSearchResult } from '@/utils/ipc';

export type LyricsTextAlign = 'left' | 'center' | 'right';

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
  fontFamily: string;
  textAlign: LyricsTextAlign;

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
  setFontFamily: (family: string) => void;
  setTextAlign: (align: LyricsTextAlign) => void;
  clear: () => void;
}

export const useLyricsStore = create<LyricsState>()(
  persist(
    (set) => ({
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
      fontFamily: 'system-ui, sans-serif',
      textAlign: 'center',

      setLines: (lines) => set({ lines, hasLyrics: lines.length > 0 }),
      setCurrentIndex: (currentIndex) => set({ currentIndex }),
      setProgress: (progress) => set({ progress }),
      setHasLyrics: (has) => set({ hasLyrics: has }),
      setOffset: (offset) => set({ offset }),
      addOffset: (delta) => set((s) => ({ offset: s.offset + delta })),
      setOnlineResults: (onlineResults) => set({ onlineResults }),
      setIsSearching: (isSearching) => set({ isSearching }),
      setFontSize: (fontSize) => set({ fontSize }),
      setLineHeight: (lineHeight) => set({ lineHeight }),
      setActiveColor: (activeColor) => set({ activeColor }),
      setInactiveColor: (inactiveColor) => set({ inactiveColor }),
      setFontFamily: (fontFamily) => set({ fontFamily }),
      setTextAlign: (textAlign) => set({ textAlign }),
      clear: () => set({ lines: [], currentIndex: null, progress: 0, hasLyrics: false, onlineResults: [] }),
    }),
    {
      name: 'ttplayer:lyrics-style',
      // 仅持久化样式配置，运行时状态（歌词行、进度等）不持久化
      partialize: (s) => ({
        fontSize: s.fontSize,
        lineHeight: s.lineHeight,
        activeColor: s.activeColor,
        inactiveColor: s.inactiveColor,
        fontFamily: s.fontFamily,
        textAlign: s.textAlign,
      }),
    },
  ),
);
