import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  PatchPhase,
  PatchProgressEvent,
  PatchCompletedEvent,
  PatchStatus,
} from '../types';

interface PatchState {
  isPatching: boolean;
  phase: PatchPhase;
  currentIndex: number;
  totalPatches: number;
  currentVersionId: string | null;
  currentRepository: string | null;
  bytesProcessed: number;
  bytesTotal: number;
  speedBytesPerSec: number;
  error: string | null;
  completedPatches: string[];

  // Actions
  startBootPatch: () => Promise<void>;
  startGamePatch: (accountId: string) => Promise<void>;
  cancelPatch: () => Promise<void>;
  getStatus: () => Promise<PatchStatus>;
  updateProgress: (event: PatchProgressEvent) => void;
  markCompleted: (event: PatchCompletedEvent) => void;
  setError: (message: string) => void;
  reset: () => void;
}

export const usePatchStore = create<PatchState>((set) => ({
  isPatching: false,
  phase: 'Idle',
  currentIndex: 0,
  totalPatches: 0,
  currentVersionId: null,
  currentRepository: null,
  bytesProcessed: 0,
  bytesTotal: 0,
  speedBytesPerSec: 0,
  error: null,
  completedPatches: [],

  startBootPatch: async (): Promise<void> => {
    set({
      isPatching: true,
      phase: 'Downloading',
      error: null,
      completedPatches: [],
    });

    try {
      await invoke('start_boot_patch');
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({
        isPatching: false,
        phase: 'Failed',
        error: errorMessage,
      });
    }
  },

  startGamePatch: async (accountId: string): Promise<void> => {
    set({
      isPatching: true,
      phase: 'Downloading',
      error: null,
      completedPatches: [],
    });

    try {
      await invoke('start_game_patch', { accountId });
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({
        isPatching: false,
        phase: 'Failed',
        error: errorMessage,
      });
    }
  },

  cancelPatch: async (): Promise<void> => {
    try {
      await invoke('cancel_patch');
    } catch (err) {
      console.error('Failed to cancel patch:', err);
    }
  },

  getStatus: async (): Promise<PatchStatus> => {
    try {
      const status = await invoke<PatchStatus>('get_patch_status');
      set({
        isPatching: status.is_patching,
        phase: status.phase,
        currentIndex: status.current_patch_index,
        totalPatches: status.total_patches,
        currentVersionId: status.current_version_id,
        currentRepository: status.current_repository,
        bytesProcessed: status.bytes_downloaded,
        bytesTotal: status.bytes_total,
        speedBytesPerSec: status.speed_bytes_per_sec,
      });
      return status;
    } catch (err) {
      return {
        is_patching: false,
        phase: 'Idle',
        current_patch_index: 0,
        total_patches: 0,
        current_version_id: null,
        current_repository: null,
        bytes_downloaded: 0,
        bytes_total: 0,
        speed_bytes_per_sec: 0,
      };
    }
  },

  updateProgress: (event: PatchProgressEvent): void => {
    set({
      isPatching: true,
      phase: event.phase,
      currentIndex: event.current_index,
      totalPatches: event.total_patches,
      currentVersionId: event.version_id,
      currentRepository: event.repository,
      bytesProcessed: event.bytes_processed,
      bytesTotal: event.bytes_total,
      speedBytesPerSec: event.speed_bytes_per_sec,
    });
  },

  markCompleted: (event: PatchCompletedEvent): void => {
    set((state) => ({
      completedPatches: [...state.completedPatches, event.version_id],
    }));
  },

  setError: (message: string): void => {
    set({
      isPatching: false,
      phase: 'Failed',
      error: message,
    });
  },

  reset: (): void => {
    set({
      isPatching: false,
      phase: 'Idle',
      currentIndex: 0,
      totalPatches: 0,
      currentVersionId: null,
      currentRepository: null,
      bytesProcessed: 0,
      bytesTotal: 0,
      speedBytesPerSec: 0,
      error: null,
      completedPatches: [],
    });
  },
}));
