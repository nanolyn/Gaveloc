import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';

interface LaunchStatus {
  is_running: boolean;
  pid: number | null;
}

interface PreflightResult {
  can_launch: boolean;
  issues: string[];
  warnings: string[];
}

interface LaunchState {
  isLaunching: boolean;
  isRunning: boolean;
  pid: number | null;
  error: string | null;
  preflight: PreflightResult | null;

  // Actions
  launchGame: (accountId: string) => Promise<void>;
  checkStatus: () => Promise<LaunchStatus>;
  runPreflight: (accountId: string) => Promise<PreflightResult>;
  clearError: () => void;
  reset: () => void;
}

export const useLaunchStore = create<LaunchState>((set, get) => ({
  isLaunching: false,
  isRunning: false,
  pid: null,
  error: null,
  preflight: null,

  launchGame: async (accountId: string) => {
    set({ isLaunching: true, error: null });

    try {
      await invoke('launch_game', { accountId });

      // Poll for status after launch
      await new Promise((resolve) => setTimeout(resolve, 2000));
      const status = await get().checkStatus();

      set({
        isLaunching: false,
        isRunning: status.is_running,
        pid: status.pid,
      });
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      set({ isLaunching: false, error: msg });
      throw err;
    }
  },

  checkStatus: async () => {
    try {
      const status = await invoke<LaunchStatus>('get_launch_status');
      set({
        isRunning: status.is_running,
        pid: status.pid,
      });
      return status;
    } catch {
      return { is_running: false, pid: null };
    }
  },

  runPreflight: async (accountId: string) => {
    try {
      const result = await invoke<PreflightResult>('preflight_check', {
        accountId,
      });
      set({ preflight: result });
      return result;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      const result = {
        can_launch: false,
        issues: [msg],
        warnings: [],
      };
      set({ preflight: result });
      return result;
    }
  },

  clearError: () => set({ error: null }),

  reset: () =>
    set({
      isLaunching: false,
      isRunning: false,
      pid: null,
      error: null,
      preflight: null,
    }),
}));
