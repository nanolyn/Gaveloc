import { useEffect } from 'react';
import { useSettingsStore } from '../../stores/settingsStore';
import { GameSettings } from './GameSettings';
import { WineSettings } from './WineSettings';
import './SettingsPage.css';

interface SettingsPageProps {
  onClose: () => void;
}

export function SettingsPage({ onClose }: SettingsPageProps) {
  const { settings, loading, saving, error, loadSettings, saveSettings } =
    useSettingsStore();

  useEffect(() => {
    loadSettings();
  }, [loadSettings]);

  const handleSave = async () => {
    if (!settings) return;
    try {
      await saveSettings(settings);
      onClose();
    } catch {
      // Error is already set in the store
    }
  };

  if (loading) {
    return (
      <div className="settings-page">
        <div className="settings-loading">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="settings-page">
      {error && <div className="settings-error">{error}</div>}

      <div className="settings-content">
        <GameSettings />
        <WineSettings />
      </div>

      <div className="settings-footer">
        <button className="secondary" onClick={onClose}>
          Cancel
        </button>
        <button className="primary" onClick={handleSave} disabled={saving}>
          {saving ? 'Saving...' : 'Save'}
        </button>
      </div>
    </div>
  );
}
