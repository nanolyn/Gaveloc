import { usePatchStore } from '../../stores/patchStore';
import './PatchProgress.css';

function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${(bytes / Math.pow(k, i)).toFixed(1)} ${sizes[i]}`;
}

function formatSpeed(bytesPerSec: number): string {
  return `${formatBytes(bytesPerSec)}/s`;
}

export function PatchProgress() {
  const {
    isPatching,
    phase,
    currentIndex,
    totalPatches,
    currentVersionId,
    currentRepository,
    bytesProcessed,
    bytesTotal,
    speedBytesPerSec,
    error,
    cancelPatch,
    reset,
  } = usePatchStore();

  if (!isPatching && phase === 'Idle') {
    return null;
  }

  // Calculate progress percentage
  const progressPercent = bytesTotal > 0 ? (bytesProcessed / bytesTotal) * 100 : 0;

  // Get phase display text
  const getPhaseText = () => {
    switch (phase) {
      case 'Downloading':
        return 'Downloading';
      case 'Verifying':
        return 'Verifying';
      case 'Applying':
        return 'Applying';
      case 'Completed':
        return 'Completed';
      case 'Failed':
        return 'Failed';
      case 'Cancelled':
        return 'Cancelled';
      default:
        return 'Preparing';
    }
  };

  // Handle error state
  if (phase === 'Failed' && error) {
    return (
      <div className="patch-progress patch-progress-error">
        <div className="patch-progress-header">
          <span className="patch-progress-title">Update Failed</span>
        </div>
        <div className="patch-progress-error-message">{error}</div>
        <div className="patch-progress-actions">
          <button className="secondary" onClick={reset}>
            Dismiss
          </button>
        </div>
      </div>
    );
  }

  // Handle cancelled state
  if (phase === 'Cancelled') {
    return (
      <div className="patch-progress patch-progress-cancelled">
        <div className="patch-progress-header">
          <span className="patch-progress-title">Update Cancelled</span>
        </div>
        <div className="patch-progress-actions">
          <button className="secondary" onClick={reset}>
            Dismiss
          </button>
        </div>
      </div>
    );
  }

  // Handle completed state
  if (phase === 'Completed') {
    return (
      <div className="patch-progress patch-progress-completed">
        <div className="patch-progress-header">
          <span className="patch-progress-title">Update Complete</span>
          <span className="patch-progress-check">All patches applied successfully</span>
        </div>
        <div className="patch-progress-actions">
          <button className="secondary" onClick={reset}>
            Dismiss
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="patch-progress">
      <div className="patch-progress-header">
        <span className="patch-progress-title">Updating: {getPhaseText()}</span>
        <span className="patch-progress-counter">
          {currentIndex + 1} / {totalPatches}
        </span>
      </div>

      <div className="patch-progress-info">
        <span className="patch-progress-repo">{currentRepository}</span>
        <span className="patch-progress-version">{currentVersionId}</span>
      </div>

      <div className="patch-progress-bar-container">
        <div
          className="patch-progress-bar"
          style={{ width: `${progressPercent}%` }}
        />
      </div>

      <div className="patch-progress-stats">
        <span className="patch-progress-bytes">
          {formatBytes(bytesProcessed)} / {formatBytes(bytesTotal)}
        </span>
        {phase === 'Downloading' && speedBytesPerSec > 0 && (
          <span className="patch-progress-speed">{formatSpeed(speedBytesPerSec)}</span>
        )}
        <span className="patch-progress-percent">{progressPercent.toFixed(0)}%</span>
      </div>

      <div className="patch-progress-actions">
        <button
          className="secondary"
          onClick={cancelPatch}
          disabled={phase === 'Applying'}
          title={phase === 'Applying' ? 'Cannot cancel while applying patches' : ''}
        >
          Cancel
        </button>
      </div>
    </div>
  );
}
