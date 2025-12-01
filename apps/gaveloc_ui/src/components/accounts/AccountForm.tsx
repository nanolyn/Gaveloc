import { useState, useEffect } from 'react';
import { useAccountStore } from '../../stores/accountStore';
import type { Account } from '../../types';
import './AccountForm.css';

interface AccountFormProps {
  editAccount?: Account | null;
  onCancel?: () => void;
  onSuccess?: () => void;
}

export function AccountForm({
  editAccount,
  onCancel,
  onSuccess,
}: AccountFormProps) {
  const { addAccount, updateAccount, removeAccount, storePassword, deletePassword, hasStoredPassword } =
    useAccountStore();

  const [username, setUsername] = useState('');
  const [isSteam, setIsSteam] = useState(false);
  const [isFreeTrial, setIsFreeTrial] = useState(false);
  const [useOtp, setUseOtp] = useState(false);
  const [password, setPassword] = useState('');
  const [savePassword, setSavePassword] = useState(false);
  const [hasExistingPassword, setHasExistingPassword] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isEditing = !!editAccount;

  useEffect(() => {
    if (editAccount) {
      setUsername(editAccount.username);
      setIsSteam(editAccount.is_steam);
      setIsFreeTrial(editAccount.is_free_trial);
      setUseOtp(editAccount.use_otp);
      setPassword('');
      setSavePassword(false);
      hasStoredPassword(editAccount.id).then(setHasExistingPassword);
    } else {
      setUsername('');
      setIsSteam(false);
      setIsFreeTrial(false);
      setUseOtp(false);
      setPassword('');
      setSavePassword(false);
      setHasExistingPassword(false);
    }
    setError(null);
  }, [editAccount, hasStoredPassword]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!username.trim()) {
      setError('Username is required');
      return;
    }

    setSaving(true);
    try {
      const request = {
        username: username.trim(),
        is_steam: isSteam,
        is_free_trial: isFreeTrial,
        use_otp: useOtp,
      };

      let account: Account;
      if (isEditing) {
        account = await updateAccount(request);
      } else {
        account = await addAccount(request);
      }

      if (savePassword && password) {
        await storePassword(account.id, password);
      } else if (isEditing && !savePassword && hasExistingPassword) {
        await deletePassword(account.id);
      }

      onSuccess?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!editAccount) return;

    const confirmed = window.confirm(
      `Delete account "${editAccount.username}"? This will also remove any saved passwords.`
    );

    if (!confirmed) return;

    setDeleting(true);
    try {
      await removeAccount(editAccount.id);
      onSuccess?.();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <form className="account-form" onSubmit={handleSubmit}>
      {error && <div className="form-error">{error}</div>}

      <div className="form-field">
        <label htmlFor="username">Username</label>
        <input
          id="username"
          type="text"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          placeholder="Square Enix ID"
          disabled={isEditing}
          autoFocus={!isEditing}
        />
      </div>

      <div className="form-row">
        <div className="form-field form-field-flex">
          <label htmlFor="password">
            Password
            {hasExistingPassword && <span className="password-badge">Saved</span>}
          </label>
          <input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={hasExistingPassword ? 'Update password' : 'Password'}
          />
        </div>
        <label className="form-checkbox form-checkbox-save">
          <input
            type="checkbox"
            checked={savePassword || hasExistingPassword}
            onChange={(e) => setSavePassword(e.target.checked)}
          />
          <span>Save</span>
        </label>
      </div>

      <div className="form-toggles">
        <label className="form-toggle">
          <input
            type="checkbox"
            checked={isSteam}
            onChange={(e) => setIsSteam(e.target.checked)}
          />
          <span>Steam</span>
        </label>
        <label className="form-toggle">
          <input
            type="checkbox"
            checked={isFreeTrial}
            onChange={(e) => setIsFreeTrial(e.target.checked)}
          />
          <span>Free Trial</span>
        </label>
        <label className="form-toggle">
          <input
            type="checkbox"
            checked={useOtp}
            onChange={(e) => setUseOtp(e.target.checked)}
          />
          <span>OTP</span>
        </label>
      </div>

      <div className="form-actions">
        {isEditing && (
          <button
            type="button"
            className="danger"
            onClick={handleDelete}
            disabled={deleting || saving}
          >
            {deleting ? 'Deleting...' : 'Delete'}
          </button>
        )}
        <div className="form-actions-right">
          {onCancel && (
            <button
              type="button"
              className="secondary"
              onClick={onCancel}
              disabled={saving || deleting}
            >
              Cancel
            </button>
          )}
          <button
            type="submit"
            className="primary"
            disabled={saving || deleting}
          >
            {saving ? 'Saving...' : isEditing ? 'Save' : 'Add Account'}
          </button>
        </div>
      </div>
    </form>
  );
}
