import { useEffect } from 'react';
import { Layout } from './components/layout/Layout';
import { Home } from './components/views/Home';
import { Accounts } from './components/views/Accounts';
import { Setup } from './components/views/Setup';
import { SettingsWindow } from './components/views/SettingsWindow';
import { useUIStore } from './stores/uiStore';
import { useAccountStore } from './stores/accountStore';
import { useLaunchStore } from './stores/launchStore';
import { useSettingsStore } from './stores/settingsStore';
import './App.css';

function App() {
  const { currentView, setView } = useUIStore();
  const { loadAccounts } = useAccountStore();
  const { checkStatus } = useLaunchStore();
  const { loadSettings, settings, loading: settingsLoading } = useSettingsStore();

  const isSettingsWindow = new URLSearchParams(window.location.search).get('window') === 'settings';

  useEffect(() => {
    if (isSettingsWindow) return;

    // Initialize main window only
    loadAccounts();
    loadSettings();
    
    const statusInterval = setInterval(() => {
        checkStatus();
    }, 5000);

    return () => clearInterval(statusInterval);
  }, [loadAccounts, checkStatus, loadSettings, isSettingsWindow]);

  // Redirect to setup if no game path is configured
  useEffect(() => {
    if (isSettingsWindow || settingsLoading) return;
    
    if (settings && !settings.game.path) {
        setView('setup');
    }
  }, [settings, settingsLoading, setView, isSettingsWindow]);

  if (isSettingsWindow) {
    return <SettingsWindow />;
  }

  const renderView = () => {
    switch (currentView) {
      case 'home':
        return <Home />;
      case 'accounts':
        return <Accounts />;
      case 'setup':
        return <Setup />;
      default:
        return <Home />;
    }
  };

  return (
    <Layout>
      {renderView()}
    </Layout>
  );
}

export default App;