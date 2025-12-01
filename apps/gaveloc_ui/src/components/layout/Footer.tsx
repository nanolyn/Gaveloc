import { useEffect, useState, useRef } from 'react';
import { useAccountStore } from '../../stores/accountStore';
import { useLaunchStore } from '../../stores/launchStore';
import { useGameStore } from '../../stores/gameStore';
import { useUIStore } from '../../stores/uiStore';
import { Icon } from '../Icon';
import './Footer.css';

export function Footer() {
  const { accounts, currentAccount, setCurrentAccount } = useAccountStore();
  const { launchGame, isLaunching, isRunning } = useLaunchStore();
  const { setView } = useUIStore();
  const {
    bootUpdates,
    gameUpdates,
    isCheckingBootUpdates,
    isCheckingGameUpdates,
    checkBootUpdates,
    checkGameUpdates,
  } = useGameStore();

  const [dropdownOpen, setDropdownOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    checkBootUpdates();
  }, [checkBootUpdates]);

  useEffect(() => {
    if (currentAccount) {
      checkGameUpdates(currentAccount.id);
    }
  }, [currentAccount, checkGameUpdates]);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setDropdownOpen(false);
      }
    };

    if (dropdownOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [dropdownOpen]);

  const handlePlay = async () => {
    if (currentAccount) {
      try {
        await launchGame(currentAccount.id);
      } catch (e) {
        console.error(e);
      }
    }
  };

  const handleAccountSelect = (accountId: string) => {
    setCurrentAccount(accountId);
    setDropdownOpen(false);
  };

  const hasUpdates = bootUpdates?.has_updates || gameUpdates?.has_updates;
  const isCheckingUpdates = isCheckingBootUpdates || isCheckingGameUpdates;
  const totalPatches = (bootUpdates?.patches.length || 0) + (gameUpdates?.patches.length || 0);

  // Don't show footer if no account is selected
  if (!currentAccount) {
    return null;
  }

  return (
    <footer className="app-footer">
      <div className="footer-content">
        {/* Account Dropdown */}
        <div className="footer-left">
          <div className="account-dropdown" ref={dropdownRef}>
            <button
              className="dropdown-trigger"
              onClick={() => setDropdownOpen(!dropdownOpen)}
            >
              <div className="dropdown-avatar">
                {currentAccount.username.charAt(0).toUpperCase()}
              </div>
              <span className="dropdown-username">{currentAccount.username}</span>
              <Icon name="caret-down" size={14} className={`dropdown-caret ${dropdownOpen ? 'open' : ''}`} />
            </button>

            {dropdownOpen && (
              <div className="dropdown-menu dropdown-menu-up">
                {accounts.map((account) => (
                  <button
                    key={account.id}
                    className={`dropdown-item ${account.id === currentAccount.id ? 'selected' : ''}`}
                    onClick={() => handleAccountSelect(account.id)}
                  >
                    <div className="dropdown-item-avatar">
                      {account.username.charAt(0).toUpperCase()}
                    </div>
                    <div className="dropdown-item-details">
                      <span className="dropdown-item-name">{account.username}</span>
                      <div className="dropdown-item-badges">
                        {account.is_steam && <span className="badge badge-steam">Steam</span>}
                        {account.is_free_trial && <span className="badge badge-trial">Trial</span>}
                      </div>
                    </div>
                    {account.id === currentAccount.id && (
                      <Icon name="check" size={18} className="dropdown-item-check" />
                    )}
                  </button>
                ))}
                <div className="dropdown-divider" />
                <button
                  className="dropdown-item dropdown-item-action"
                  onClick={() => {
                    setDropdownOpen(false);
                    setView('accounts');
                  }}
                >
                  <Icon name="user-circle" size={18} />
                  <span>Manage Accounts</span>
                </button>
              </div>
            )}
          </div>
        </div>

        {/* Update Status */}
        <div className="footer-center">
          <div className="update-status-badge">
            {isCheckingUpdates ? (
              <span className="status-checking">
                <Icon name="spinner" size={16} className="spinning" />
                Checking...
              </span>
            ) : hasUpdates ? (
              <span className="status-available">
                <Icon name="download" size={16} />
                {totalPatches} update{totalPatches !== 1 ? 's' : ''}
              </span>
            ) : (
              <span className="status-current">
                <Icon name="check-circle" size={16} />
                Up to date
              </span>
            )}
          </div>
        </div>

        {/* Play Button */}
        <div className="footer-right">
          <button
            className="play-button"
            onClick={handlePlay}
            disabled={isLaunching || isRunning}
          >
            {isLaunching ? (
              <>
                <Icon name="spinner" size={20} className="spinning" />
                Launching...
              </>
            ) : isRunning ? (
              <>
                <Icon name="game-controller" size={20} />
                Running
              </>
            ) : (
              <>
                <Icon name="play" size={20} />
                Play Game
              </>
            )}
          </button>
        </div>
      </div>
    </footer>
  );
}
