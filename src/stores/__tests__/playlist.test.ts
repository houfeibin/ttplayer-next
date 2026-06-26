import { describe, it, expect, beforeEach } from 'vitest';
import { usePlaylistStore, pathToName } from '@/stores/playlist';

describe('PlaylistStore', () => {
  beforeEach(() => {
    usePlaylistStore.setState({ items: [], currentIndex: -1 });
  });

  it('should have correct initial state', () => {
    const s = usePlaylistStore.getState();
    expect(s.items).toEqual([]);
    expect(s.currentIndex).toBe(-1);
  });

  it('should update items', () => {
    usePlaylistStore.setState({
      items: [
        { path: 'C:/music/song1.mp3', name: 'song1', format: 'Mp3' },
        { path: 'C:/music/song2.flac', name: 'song2', format: 'Flac' },
      ],
    });
    expect(usePlaylistStore.getState().items).toHaveLength(2);
  });

  it('should update current index', () => {
    usePlaylistStore.setState({ currentIndex: 1 });
    expect(usePlaylistStore.getState().currentIndex).toBe(1);
  });
});

describe('pathToName', () => {
  it('should extract filename from Windows path', () => {
    expect(pathToName('C:\\music\\song.mp3')).toBe('song');
  });

  it('should extract filename from Unix path', () => {
    expect(pathToName('/home/user/music/song.flac')).toBe('song');
  });

  it('should handle filename only', () => {
    expect(pathToName('song.mp3')).toBe('song');
  });
});
