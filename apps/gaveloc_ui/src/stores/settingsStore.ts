import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Settings, GameSettings, WineSettings } from '../types';

interface ValidationResult {
  valid: boolean;
  message: string;
}

interface SettingsState {
  // State
  settings: Settings | null;
  loading: boolean;
  saving: boolean;
  error: string | null;
  validationResult: ValidationResult | null;

  // Actions
  loadSettings: () => Promise<void>;
  saveSettings: (settings: Settings) => Promise<void>;
  updateGameSettings: (gameSettings: Partial<GameSettings>) => void;
  updateWineSettings: (wineSettings: Partial<WineSettings>) => void;
  validateGamePath: (path: string) => Promise<ValidationResult>;
  detectGameInstall: () => Promise<string | null>;
  getDefaultInstallPath: () => Promise<string>;
  setError: (error: string | null) => void;
}

const defaultSettings: Settings = {
  game: {
    path: null,
    region: 'Europe',
    language: 'English',
    gamemode: true,
    mangohud: false,
    gamescope: false,
    gamescope_settings: {
      width: null,
      height: null,
      refresh_rate: null,
      fullscreen: false,
      borderless: false,
      extra_args: null,
    },
  },
  wine: {
    runner_path: null,
    prefix_path: null,
    esync: true,
    fsync: true,
    winesync: false,
    dxvk_hud: null,
  },
  log_level: 'info',
};

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  loading: false,
  saving: false,
  error: null,
  validationResult: null,

  loadSettings: async () => {
    set({ loading: true, error: null });
    try {
      const settings = await invoke<Settings>('get_settings');
      set({ settings, loading: false });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error, loading: false, settings: defaultSettings });
    }
  },

  saveSettings: async (settings: Settings) => {
    set({ saving: true, error: null });
    try {
      await invoke('save_settings', { settings });
      set({ settings, saving: false });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error, saving: false });
      throw err;
    }
  },

  updateGameSettings: (gameSettings: Partial<GameSettings>) => {
    const { settings } = get();
    if (!settings) return;

    set({
      settings: {
        ...settings,
        game: { ...settings.game, ...gameSettings },
      },
    });
  },

  updateWineSettings: (wineSettings: Partial<WineSettings>) => {
    const { settings } = get();
    if (!settings) return;

    set({
      settings: {
        ...settings,
        wine: { ...settings.wine, ...wineSettings },
      },
    });
  },

  validateGamePath: async (path: string) => {
    try {
      const result = await invoke<ValidationResult>('validate_game_path', { path });
      set({ validationResult: result });
      return result;
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      const result: ValidationResult = { valid: false, message: error };
      set({ validationResult: result });
      return result;
    }
  },

  detectGameInstall: async () => {
    try {
      return await invoke<string | null>('detect_game_install');
    } catch (err) {
      console.error("Failed to detect game:", err);
      return null;
    }
  },

  getDefaultInstallPath: async () => {
    try {
      return await invoke<string>('get_default_install_path');
    } catch (err) {
      console.error("Failed to get default path:", err);
      return "Games/ffxiv";
    }
  },

  setError: (error: string | null) => {
    set({ error });
  },
}));
