import { useEffect, useState } from 'react';
import { open } from '@tauri-apps/plugin-dialog';
import { useSettingsStore } from '../../stores/settingsStore';
import { useRunnerStore } from '../../stores/runnerStore';
import type { WineRunner } from '../../types';
import './SettingsPage.css';

// Group runners by type for display
function groupRunnersByType(runners: WineRunner[]): Map<string, WineRunner[]> {
  const groups = new Map<string, WineRunner[]>();
  const typeOrder = ['System', 'Proton', 'Lutris', 'GavelocManaged', 'Custom'];

  for (const type of typeOrder) {
    groups.set(type, []);
  }

  for (const runner of runners) {
    const existing = groups.get(runner.runner_type) || [];
    existing.push(runner);
    groups.set(runner.runner_type, existing);
  }

  // Remove empty groups
  for (const [type, list] of groups) {
    if (list.length === 0) {
      groups.delete(type);
    }
  }

  return groups;
}

// Human-readable type names
function getTypeName(type: string): string {
  const names: Record<string, string> = {
    System: 'System Wine',
    Proton: 'Steam Proton',
    Lutris: 'Lutris Runners',
    GavelocManaged: 'Gaveloc Managed',
    Custom: 'Custom',
  };
  return names[type] || type;
}

export function WineSettings() {
  const { settings, updateWineSettings } = useSettingsStore();
  const {
    runners,
    selectedRunner,
    isLoading,
    isValidating,
    error,
    loadRunners,
    loadSelectedRunner,
    selectRunner,
    validateRunner,
    clearError,
  } = useRunnerStore();

  const [showCustomInput, setShowCustomInput] = useState(false);
  const [customPath, setCustomPath] = useState('');
  const [customError, setCustomError] = useState<string | null>(null);

  // Load runners on mount
  useEffect(() => {
    loadRunners();
    loadSelectedRunner();
  }, [loadRunners, loadSelectedRunner]);

  if (!settings) return null;

  const { wine } = settings;
  const groupedRunners = groupRunnersByType(runners);

  const handleRunnerChange = async (value: string) => {
    setCustomError(null);
    clearError();

    if (value === '__auto__') {
      // Auto-detect: select first available
      try {
        await selectRunner(null);
        setShowCustomInput(false);
      } catch {
        // Error is set in store
      }
    } else if (value === '__custom__') {
      // Show custom input
      setShowCustomInput(true);
      setCustomPath('');
    } else {
      // Select specific runner by path
      try {
        await selectRunner(value);
        setShowCustomInput(false);
      } catch {
        // Error is set in store
      }
    }
  };

  const handleCustomPathSubmit = async () => {
    if (!customPath.trim()) {
      setCustomError('Please enter a path');
      return;
    }

    setCustomError(null);
    clearError();

    try {
      await validateRunner(customPath);
      await selectRunner(customPath);
      setShowCustomInput(false);
      setCustomPath('');
    } catch (e) {
      setCustomError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleBrowseCustomRunner = async () => {
    const selected = await open({
      directory: false,
      title: 'Select Wine/Proton Executable',
    });
    if (selected) {
      setCustomPath(selected);
    }
  };

  const handleBrowsePrefix = async () => {
    const selected = await open({
      directory: true,
      title: 'Select Wine Prefix Folder',
    });
    if (selected) {
      updateWineSettings({ prefix_path: selected });
    }
  };

  // Determine current selection value
  const getCurrentValue = (): string => {
    if (showCustomInput) return '__custom__';
    if (!selectedRunner) return '__auto__';
    // Check if selected runner is in the detected list
    const inList = runners.some((r) => r.path === selectedRunner.path);
    return inList ? selectedRunner.path : '__custom__';
  };

  return (
    <div className="settings-section">
      <h2>Wine Settings</h2>
      <div className="settings-group">
        <div className="settings-row">
          <label>Wine/Proton Runner</label>
          <div className="runner-select-container">
            <select
              className="runner-select"
              value={getCurrentValue()}
              onChange={(e) => handleRunnerChange(e.target.value)}
              disabled={isLoading || isValidating}
            >
              <option value="__auto__">Auto-detect</option>
              {Array.from(groupedRunners.entries()).map(([type, typeRunners]) => (
                <optgroup key={type} label={getTypeName(type)}>
                  {typeRunners.map((runner) => (
                    <option key={runner.path} value={runner.path}>
                      {runner.name}
                      {!runner.is_valid && ' (invalid)'}
                    </option>
                  ))}
                </optgroup>
              ))}
              <option value="__custom__">Custom path...</option>
            </select>
            {isLoading && <span className="runner-loading">Loading...</span>}
            {isValidating && <span className="runner-loading">Validating...</span>}
          </div>

          {showCustomInput && (
            <div className="custom-runner-input">
              <div className="settings-path-input">
                <input
                  type="text"
                  value={customPath}
                  onChange={(e) => setCustomPath(e.target.value)}
                  placeholder="/path/to/wine or /path/to/proton"
                  disabled={isValidating}
                />
                <button
                  className="secondary"
                  onClick={handleBrowseCustomRunner}
                  disabled={isValidating}
                >
                  Browse
                </button>
                <button
                  className="primary"
                  onClick={handleCustomPathSubmit}
                  disabled={isValidating || !customPath.trim()}
                >
                  {isValidating ? 'Validating...' : 'Apply'}
                </button>
              </div>
              {customError && (
                <span className="runner-error">{customError}</span>
              )}
            </div>
          )}

          {error && !showCustomInput && (
            <span className="runner-error">{error}</span>
          )}

          {selectedRunner && !showCustomInput && (
            <span className="runner-path-hint">
              Path: {selectedRunner.path}
            </span>
          )}

          {!selectedRunner && !showCustomInput && runners.length === 0 && !isLoading && (
            <span className="runner-warning">
              No runners detected. Install Wine, Proton, or specify a custom path.
            </span>
          )}
        </div>

        <div className="settings-row">
          <label>Wine Prefix Path (optional)</label>
          <div className="settings-path-input">
            <input
              type="text"
              value={wine.prefix_path || ''}
              onChange={(e) =>
                updateWineSettings({ prefix_path: e.target.value || null })
              }
              placeholder="Default: ~/.local/share/gaveloc/prefix"
            />
            <button className="secondary" onClick={handleBrowsePrefix}>
              Browse
            </button>
          </div>
        </div>

        <div className="checkbox-row">
          <input
            type="checkbox"
            id="esync"
            checked={wine.esync}
            onChange={(e) => updateWineSettings({ esync: e.target.checked })}
          />
          <label htmlFor="esync">Enable Esync</label>
          <span className="hint">Event-based synchronization</span>
        </div>

        <div className="checkbox-row">
          <input
            type="checkbox"
            id="fsync"
            checked={wine.fsync}
            onChange={(e) => updateWineSettings({ fsync: e.target.checked })}
          />
          <label htmlFor="fsync">Enable Fsync</label>
          <span className="hint">Futex-based sync (requires kernel support)</span>
        </div>

        <div className="checkbox-row">
          <input
            type="checkbox"
            id="winesync"
            checked={wine.winesync}
            onChange={(e) => updateWineSettings({ winesync: e.target.checked })}
          />
          <label htmlFor="winesync">Enable Winesync</label>
          <span className="hint">Kernel-level Wine sync</span>
        </div>

        <div className="settings-row">
          <label>DXVK HUD (optional)</label>
          <input
            type="text"
            value={wine.dxvk_hud || ''}
            onChange={(e) =>
              updateWineSettings({ dxvk_hud: e.target.value || null })
            }
            placeholder="e.g., fps,frametimes,gpuload"
          />
          <span
            style={{
              fontSize: 'var(--font-size-xs)',
              color: 'var(--color-text-secondary)',
            }}
          >
            Comma-separated DXVK HUD options (devinfo, fps, frametimes, etc.)
          </span>
        </div>
      </div>
    </div>
  );
}
