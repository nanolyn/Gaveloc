import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useSettingsStore } from '../../stores/settingsStore';
import { Icon } from '../Icon';
import type { Language } from '../../types';
import './Settings.css';

export function Settings() {
  const {
    settings,
    loading,
    saving,
    error,
    loadSettings,
    saveSettings,
    updateGameSettings,
    updateWineSettings,
  } = useSettingsStore();

  const [hasChanges, setHasChanges] = useState(false);

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const handleSave = async () => {
    if (settings) {
      await saveSettings(settings);
      setHasChanges(false);
    }
  };

  const handleGamePathBrowse = async () => {
    const selected = await open({
      directory: true,
      title: 'Select Game Directory',
    });
    if (selected && typeof selected === 'string') {
      updateGameSettings({ path: selected });
      setHasChanges(true);
    }
  };

  const handleRunnerPathBrowse = async () => {
    const selected = await open({
      directory: false,
      title: 'Select Wine Runner',
      filters: [{ name: 'Executable', extensions: [''] }],
    });
    if (selected && typeof selected === 'string') {
      updateWineSettings({ runner_path: selected });
      setHasChanges(true);
    }
  };

  const handlePrefixPathBrowse = async () => {
    const selected = await open({
      directory: true,
      title: 'Select Wine Prefix Directory',
    });
    if (selected && typeof selected === 'string') {
      updateWineSettings({ prefix_path: selected });
      setHasChanges(true);
    }
  };

  if (loading || !settings) {
    return (
      <div className="settings-view">
        <div className="settings-loading">
          <Icon name="spinner" size={24} className="spinning" />
          <span>Loading settings...</span>
        </div>
      </div>
    );
  }

  return (
    <div className="settings-view">
      <div className="settings-header">
        <h2 className="settings-title">Settings</h2>
        {error && <span className="settings-error">{error}</span>}
      </div>

      <div className="settings-content">
        {/* Game Settings Section */}
        <section className="settings-section">
          <h3 className="section-title">Game</h3>

          <div className="setting-field">
            <label>Game Path</label>
            <div className="path-input">
              <input
                type="text"
                value={settings.game.path || ''}
                onChange={(e) => {
                  updateGameSettings({ path: e.target.value || null });
                  setHasChanges(true);
                }}
                placeholder="Select game directory..."
              />
              <button
                type="button"
                className="secondary"
                onClick={handleGamePathBrowse}
              >
                <Icon name="folder-open" size={14} />
              </button>
            </div>
          </div>

          <div className="setting-field">
            <label>Language</label>
            <select
              value={settings.game.language}
              onChange={(e) => {
                updateGameSettings({ language: e.target.value as Language });
                setHasChanges(true);
              }}
            >
              <option value="English">English</option>
              <option value="Japanese">Japanese</option>
              <option value="German">German</option>
              <option value="French">French</option>
            </select>
          </div>

          <div className="setting-toggles">
            <label className="toggle-item">
              <input
                type="checkbox"
                checked={settings.game.gamemode}
                onChange={(e) => {
                  updateGameSettings({ gamemode: e.target.checked });
                  setHasChanges(true);
                }}
              />
              <span>GameMode</span>
            </label>
            <label className="toggle-item">
              <input
                type="checkbox"
                checked={settings.game.mangohud}
                onChange={(e) => {
                  updateGameSettings({ mangohud: e.target.checked });
                  setHasChanges(true);
                }}
              />
              <span>MangoHud</span>
            </label>
            <label className="toggle-item">
              <input
                type="checkbox"
                checked={settings.game.gamescope}
                onChange={(e) => {
                  updateGameSettings({ gamescope: e.target.checked });
                  setHasChanges(true);
                }}
              />
              <span>Gamescope</span>
            </label>
          </div>
        </section>

        {/* Wine Settings Section */}
        <section className="settings-section">
          <h3 className="section-title">Wine</h3>

          <div className="setting-field">
            <label>Runner Path</label>
            <div className="path-input">
              <input
                type="text"
                value={settings.wine.runner_path || ''}
                onChange={(e) => {
                  updateWineSettings({ runner_path: e.target.value || null });
                  setHasChanges(true);
                }}
                placeholder="Auto-detect or select runner..."
              />
              <button
                type="button"
                className="secondary"
                onClick={handleRunnerPathBrowse}
              >
                <Icon name="folder-open" size={14} />
              </button>
            </div>
          </div>

          <div className="setting-field">
            <label>Prefix Path</label>
            <div className="path-input">
              <input
                type="text"
                value={settings.wine.prefix_path || ''}
                onChange={(e) => {
                  updateWineSettings({ prefix_path: e.target.value || null });
                  setHasChanges(true);
                }}
                placeholder="Default or select prefix..."
              />
              <button
                type="button"
                className="secondary"
                onClick={handlePrefixPathBrowse}
              >
                <Icon name="folder-open" size={14} />
              </button>
            </div>
          </div>

          <div className="setting-toggles">
            <label className="toggle-item">
              <input
                type="checkbox"
                checked={settings.wine.esync}
                onChange={(e) => {
                  updateWineSettings({ esync: e.target.checked });
                  setHasChanges(true);
                }}
              />
              <span>Esync</span>
            </label>
            <label className="toggle-item">
              <input
                type="checkbox"
                checked={settings.wine.fsync}
                onChange={(e) => {
                  updateWineSettings({ fsync: e.target.checked });
                  setHasChanges(true);
                }}
              />
              <span>Fsync</span>
            </label>
            <label className="toggle-item">
              <input
                type="checkbox"
                checked={settings.wine.winesync}
                onChange={(e) => {
                  updateWineSettings({ winesync: e.target.checked });
                  setHasChanges(true);
                }}
              />
              <span>Winesync</span>
            </label>
          </div>

          <div className="setting-field">
            <label>DXVK HUD</label>
            <input
              type="text"
              value={settings.wine.dxvk_hud || ''}
              onChange={(e) => {
                updateWineSettings({ dxvk_hud: e.target.value || null });
                setHasChanges(true);
              }}
              placeholder="e.g., fps,frametimes,gpuload"
            />
          </div>
        </section>
      </div>

      <div className="settings-footer">
        <button
          className="primary"
          onClick={handleSave}
          disabled={saving || !hasChanges}
        >
          {saving ? (
            <>
              <Icon name="spinner" size={14} className="spinning" />
              Saving...
            </>
          ) : (
            'Save Settings'
          )}
        </button>
      </div>
    </div>
  );
}
