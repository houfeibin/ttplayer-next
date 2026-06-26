import { describe, it, expect, beforeEach } from 'vitest';
import { useSkinStore } from '@/stores/skin';

describe('SkinStore', () => {
  beforeEach(() => {
    useSkinStore.setState({
      currentSkinId: 'default',
      availableSkins: [],
      cssVariables: '',
      loading: false,
    });
  });

  it('should have correct initial state', () => {
    const s = useSkinStore.getState();
    expect(s.currentSkinId).toBe('default');
    expect(s.availableSkins).toEqual([]);
    expect(s.cssVariables).toBe('');
    expect(s.loading).toBe(false);
  });

  it('should update current skin id', () => {
    useSkinStore.setState({ currentSkinId: 'ttplayer-blue' });
    expect(useSkinStore.getState().currentSkinId).toBe('ttplayer-blue');
  });

  it('should update css variables', () => {
    useSkinStore.setState({ cssVariables: ':root { --bg-primary: #000; }' });
    expect(useSkinStore.getState().cssVariables).toContain('--bg-primary');
  });

  it('should update available skins', () => {
    const skins = [
      { id: 'default', name: 'Default', builtin: true },
      { id: 'ttplayer-blue', name: '千千蓝', builtin: true },
    ];
    useSkinStore.setState({ availableSkins: skins as any });
    expect(useSkinStore.getState().availableSkins).toHaveLength(2);
  });
});
