import { useEffect } from 'react';
import { useLaunchStore } from '../../stores/launchStore';
import { useAuthStore } from '../../stores/authStore';
import { useAccountStore } from '../../stores/accountStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { useGameStore } from '../../stores/gameStore';
import { usePatchStore } from '../../stores/patchStore';
import './LaunchButton.css';

export function LaunchButton() {
  const { currentAccount } = useAccountStore();
  const { loginState } = useAuthStore();
  const { settings } = useSettingsStore();
  const { versions, bootUpdates, gameUpdates } = useGameStore();
  const { isPatching } = usePatchStore();
  const {
    isLaunching,
    isRunning,
    error,
    preflight,
    launchGame,
    checkStatus,
    runPreflight,
    clearError,
  } = useLaunchStore();

  const isLoggedIn = loginState === 'LoggedIn';
  const hasGamePath = !!settings?.game?.path;
  const isGameValid = versions?.game_path_valid ?? false;
  const hasUpdates = bootUpdates?.has_updates || gameUpdates?.has_updates;

  // Poll for game status periodically when running
  useEffect(() => {
    if (!isRunning && !isLaunching) return;

    const interval = setInterval(() => {
      checkStatus();
    }, 5000); // Check every 5 seconds

    return () => clearInterval(interval);
  }, [isRunning, isLaunching, checkStatus]);

  // Run preflight when relevant state changes
  useEffect(() => {
    if (currentAccount && isLoggedIn && hasGamePath && isGameValid) {
      runPreflight(currentAccount.id);
    }
  }, [currentAccount, isLoggedIn, hasGamePath, isGameValid, runPreflight]);

  const handleLaunch = async () => {
    if (!currentAccount) return;
    clearError();

    try {
      await launchGame(currentAccount.id);
    } catch {
      // Error is set in store
    }
  };

  // Determine button state
  const canLaunch =
    isLoggedIn &&
    hasGamePath &&
    isGameValid &&
    !isPatching &&
    !isLaunching &&
    !isRunning &&
    !hasUpdates &&
    (preflight?.can_launch ?? false);

  const getButtonText = () => {
    if (isLaunching) return 'Launching...';
    if (isRunning) return 'Game Running';
    if (!isLoggedIn) return 'Login Required';
    if (!hasGamePath) return 'Set Game Path';
    if (!isGameValid) return 'Invalid Game Path';
    if (isPatching) return 'Patching...';
    if (hasUpdates) return 'Updates Available';
    return 'Launch Game';
  };

  const getButtonClass = () => {
    if (isRunning) return 'launch-button running';
    if (isLaunching) return 'launch-button launching';
    if (!canLaunch) return 'launch-button disabled';
    return 'launch-button';
  };

  return (
    <div className="launch-button-container">
      <button
        className={getButtonClass()}
        onClick={handleLaunch}
        disabled={!canLaunch}
      >
        {isLaunching && <span className="launch-spinner" />}
        <span className="launch-text">{getButtonText()}</span>
      </button>

      {error && (
        <div className="launch-error" onClick={clearError}>
          {error}
          <span className="launch-error-dismiss">dismiss</span>
        </div>
      )}

      {preflight && preflight.warnings.length > 0 && !error && (
        <div className="launch-warnings">
          {preflight.warnings.map((warning, i) => (
            <div key={i} className="launch-warning">
              {warning}
            </div>
          ))}
        </div>
      )}

      {preflight && !preflight.can_launch && preflight.issues.length > 0 && (
        <div className="launch-issues">
          {preflight.issues.map((issue, i) => (
            <div key={i} className="launch-issue">
              {issue}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
