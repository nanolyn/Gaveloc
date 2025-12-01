import { useState } from 'react';
import { useAccountStore } from '../../stores/accountStore';
import { Icon } from '../Icon';
import { Modal } from '../Modal';
import { AccountForm } from '../accounts/AccountForm';
import { Account } from '../../types';
import './Accounts.css';

export function Accounts() {
  const { accounts, currentAccount, setCurrentAccount } = useAccountStore();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingAccount, setEditingAccount] = useState<Account | null>(null);

  const handleAdd = () => {
    setEditingAccount(null);
    setIsModalOpen(true);
  };

  const handleEdit = (e: React.MouseEvent, account: Account) => {
    e.stopPropagation();
    setEditingAccount(account);
    setIsModalOpen(true);
  };

  const handleClose = () => {
    setIsModalOpen(false);
    setEditingAccount(null);
  };

  return (
    <div className="accounts-view">
      <div className="accounts-header">
        <h2 className="accounts-title">Accounts</h2>
        <button className="primary" onClick={handleAdd}>
          <Icon name="plus" size={14} />
          Add Account
        </button>
      </div>

      {accounts.length === 0 ? (
        <div className="accounts-empty">
          <Icon name="user-circle" size={40} />
          <p>No accounts yet</p>
          <button className="secondary" onClick={handleAdd}>
            Add your first account
          </button>
        </div>
      ) : (
        <div className="accounts-list">
          {accounts.map((account) => (
            <div
              key={account.id}
              className={`account-card ${currentAccount?.id === account.id ? 'selected' : ''}`}
              onClick={() => setCurrentAccount(account.id)}
            >
              <div className="account-card-avatar">
                {account.username.charAt(0).toUpperCase()}
              </div>
              <div className="account-card-info">
                <span className="account-card-name">{account.username}</span>
                <div className="account-card-meta">
                  {account.is_free_trial && (
                    <span className="account-card-badge">Trial</span>
                  )}
                  {account.is_steam && (
                    <span className="account-card-badge">Steam</span>
                  )}
                </div>
              </div>
              <div className="account-card-actions">
                {currentAccount?.id === account.id && (
                  <span className="account-card-selected">
                    <Icon name="check" size={14} />
                  </span>
                )}
                <button
                  className="account-card-edit"
                  onClick={(e) => handleEdit(e, account)}
                  title="Edit account"
                >
                  <Icon name="pencil" size={14} />
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      <Modal
        isOpen={isModalOpen}
        onClose={handleClose}
        title={editingAccount ? 'Edit Account' : 'Add Account'}
      >
        <AccountForm
          editAccount={editingAccount}
          onCancel={handleClose}
          onSuccess={handleClose}
        />
      </Modal>
    </div>
  );
}
