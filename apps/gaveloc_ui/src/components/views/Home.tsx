import { useAccountStore } from '../../stores/accountStore';
import { useUIStore } from '../../stores/uiStore';
import { NewsFeed } from './NewsFeed';
import { Icon } from '../Icon';
import './Home.css';

export function Home() {
  const { currentAccount } = useAccountStore();
  const { setView } = useUIStore();

  // Empty state - no account selected
  if (!currentAccount) {
    return (
      <div className="home-view">
        <div className="home-empty">
          <div className="home-empty-icon">
            <Icon name="user-circle" size={48} />
          </div>
          <h2 className="home-empty-title">Welcome to Gaveloc</h2>
          <p className="home-empty-text">Add an account to get started</p>
          <button className="primary" onClick={() => setView('accounts')}>
            <Icon name="user-plus" size={16} />
            Add Account
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="home-view">
      <NewsFeed />
    </div>
  );
}
