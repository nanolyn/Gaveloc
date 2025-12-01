import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { WineRunner } from '../types';

interface RunnerState {
  // State
  runners: WineRunner[];
  selectedRunner: WineRunner | null;
  isLoading: boolean;
  isValidating: boolean;
  error: string | null;

  // Actions
  loadRunners: () => Promise<void>;
  loadSelectedRunner: () => Promise<void>;
  selectRunner: (path: string | null) => Promise<WineRunner>;
  validateRunner: (path: string) => Promise<WineRunner>;
  clearError: () => void;
}

export const useRunnerStore = create<RunnerState>((set) => ({
  runners: [],
  selectedRunner: null,
  isLoading: false,
  isValidating: false,
  error: null,

  loadRunners: async () => {
    set({ isLoading: true, error: null });
    try {
      const runners = await invoke<WineRunner[]>('list_runners');
      set({ runners, isLoading: false });
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg, isLoading: false });
    }
  },

  loadSelectedRunner: async () => {
    try {
      const runner = await invoke<WineRunner | null>('get_selected_runner');
      set({ selectedRunner: runner });
    } catch {
      // Silently fail - no runner selected yet
    }
  },

  selectRunner: async (path: string | null) => {
    set({ isValidating: true, error: null });
    try {
      const runner = await invoke<WineRunner>('select_runner', { path });
      set({ selectedRunner: runner, isValidating: false });
      return runner;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg, isValidating: false });
      throw e;
    }
  },

  validateRunner: async (path: string) => {
    set({ isValidating: true, error: null });
    try {
      const runner = await invoke<WineRunner>('validate_runner', { path });
      set({ isValidating: false });
      return runner;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      set({ error: msg, isValidating: false });
      throw e;
    }
  },

  clearError: () => set({ error: null }),
}));
