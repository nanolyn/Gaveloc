import { useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useSettingsStore } from '../../stores/settingsStore';
import type { Region, Language } from '../../types';
import './SettingsPage.css';

const REGIONS: { value: Region; label: string }[] = [
  { value: 'Japan', label: 'Japan' },
  { value: 'NorthAmerica', label: 'North America' },
  { value: 'Europe', label: 'Europe' },
];

const LANGUAGES: { value: Language; label: string }[] = [
  { value: 'Japanese', label: 'Japanese' },
  { value: 'English', label: 'English' },
  { value: 'German', label: 'German' },
  { value: 'French', label: 'French' },
];

export function GameSettings() {
  const { settings, updateGameSettings, validateGamePath, validationResult } =
    useSettingsStore();
  const [validating, setValidating] = useState(false);

  if (!settings) return null;

  const { game } = settings;

  const handlePathChange = async (newPath: string) => {
    updateGameSettings({ path: newPath || null });
    if (newPath) {
      setValidating(true);
      await validateGamePath(newPath);
      setValidating(false);
    }
  };

  const handleBrowse = async () => {
    const selected = await open({
      directory: true,
      title: 'Select FFXIV Installation Folder',
    });
    if (selected) {
      handlePathChange(selected);
    }
  };

  return (
    <div className="settings-section">
      <h2>Game Settings</h2>
      <div className="settings-group">
        <div className="settings-row">
          <label>Game Installation Path</label>
          <div className="settings-path-input">
            <input
              type="text"
              value={game.path || ''}
              onChange={(e) => handlePathChange(e.target.value)}
              placeholder="/path/to/FFXIV"
            />
            <button className="secondary" onClick={handleBrowse}>
              Browse
            </button>
          </div>
          {validating && (
            <div className="settings-validation">Validating...</div>
          )}
          {!validating && validationResult && (
            <div
              className={`settings-validation ${
                validationResult.valid ? 'valid' : 'invalid'
              }`}
            >
              {validationResult.message}
            </div>
          )}
        </div>

        <div className="settings-row">
          <label>Region</label>
          <select
            value={game.region}
            onChange={(e) =>
              updateGameSettings({ region: e.target.value as Region })
            }
          >
            {REGIONS.map((r) => (
              <option key={r.value} value={r.value}>
                {r.label}
              </option>
            ))}
          </select>
        </div>

        <div className="settings-row">
          <label>Language</label>
          <select
            value={game.language}
            onChange={(e) =>
              updateGameSettings({ language: e.target.value as Language })
            }
          >
            {LANGUAGES.map((l) => (
              <option key={l.value} value={l.value}>
                {l.label}
              </option>
            ))}
          </select>
        </div>

        <div className="checkbox-row">
          <input
            type="checkbox"
            id="gamemode"
            checked={game.gamemode}
            onChange={(e) => updateGameSettings({ gamemode: e.target.checked })}
          />
          <label htmlFor="gamemode">Enable GameMode</label>
          <span className="hint">Optimizes CPU for gaming</span>
        </div>

        <div className="checkbox-row">
          <input
            type="checkbox"
            id="mangohud"
            checked={game.mangohud}
            onChange={(e) => updateGameSettings({ mangohud: e.target.checked })}
          />
          <label htmlFor="mangohud">Enable MangoHud</label>
          <span className="hint">Shows FPS overlay</span>
        </div>

        <div className="checkbox-row">
          <input
            type="checkbox"
            id="gamescope"
            checked={game.gamescope}
            onChange={(e) =>
              updateGameSettings({ gamescope: e.target.checked })
            }
          />
          <label htmlFor="gamescope">Enable Gamescope</label>
          <span className="hint">SteamOS compositor</span>
        </div>

        {game.gamescope && (
          <div className="settings-group" style={{ marginLeft: '24px' }}>
            <div className="settings-row">
              <label>Resolution (optional)</label>
              <div style={{ display: 'flex', gap: '8px' }}>
                <input
                  type="number"
                  placeholder="Width"
                  value={game.gamescope_settings.width || ''}
                  onChange={(e) =>
                    updateGameSettings({
                      gamescope_settings: {
                        ...game.gamescope_settings,
                        width: e.target.value ? parseInt(e.target.value) : null,
                      },
                    })
                  }
                  style={{ width: '100px' }}
                />
                <span style={{ color: 'var(--color-text-secondary)' }}>x</span>
                <input
                  type="number"
                  placeholder="Height"
                  value={game.gamescope_settings.height || ''}
                  onChange={(e) =>
                    updateGameSettings({
                      gamescope_settings: {
                        ...game.gamescope_settings,
                        height: e.target.value
                          ? parseInt(e.target.value)
                          : null,
                      },
                    })
                  }
                  style={{ width: '100px' }}
                />
              </div>
            </div>

            <div className="checkbox-row">
              <input
                type="checkbox"
                id="gamescope-fullscreen"
                checked={game.gamescope_settings.fullscreen}
                onChange={(e) =>
                  updateGameSettings({
                    gamescope_settings: {
                      ...game.gamescope_settings,
                      fullscreen: e.target.checked,
                    },
                  })
                }
              />
              <label htmlFor="gamescope-fullscreen">Fullscreen</label>
            </div>

            <div className="checkbox-row">
              <input
                type="checkbox"
                id="gamescope-borderless"
                checked={game.gamescope_settings.borderless}
                onChange={(e) =>
                  updateGameSettings({
                    gamescope_settings: {
                      ...game.gamescope_settings,
                      borderless: e.target.checked,
                    },
                  })
                }
              />
              <label htmlFor="gamescope-borderless">Borderless</label>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
