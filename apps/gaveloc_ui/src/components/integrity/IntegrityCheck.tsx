import { useState } from 'react';
import { useIntegrityStore } from '../../stores/integrityStore';
import { useGameStore } from '../../stores/gameStore';
import './IntegrityCheck.css';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

export function IntegrityCheck() {
  const {
    isChecking,
    isRepairing,
    progress,
    result,
    error,
    startVerify,
    cancelVerify,
    repairFiles,
    clearError,
    reset,
  } = useIntegrityStore();

  const { checkBootUpdates } = useGameStore();

  const [showProblems, setShowProblems] = useState(false);
  const [repairResult, setRepairResult] = useState<{
    success: number;
    failure: number;
  } | null>(null);

  const handleVerify = async () => {
    setRepairResult(null);
    try {
      await startVerify();
    } catch (e) {
      // Error is handled by the store
    }
  };

  const handleRepair = async () => {
    if (!result || result.problems.length === 0) return;

    // Filter out unreadable files (user must fix permissions)
    const repairableFiles = result.problems.filter(
      (f) => f.status === 'Mismatch' || f.status === 'Missing'
    );

    if (repairableFiles.length === 0) {
      return;
    }

    try {
      const repairRes = await repairFiles(repairableFiles);
      setRepairResult({
        success: repairRes.success_count,
        failure: repairRes.failure_count,
      });
      // Clear the result so user knows they need to check updates
      reset();
    } catch (e) {
      // Error is handled by the store
    }
  };

  const handleCheckUpdatesAfterRepair = async () => {
    setRepairResult(null);
    await checkBootUpdates();
  };

  // Calculate repairable file count
  const repairableCount =
    result?.problems.filter(
      (f) => f.status === 'Mismatch' || f.status === 'Missing'
    ).length || 0;

  // Show repair success message
  if (repairResult) {
    return (
      <div className="integrity-check">
        <div className="integrity-check-header">
          <h4>Repair Complete</h4>
        </div>
        <div className="integrity-repair-result">
          <p className="text-success">
            Successfully removed {repairResult.success} corrupted file(s).
          </p>
          {repairResult.failure > 0 && (
            <p className="text-error">
              Failed to remove {repairResult.failure} file(s).
            </p>
          )}
          <p className="text-secondary mt-sm">
            Click "Check for Updates" to download and restore the removed files.
          </p>
        </div>
        <button className="primary mt-md" onClick={handleCheckUpdatesAfterRepair}>
          Check for Updates
        </button>
      </div>
    );
  }

  // Show progress during check
  if (isChecking && progress) {
    return (
      <div className="integrity-check integrity-check-active">
        <div className="integrity-check-header">
          <h4>Verifying Files</h4>
          <span className="integrity-check-counter">
            {progress.files_checked} / {progress.total_files}
          </span>
        </div>

        <div className="integrity-check-file">
          {progress.current_file.length > 50
            ? `...${progress.current_file.slice(-47)}`
            : progress.current_file}
        </div>

        <div className="integrity-progress-bar-container">
          <div
            className="integrity-progress-bar"
            style={{ width: `${progress.percent}%` }}
          />
        </div>

        <div className="integrity-check-stats">
          <span>
            {formatBytes(progress.bytes_processed)} /{' '}
            {formatBytes(progress.total_bytes)}
          </span>
          <span>{progress.percent.toFixed(0)}%</span>
        </div>

        <div className="integrity-check-actions">
          <button className="secondary" onClick={cancelVerify}>
            Cancel
          </button>
        </div>
      </div>
    );
  }

  // Show error
  if (error) {
    return (
      <div className="integrity-check integrity-check-error">
        <div className="integrity-check-header">
          <h4>Verification Failed</h4>
        </div>
        <div className="integrity-error-message">{error}</div>
        <div className="integrity-check-actions">
          <button className="secondary" onClick={clearError}>
            Dismiss
          </button>
          <button className="primary" onClick={handleVerify}>
            Retry
          </button>
        </div>
      </div>
    );
  }

  // Show results
  if (result) {
    const hasProblems =
      result.mismatch_count + result.missing_count + result.unreadable_count >
      0;

    return (
      <div
        className={`integrity-check ${hasProblems ? 'integrity-check-problems' : 'integrity-check-success'}`}
      >
        <div className="integrity-check-header">
          <h4>Verification Complete</h4>
          <button className="secondary small" onClick={handleVerify}>
            Re-check
          </button>
        </div>

        <div className="integrity-results">
          <div className="integrity-result-row">
            <span className="integrity-result-icon valid">&#10003;</span>
            <span className="integrity-result-label">Valid</span>
            <span className="integrity-result-count">{result.valid_count}</span>
          </div>
          {result.mismatch_count > 0 && (
            <div className="integrity-result-row">
              <span className="integrity-result-icon mismatch">!</span>
              <span className="integrity-result-label">Mismatch</span>
              <span className="integrity-result-count">
                {result.mismatch_count}
              </span>
            </div>
          )}
          {result.missing_count > 0 && (
            <div className="integrity-result-row">
              <span className="integrity-result-icon missing">&#10005;</span>
              <span className="integrity-result-label">Missing</span>
              <span className="integrity-result-count">
                {result.missing_count}
              </span>
            </div>
          )}
          {result.unreadable_count > 0 && (
            <div className="integrity-result-row">
              <span className="integrity-result-icon unreadable">&#8709;</span>
              <span className="integrity-result-label">
                Unreadable (check permissions)
              </span>
              <span className="integrity-result-count">
                {result.unreadable_count}
              </span>
            </div>
          )}
        </div>

        {hasProblems && result.problems.length > 0 && (
          <>
            <button
              className="integrity-toggle-problems"
              onClick={() => setShowProblems(!showProblems)}
            >
              {showProblems ? '▼' : '▶'} Show {result.problems.length} problem
              file(s)
            </button>

            {showProblems && (
              <div className="integrity-problems-list">
                {result.problems.slice(0, 50).map((file, idx) => (
                  <div key={idx} className="integrity-problem-item">
                    <span
                      className={`integrity-problem-status ${file.status.toLowerCase()}`}
                    >
                      [{file.status}]
                    </span>
                    <span className="integrity-problem-path">
                      {file.relative_path}
                    </span>
                  </div>
                ))}
                {result.problems.length > 50 && (
                  <div className="integrity-problem-more">
                    ...and {result.problems.length - 50} more
                  </div>
                )}
              </div>
            )}

            {repairableCount > 0 && (
              <div className="integrity-repair-section">
                <button
                  className="primary"
                  onClick={handleRepair}
                  disabled={isRepairing}
                >
                  {isRepairing
                    ? 'Repairing...'
                    : `Repair ${repairableCount} file(s)`}
                </button>
                <p className="integrity-repair-warning">
                  Repair will delete corrupted files. Run "Check for Updates"
                  after to restore them.
                </p>
              </div>
            )}
          </>
        )}

        {!hasProblems && (
          <p className="integrity-success-message">
            All files are valid. No issues found.
          </p>
        )}
      </div>
    );
  }

  // Default: Show verify button
  return (
    <div className="integrity-check">
      <div className="integrity-check-header">
        <h4>File Integrity</h4>
      </div>
      <p className="integrity-description">
        Verify game files for corruption or missing data.
      </p>
      <button
        className="secondary"
        onClick={handleVerify}
        disabled={isChecking}
      >
        {isChecking ? 'Checking...' : 'Verify Files'}
      </button>
    </div>
  );
}
