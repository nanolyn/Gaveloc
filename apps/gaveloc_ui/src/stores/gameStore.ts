import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { GameVersions, UpdateCheckResult } from '../types';

interface GameState {
  versions: GameVersions | null;
  bootUpdates: UpdateCheckResult | null;
  gameUpdates: UpdateCheckResult | null;
  isLoadingVersions: boolean;
  isCheckingBootUpdates: boolean;
  isCheckingGameUpdates: boolean;
  error: string | null;

  // Actions
  initVersionRepo: () => Promise<void>;
  loadVersions: () => Promise<void>;
  checkBootUpdates: () => Promise<UpdateCheckResult>;
  checkGameUpdates: (accountId: string) => Promise<UpdateCheckResult>;
  clearUpdates: () => void;
  clearError: () => void;
}

export const useGameStore = create<GameState>((set) => ({
  versions: null,
  bootUpdates: null,
  gameUpdates: null,
  isLoadingVersions: false,
  isCheckingBootUpdates: false,
  isCheckingGameUpdates: false,
  error: null,

  initVersionRepo: async (): Promise<void> => {
    try {
      await invoke('init_version_repo');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ error: errorMessage });
    }
  },

  loadVersions: async (): Promise<void> => {
    set({ isLoadingVersions: true, error: null });

    try {
      const versions = await invoke<GameVersions>('get_game_versions');
      set({ versions, isLoadingVersions: false });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ error: errorMessage, isLoadingVersions: false });
    }
  },

  checkBootUpdates: async (): Promise<UpdateCheckResult> => {
    set({ isCheckingBootUpdates: true, error: null });

    try {
      const result = await invoke<UpdateCheckResult>('check_boot_updates');
      set({ bootUpdates: result, isCheckingBootUpdates: false });

      if (result.error) {
        set({ error: result.error });
      }

      return result;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ error: errorMessage, isCheckingBootUpdates: false });
      return {
        has_updates: false,
        patches: [],
        total_size_bytes: 0,
        error: errorMessage,
      };
    }
  },

  checkGameUpdates: async (accountId: string): Promise<UpdateCheckResult> => {
    set({ isCheckingGameUpdates: true, error: null });

    try {
      const result = await invoke<UpdateCheckResult>('check_game_updates', {
        accountId,
      });
      set({ gameUpdates: result, isCheckingGameUpdates: false });

      if (result.error) {
        set({ error: result.error });
      }

      return result;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({ error: errorMessage, isCheckingGameUpdates: false });
      return {
        has_updates: false,
        patches: [],
        total_size_bytes: 0,
        error: errorMessage,
      };
    }
  },

  clearUpdates: (): void => {
    set({ bootUpdates: null, gameUpdates: null });
  },

  clearError: (): void => {
    set({ error: null });
  },
}));
