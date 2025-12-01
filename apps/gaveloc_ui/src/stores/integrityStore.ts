import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type {
  IntegrityResult,
  IntegrityProgress,
  FileIntegrityResult,
  FileToRepair,
  RepairResult,
} from '../types';

interface IntegrityState {
  // State
  isChecking: boolean;
  isRepairing: boolean;
  progress: IntegrityProgress | null;
  result: IntegrityResult | null;
  error: string | null;

  // Actions
  startVerify: () => Promise<IntegrityResult>;
  cancelVerify: () => Promise<void>;
  repairFiles: (files: FileIntegrityResult[]) => Promise<RepairResult>;
  updateProgress: (progress: IntegrityProgress) => void;
  setResult: (result: IntegrityResult) => void;
  setError: (error: string) => void;
  reset: () => void;
  clearError: () => void;
}

export const useIntegrityStore = create<IntegrityState>((set) => ({
  // Initial state
  isChecking: false,
  isRepairing: false,
  progress: null,
  result: null,
  error: null,

  // Start integrity verification
  startVerify: async () => {
    set({ isChecking: true, error: null, result: null, progress: null });
    try {
      const result = await invoke<IntegrityResult>('verify_integrity');
      set({ result, isChecking: false });
      return result;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      set({ error: errorMsg, isChecking: false });
      throw e;
    }
  },

  // Cancel ongoing verification
  cancelVerify: async () => {
    try {
      await invoke('cancel_integrity_check');
    } catch (e) {
      console.error('Failed to cancel integrity check:', e);
    }
  },

  // Repair files (deletes them so patching can restore)
  repairFiles: async (files: FileIntegrityResult[]) => {
    set({ isRepairing: true, error: null });

    // Convert to FileToRepair format
    const filesToRepair: FileToRepair[] = files
      .filter((f) => f.status !== 'Valid' && f.status !== 'Unreadable')
      .map((f) => ({
        relative_path: f.relative_path,
        expected_hash: f.expected_hash,
      }));

    try {
      const result = await invoke<RepairResult>('repair_files', {
        files: filesToRepair,
      });
      set({ isRepairing: false });
      return result;
    } catch (e) {
      const errorMsg = e instanceof Error ? e.message : String(e);
      set({ error: errorMsg, isRepairing: false });
      throw e;
    }
  },

  // Update progress from event
  updateProgress: (progress: IntegrityProgress) => {
    set({ progress, isChecking: true });
  },

  // Set result from event
  setResult: (result: IntegrityResult) => {
    set({ result, isChecking: false, progress: null });
  },

  // Set error
  setError: (error: string) => {
    set({ error, isChecking: false, isRepairing: false });
  },

  // Reset all state
  reset: () => {
    set({
      isChecking: false,
      isRepairing: false,
      progress: null,
      result: null,
      error: null,
    });
  },

  // Clear error only
  clearError: () => {
    set({ error: null });
  },
}));
