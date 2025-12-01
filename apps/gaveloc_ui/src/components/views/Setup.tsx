import { useEffect, useState } from 'react';
import { useSettingsStore } from '../../stores/settingsStore';
import { useUIStore } from '../../stores/uiStore';
import { open } from '@tauri-apps/plugin-dialog';
import { Icon } from '../Icon';
import './Setup.css';

export function Setup() {
  const {
    settings,
    saveSettings,
    detectGameInstall,
    getDefaultInstallPath
  } = useSettingsStore();
  const { setView } = useUIStore();

  const [step, setStep] = useState<'detecting' | 'found' | 'manual'>('detecting');
  const [detectedPath, setDetectedPath] = useState<string | null>(null);
  const [manualPath, setManualPath] = useState('');
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const init = async () => {
      const path = await detectGameInstall();
      if (path) {
        setDetectedPath(path);
        setStep('found');
      } else {
        const defaultPath = await getDefaultInstallPath();
        setManualPath(defaultPath);
        setStep('manual');
      }
    };
    init();
  }, [detectGameInstall, getDefaultInstallPath]);

  const handleConfirmFound = async () => {
    if (!settings || !detectedPath || isSubmitting) return;

    setIsSubmitting(true);
    setError(null);
    try {
      await saveSettings({
        ...settings,
        game: { ...settings.game, path: detectedPath }
      });
      setView('home');
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to save settings';
      setError(message);
      setIsSubmitting(false);
    }
  };

  const handleManualSubmit = async () => {
    if (!settings || isSubmitting) return;

    setIsSubmitting(true);
    setError(null);
    try {
      let finalPath = manualPath;
      if (!finalPath.trim()) {
        finalPath = await getDefaultInstallPath();
      }

      await saveSettings({
        ...settings,
        game: { ...settings.game, path: finalPath }
      });
      setView('home');
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to save settings';
      setError(message);
      setIsSubmitting(false);
    }
  };

  const handleBrowse = async () => {
    const selected = await open({
      directory: true,
      multiple: false,
      defaultPath: manualPath,
    });

    if (selected && typeof selected === 'string') {
      setManualPath(selected);
    }
  };

  if (step === 'detecting') {
    return (
      <div className="setup-view">
        <div className="setup-loading">
          <Icon name="spinner" size={24} className="spinning" />
          <span>Searching for FFXIV...</span>
        </div>
      </div>
    );
  }

  if (step === 'found') {
    return (
      <div className="setup-view">
        <h2 className="setup-title">Game Found!</h2>
        <p className="setup-description">We found an existing FFXIV installation:</p>
        <code className="setup-detected-path">{detectedPath}</code>
        {error && <div className="setup-error">{error}</div>}
        <div className="setup-buttons">
          <button className="primary" disabled={isSubmitting} onClick={handleConfirmFound}>
            {isSubmitting ? (
              <>
                <Icon name="spinner" size={14} className="spinning" />
                Saving...
              </>
            ) : (
              'Use this installation'
            )}
          </button>
          <button className="secondary" disabled={isSubmitting} onClick={() => setStep('manual')}>
            Use different location
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="setup-view">
      <h2 className="setup-title">Setup FFXIV</h2>
      <p className="setup-description">
        Game not found. Select where you want to install it, or locate an existing installation.
      </p>

      {error && <div className="setup-error">{error}</div>}

      <div className="setup-path-input">
        <input
          type="text"
          value={manualPath}
          onChange={(e) => setManualPath(e.target.value)}
          placeholder="Installation Path"
        />
        <button className="secondary" onClick={handleBrowse}>
          <Icon name="folder-open" size={14} />
        </button>
      </div>

      <p className="setup-hint">If the folder is empty, the game will be installed there.</p>

      <div className="setup-buttons">
        <button className="primary" disabled={isSubmitting} onClick={handleManualSubmit}>
          {isSubmitting ? (
            <>
              <Icon name="spinner" size={14} className="spinning" />
              Saving...
            </>
          ) : (
            'Continue'
          )}
        </button>
      </div>
    </div>
  );
}
