import { describe, it, expect, beforeEach } from 'vitest';
import { usePlayerStore } from '@/stores/player';

describe('PlayerStore', () => {
  beforeEach(() => {
    usePlayerStore.setState({
      state: 'Idle',
      currentFile: null,
      positionMs: 0,
      durationMs: 0,
      volume: 80,
      metadata: {
        title: '', artist: '', album: '', albumArtist: '',
        year: null, track: null, genre: '', comment: '',
        durationMs: null, bitRate: null, sampleRate: null,
        channels: null, bitDepth: null, coverArt: null,
      },
    });
  });

  it('should have correct initial state', () => {
    const s = usePlayerStore.getState();
    expect(s.state).toBe('Idle');
    expect(s.currentFile).toBeNull();
    expect(s.positionMs).toBe(0);
    expect(s.durationMs).toBe(0);
    expect(s.volume).toBe(80);
  });

  it('should update state', () => {
    usePlayerStore.setState({ state: 'Playing' });
    expect(usePlayerStore.getState().state).toBe('Playing');
  });

  it('should update position', () => {
    usePlayerStore.setState({ positionMs: 5000 });
    expect(usePlayerStore.getState().positionMs).toBe(5000);
  });

  it('should update volume', () => {
    usePlayerStore.setState({ volume: 50 });
    expect(usePlayerStore.getState().volume).toBe(50);
  });

  it('should update metadata', () => {
    const meta = {
      title: 'Test', artist: 'Artist', album: 'Album', albumArtist: '',
      year: null, track: null, genre: '', comment: '',
      durationMs: null, bitRate: null, sampleRate: null,
      channels: null, bitDepth: null, coverArt: null,
    };
    usePlayerStore.setState({ metadata: meta });
    expect(usePlayerStore.getState().metadata.title).toBe('Test');
  });
});
