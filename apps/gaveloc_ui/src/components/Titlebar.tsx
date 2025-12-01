import { getCurrentWindow } from '@tauri-apps/api/window';
import { Icon } from './Icon';
import './Titlebar.css';

export function Titlebar() {
  const appWindow = getCurrentWindow();

  const handleMinimize = () => {
    appWindow.minimize();
  };

  const handleMaximize = () => {
    appWindow.toggleMaximize();
  };

  const handleClose = () => {
    appWindow.close();
  };

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar-icon">
         {/* Potentially an App Icon here if we had one, or empty spacer */}
      </div>
      <span className="titlebar-title" data-tauri-drag-region>
        Gaveloc
      </span>
      <div className="titlebar-controls">
        <button
          className="titlebar-button"
          onClick={handleMinimize}
          aria-label="Minimize"
        >
          <Icon name="minus" size={12} />
        </button>
        <button
          className="titlebar-button"
          onClick={handleMaximize}
          aria-label="Maximize"
        >
          <Icon name="corners-out" size={12} />
        </button>
        <button
          className="titlebar-button titlebar-close"
          onClick={handleClose}
          aria-label="Close"
        >
          <Icon name="x" size={12} />
        </button>
      </div>
    </div>
  );
}
