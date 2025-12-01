import { useEffect } from 'react';
import { useGameStore } from '../../stores/gameStore';
import { useSettingsStore } from '../../stores/settingsStore';
import { useAuthStore } from '../../stores/authStore';
import { useAccountStore } from '../../stores/accountStore';
import { usePatchStore } from '../../stores/patchStore';
import { Icon } from '../Icon';
import { PatchProgress } from '../patching/PatchProgress';
import { IntegrityCheck } from '../integrity/IntegrityCheck';
import { LaunchButton } from '../launcher/LaunchButton';
import './GameStatus.css';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function GameStatus() {
  const { settings } = useSettingsStore();
  const { currentAccount } = useAccountStore();
  const { loginState } = useAuthStore();
  const {
    versions,
    bootUpdates,
    gameUpdates,
    isLoadingVersions,
    isCheckingBootUpdates,
    isCheckingGameUpdates,
    error,
    loadVersions,
    checkBootUpdates,
    checkGameUpdates,
    clearError,
  } = useGameStore();

  const {
    isPatching,
    phase: patchPhase,
    startBootPatch,
    startGamePatch,
  } = usePatchStore();

  const gamePath = settings?.game?.path;
  const isLoggedIn = loginState === 'LoggedIn';

  // Load versions when game path changes
  useEffect(() => {
    if (gamePath) {
      loadVersions();
    }
  }, [gamePath, loadVersions]);

  const handleCheckBootUpdates = async () => {
    await checkBootUpdates();
  };

  const handleCheckGameUpdates = async () => {
    if (currentAccount && isLoggedIn) {
      await checkGameUpdates(currentAccount.id);
    }
  };

  const handleCheckAllUpdates = async () => {
    await checkBootUpdates();
    if (currentAccount && isLoggedIn) {
      await checkGameUpdates(currentAccount.id);
    }
  };

  const handleStartPatching = async () => {
    // Start boot patches first, then game patches
    if (bootUpdates?.has_updates) {
      await startBootPatch();
    } else if (gameUpdates?.has_updates && currentAccount && isLoggedIn) {
      await startGamePatch(currentAccount.id);
    }
  };

  // Calculate total update size
  const totalUpdateSize =
    (bootUpdates?.total_size_bytes || 0) + (gameUpdates?.total_size_bytes || 0);
  const hasUpdates = bootUpdates?.has_updates || gameUpdates?.has_updates;

  if (!gamePath) {
    return (
      <div className="game-status card">
        <h3>Game Status</h3>
        <div className="game-status-empty">
          <p className="text-secondary">No game path configured.</p>
          <p className="text-secondary mt-sm">
            Open Settings to set your FFXIV installation path.
          </p>
        </div>
      </div>
    );
  }

  if (!versions?.game_path_valid) {
    return (
      <div className="game-status card">
        <h3>Game Status</h3>
        <div className="game-status-empty">
          <p className="text-warning">Invalid game installation detected.</p>
          <p className="text-secondary mt-sm">
            The configured path doesn't contain a valid FFXIV installation.
          </p>
        </div>
      </div>
    );
  }

  const isChecking = isCheckingBootUpdates || isCheckingGameUpdates;

  return (
    <div className="game-status card">
      <div className="game-status-header">
        <h3>Game Status</h3>
        <button
          className="secondary game-status-refresh"
          onClick={handleCheckAllUpdates}
          disabled={isChecking || isLoadingVersions}
          title="Check for updates"
        >
          {isChecking ? (
            <Icon name="spinner" size={16} className="spinning" />
          ) : (
            <Icon name="arrow-clockwise" size={16} />
          )}
        </button>
      </div>

      {error && (
        <div className="game-status-error" onClick={clearError}>
          {error}
          <span className="game-status-error-dismiss">dismiss</span>
        </div>
      )}

      {isLoadingVersions ? (
        <div className="game-status-loading">
          <Icon name="spinner" size={16} className="spinning" />
          <span>Loading game information...</span>
        </div>
      ) : (
        <>
          <div className="game-status-section">
            <h4>Installed Versions</h4>
            <div className="game-status-versions">
              <div className="game-status-version-row">
                <span className="game-status-version-label">Boot</span>
                <span className="game-status-version-value">
                  {versions?.boot || 'Not found'}
                </span>
              </div>
              <div className="game-status-version-row">
                <span className="game-status-version-label">Game</span>
                <span className="game-status-version-value">
                  {versions?.game || 'Not found'}
                </span>
              </div>
            </div>
          </div>

          {versions?.expansions && versions.expansions.length > 0 && (
            <div className="game-status-section">
              <h4>Expansions</h4>
              <div className="game-status-expansions">
                {versions.expansions.map((exp) => (
                  <div
                    key={exp.name}
                    className={`game-status-expansion ${exp.installed ? '' : 'not-installed'}`}
                  >
                    <span className="game-status-expansion-name">{exp.name}</span>
                    <span className="game-status-expansion-status">
                      {exp.installed ? (
                        <span className="text-success">Installed</span>
                      ) : (
                        <span className="text-secondary">Not installed</span>
                      )}
                    </span>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="game-status-section">
            <h4>Updates</h4>
            <div className="game-status-updates">
              <div className="game-status-update-row">
                <div className="game-status-update-info">
                  <span className="game-status-update-label">Boot Updates</span>
                  {bootUpdates ? (
                    bootUpdates.has_updates ? (
                      <span className="text-warning">
                        {bootUpdates.patches.length} update(s) available ({formatBytes(bootUpdates.total_size_bytes)})
                      </span>
                    ) : (
                      <span className="text-success">Up to date</span>
                    )
                  ) : (
                    <span className="text-secondary">Not checked</span>
                  )}
                </div>
                <button
                  className="secondary small"
                  onClick={handleCheckBootUpdates}
                  disabled={isCheckingBootUpdates}
                >
                  {isCheckingBootUpdates ? 'Checking...' : 'Check'}
                </button>
              </div>

              <div className="game-status-update-row">
                <div className="game-status-update-info">
                  <span className="game-status-update-label">Game Updates</span>
                  {!isLoggedIn ? (
                    <span className="text-secondary">Login required</span>
                  ) : gameUpdates ? (
                    gameUpdates.has_updates ? (
                      <span className="text-warning">
                        {gameUpdates.patches.length} update(s) available ({formatBytes(gameUpdates.total_size_bytes)})
                      </span>
                    ) : gameUpdates.error ? (
                      <span className="text-error">{gameUpdates.error}</span>
                    ) : (
                      <span className="text-success">Up to date</span>
                    )
                  ) : (
                    <span className="text-secondary">Not checked</span>
                  )}
                </div>
                <button
                  className="secondary small"
                  onClick={handleCheckGameUpdates}
                  disabled={isCheckingGameUpdates || !isLoggedIn}
                  title={!isLoggedIn ? 'Login required to check game updates' : ''}
                >
                  {isCheckingGameUpdates ? 'Checking...' : 'Check'}
                </button>
              </div>
            </div>
          </div>

          {hasUpdates && !isPatching && patchPhase === 'Idle' && (
            <div className="game-status-section">
              <h4>Available Patches</h4>
              <div className="game-status-patches">
                {bootUpdates?.patches.map((patch) => (
                  <div key={`boot-${patch.version_id}`} className="game-status-patch">
                    <span className="game-status-patch-repo">{patch.repository}</span>
                    <span className="game-status-patch-version">{patch.version_id}</span>
                    <span className="game-status-patch-size">{formatBytes(patch.size_bytes)}</span>
                  </div>
                ))}
                {gameUpdates?.patches.map((patch) => (
                  <div key={`game-${patch.version_id}`} className="game-status-patch">
                    <span className="game-status-patch-repo">{patch.repository}</span>
                    <span className="game-status-patch-version">{patch.version_id}</span>
                    <span className="game-status-patch-size">{formatBytes(patch.size_bytes)}</span>
                  </div>
                ))}
              </div>
              <button
                className="primary game-status-update-button"
                onClick={handleStartPatching}
                disabled={isPatching}
              >
                Update Now ({formatBytes(totalUpdateSize)})
              </button>
            </div>
          )}

          {/* Show inline patch progress when patching */}
          <PatchProgress />

          {/* Integrity check section */}
          <IntegrityCheck />

          {/* Launch section - shown when game is up to date */}
          {!hasUpdates && !isPatching && versions?.game_path_valid && (
            <div className="game-status-section game-status-launch">
              <LaunchButton />
            </div>
          )}
        </>
      )}
    </div>
  );
}
