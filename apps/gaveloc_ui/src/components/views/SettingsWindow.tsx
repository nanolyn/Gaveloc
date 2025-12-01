import { getCurrentWindow } from '@tauri-apps/api/window';
import { Settings } from './Settings';
import { Icon } from '../Icon';
import './SettingsWindow.css';

export function SettingsWindow() {
  const appWindow = getCurrentWindow();

  const handleClose = () => {
    appWindow.close();
  };

  return (
    <div className="settings-window">
      <div className="settings-window-titlebar" data-tauri-drag-region>
        <span className="settings-window-title" data-tauri-drag-region>
          Settings
        </span>
        <button
          className="settings-window-close"
          onClick={handleClose}
          aria-label="Close"
        >
          <Icon name="x" size={12} />
        </button>
      </div>
      <div className="settings-window-content">
        <Settings />
      </div>
    </div>
  );
}
