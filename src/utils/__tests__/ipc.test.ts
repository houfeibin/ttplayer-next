import { describe, it, expect, vi, beforeEach } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { playFile, togglePlayPause, stop, seek, setVolume } from '@/utils/ipc';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('IPC Functions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('playFile should invoke player_play', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await playFile('C:/music/song.mp3');
    expect(invoke).toHaveBeenCalledWith('player_play', { path: 'C:/music/song.mp3' });
  });

  it('togglePlayPause should invoke player_toggle', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await togglePlayPause();
    expect(invoke).toHaveBeenCalledWith('player_toggle');
  });

  it('stop should invoke player_stop', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await stop();
    expect(invoke).toHaveBeenCalledWith('player_stop');
  });

  it('seek should invoke player_seek', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await seek(5000);
    expect(invoke).toHaveBeenCalledWith('player_seek', { positionMs: 5000 });
  });

  it('setVolume should invoke player_set_volume', async () => {
    vi.mocked(invoke).mockResolvedValue(undefined);
    await setVolume(0.5);
    expect(invoke).toHaveBeenCalledWith('player_set_volume', { volume: 0.5 });
  });

  it('playFile should propagate errors', async () => {
    vi.mocked(invoke).mockRejectedValue(new Error('File not found'));
    await expect(playFile('nonexistent.mp3')).rejects.toThrow('File not found');
  });
});
