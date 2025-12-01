import { useState, useEffect, useRef } from 'react';
import { useAccountStore } from '../../stores/accountStore';
import { Icon } from '../Icon';
import type { Account } from '../../types';
import './AccountSwitcher.css';

interface AccountSwitcherProps {
  onAddAccount: () => void;
  onEditAccount: (account: Account) => void;
}

export function AccountSwitcher({
  onAddAccount,
  onEditAccount,
}: AccountSwitcherProps) {
  const { accounts, currentAccount, loading, loadAccounts, setCurrentAccount } =
    useAccountStore();
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    loadAccounts();
  }, [loadAccounts]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        dropdownRef.current &&
        !dropdownRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [isOpen]);

  const handleSelectAccount = async (account: Account) => {
    await setCurrentAccount(account.id);
    setIsOpen(false);
  };

  const handleEditClick = (e: React.MouseEvent, account: Account) => {
    e.stopPropagation();
    setIsOpen(false);
    onEditAccount(account);
  };

  if (loading) {
    return (
      <div className="account-switcher">
        <button className="account-switcher-button" disabled>
          Loading...
        </button>
      </div>
    );
  }

  return (
    <div className="account-switcher" ref={dropdownRef}>
      <button
        className="account-switcher-button"
        onClick={() => setIsOpen(!isOpen)}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
      >
        <span className="account-switcher-icon">
          <Icon name="user" size={16} />
        </span>
        <span className="account-switcher-name">
          {currentAccount?.username ?? 'No Account'}
        </span>
        <span className="account-switcher-chevron">
          <Icon name="caret-down" size={12} />
        </span>
      </button>

      {isOpen && (
        <div className="account-dropdown" role="listbox">
          {accounts.length > 0 ? (
            <>
              {accounts.map((account) => (
                <div
                  key={account.id}
                  className={`account-dropdown-item ${
                    account.id === currentAccount?.id ? 'active' : ''
                  }`}
                  role="option"
                  aria-selected={account.id === currentAccount?.id}
                  onClick={() => handleSelectAccount(account)}
                >
                  <div className="account-dropdown-item-info">
                    <span className="account-dropdown-item-name">
                      {account.username}
                    </span>
                  </div>
                  <button
                    className="account-dropdown-item-edit"
                    onClick={(e) => handleEditClick(e, account)}
                    aria-label={`Edit ${account.username}`}
                  >
                    <Icon name="pencil" size={14} />
                  </button>
                </div>
              ))}
              <div className="account-dropdown-divider" />
            </>
          ) : null}
          <button className="account-dropdown-add" onClick={onAddAccount}>
            <Icon name="user-plus" size={14} />
            Add Account
          </button>
        </div>
      )}
    </div>
  );
}
