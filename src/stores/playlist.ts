import { create } from 'zustand';

export interface PlaylistItem {
  path: string;
  format: string;
  name: string;
}

export type PlayMode = 'single' | 'sequential' | 'loop' | 'loop_one' | 'random';

interface PlaylistStore {
  items: PlaylistItem[];
  currentIndex: number;
  playMode: PlayMode;

  setItems: (items: PlaylistItem[], currentIndex: number) => void;
  setCurrentIndex: (idx: number) => void;
  setPlayMode: (mode: PlayMode) => void;
  removeItem: (index: number) => void;
  moveItem: (from: number, to: number) => void;
}

/** Extract display name from path */
export function pathToName(path: string): string {
  const segs = path.split(/[/\\]/);
  const last = segs[segs.length - 1] ?? path;
  // strip extension
  const dot = last.lastIndexOf('.');
  return dot > 0 ? last.slice(0, dot) : last;
}

export const usePlaylistStore = create<PlaylistStore>((set) => ({
  items: [],
  currentIndex: -1,
  playMode: 'sequential',

  setItems: (items, currentIndex) => set({ items, currentIndex }),
  setCurrentIndex: (currentIndex) => set({ currentIndex }),
  setPlayMode: (playMode) => set({ playMode }),
  removeItem: (index) => set((s) => {
    const next = [...s.items];
    next.splice(index, 1);
    let ci = s.currentIndex;
    if (index < ci) ci--;
    else if (index === ci) ci = Math.min(ci, next.length - 1);
    return { items: next, currentIndex: next.length > 0 ? ci : -1 };
  }),
  moveItem: (from, to) => set((s) => {
    if (from < 0 || from >= s.items.length || to < 0 || to >= s.items.length || from === to) {
      return s;
    }
    const next = [...s.items];
    const [item] = next.splice(from, 1);
    next.splice(to, 0, item);
    // Track current index movement (mirror backend logic)
    let ci = s.currentIndex;
    if (ci === from) ci = to;
    else if (from < ci && to >= ci) ci--;
    else if (from > ci && to <= ci) ci++;
    return { items: next, currentIndex: ci };
  }),
}));
