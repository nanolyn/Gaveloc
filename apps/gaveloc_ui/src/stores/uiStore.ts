import { create } from 'zustand';

export type View = 'home' | 'accounts' | 'setup';

interface UIState {
  currentView: View;
  setView: (view: View) => void;
}

export const useUIStore = create<UIState>((set) => ({
  currentView: 'home',
  setView: (view) => set({ currentView: view }),
}));
