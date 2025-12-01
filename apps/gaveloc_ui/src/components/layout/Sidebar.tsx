import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { Icon, IconName } from '../Icon';
import { useUIStore, View } from '../../stores/uiStore';
import './Sidebar.css';

export function Sidebar() {
  const { currentView, setView } = useUIStore();

  const NavItem = ({ view, icon, label }: { view: View; icon: IconName; label: string }) => (
    <button
      className={`sidebar-item ${currentView === view ? 'active' : ''}`}
      onClick={() => setView(view)}
      title={label}
    >
      <Icon name={icon} size={20} weight="duotone" />
      <span className="sidebar-label">{label}</span>
    </button>
  );

  const openSettings = async () => {
    const existingWindow = await WebviewWindow.getByLabel('settings');
    if (existingWindow) {
      await existingWindow.setFocus();
      return;
    }

    const settingsWindow = new WebviewWindow('settings', {
      url: '?window=settings',
      title: 'Settings',
      width: 500,
      height: 550,
      center: true,
      resizable: false,
      decorations: false,
    });

    settingsWindow.once('tauri://error', (e) => {
      console.error('Failed to create settings window:', e);
    });
  };

  return (
    <aside className="sidebar">
      <div className="sidebar-header">
        <div className="sidebar-logo">
          <Icon name="game-controller" size={28} weight="duotone" />
          <span className="sidebar-logo-text">GAVELOC</span>
        </div>
      </div>

      <nav className="sidebar-nav">
        <NavItem view="home" icon="house" label="Home" />
        <NavItem view="accounts" icon="user-circle" label="Accounts" />
      </nav>

      <div className="sidebar-footer">
        <button
          className="sidebar-item sidebar-settings"
          onClick={openSettings}
          title="Settings"
        >
          <Icon name="gear" size={20} weight="duotone" />
          <span className="sidebar-label">Settings</span>
        </button>
      </div>
    </aside>
  );
}
