import { create } from 'zustand';
import type { SkinInfo } from '@/utils/ipc';

interface SkinStore {
  currentSkinId: string;
  availableSkins: SkinInfo[];
  cssVariables: string;
  loading: boolean;

  setCurrentSkinId: (id: string) => void;
  setAvailableSkins: (skins: SkinInfo[]) => void;
  setCssVariables: (css: string) => void;
  setLoading: (loading: boolean) => void;
}

export const useSkinStore = create<SkinStore>((set) => ({
  currentSkinId: 'default',
  availableSkins: [],
  cssVariables: '',
  loading: false,

  setCurrentSkinId: (id) => set({ currentSkinId: id }),
  setAvailableSkins: (skins) => set({ availableSkins: skins }),
  setCssVariables: (css) => set({ cssVariables: css }),
  setLoading: (loading) => set({ loading }),
}));
