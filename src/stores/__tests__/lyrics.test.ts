import { describe, it, expect, beforeEach } from 'vitest';
import { useLyricsStore } from '@/stores/lyrics';

describe('LyricsStore', () => {
  beforeEach(() => {
    useLyricsStore.setState({
      lines: [],
      currentIndex: null,
      progress: 0,
      hasLyrics: false,
      offset: 0,
      onlineResults: [],
      isSearching: false,
    });
  });

  it('should have correct initial state', () => {
    const s = useLyricsStore.getState();
    expect(s.lines).toEqual([]);
    expect(s.currentIndex).toBeNull();
    expect(s.progress).toBe(0);
    expect(s.hasLyrics).toBe(false);
    expect(s.offset).toBe(0);
    expect(s.onlineResults).toEqual([]);
    expect(s.isSearching).toBe(false);
  });

  it('should update lines', () => {
    const lines = [
      { timeMs: 0, text: 'Line 1', progress: 0 },
      { timeMs: 5000, text: 'Line 2', progress: 0 },
    ];
    useLyricsStore.setState({ lines });
    expect(useLyricsStore.getState().lines).toEqual(lines);
  });

  it('should update current index', () => {
    useLyricsStore.setState({ currentIndex: 2 });
    expect(useLyricsStore.getState().currentIndex).toBe(2);
  });

  it('should update progress', () => {
    useLyricsStore.setState({ progress: 0.5 });
    expect(useLyricsStore.getState().progress).toBe(0.5);
  });

  it('should update offset', () => {
    useLyricsStore.setState({ offset: 500 });
    expect(useLyricsStore.getState().offset).toBe(500);
  });
});
