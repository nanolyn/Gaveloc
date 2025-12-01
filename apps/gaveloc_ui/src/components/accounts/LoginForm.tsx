import { useState, useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useAuthStore } from '../../stores/authStore';
import type { Account, LoginErrorType } from '../../types';
import './LoginForm.css';

interface LoginFormProps {
  account: Account | null;
  onCancel?: () => void;
  onLoginSuccess?: () => void;
}

const ERROR_MESSAGES: Record<LoginErrorType, string> = {
  invalid_credentials: 'Invalid username or password. Please try again.',
  invalid_otp: 'Invalid one-time password. Please check and try again.',
  account_locked: 'Your account has been locked. Please contact Square Enix support.',
  maintenance: 'The server is currently under maintenance. Please try again later.',
  rate_limited: 'Too many login attempts. Please wait a few minutes and try again.',
  no_subscription: 'Your account does not have an active subscription.',
  terms_not_accepted: 'You need to accept the terms of service. Please log in via the official launcher first.',
  unknown: 'An unexpected error occurred. Please try again.',
};

export function LoginForm({
  account,
  onCancel,
  onLoginSuccess,
}: LoginFormProps) {
  const {
    loginState,
    loginError,
    errorType,
    otpListenerActive,
    login,
    getStoredPassword,
    startOtpListener,
    stopOtpListener,
    clearError,
  } = useAuthStore();

  const [password, setPassword] = useState('');
  const [otp, setOtp] = useState('');
  const [savePassword, setSavePassword] = useState(false);
  const [hasStoredPassword, setHasStoredPassword] = useState(false);

  // Load stored password on mount/account change
  useEffect(() => {
    if (account) {
      clearError();
      setOtp('');

      // Try to get stored password
      getStoredPassword(account.id).then((storedPwd) => {
        if (storedPwd) {
          setPassword(storedPwd);
          setSavePassword(true);
          setHasStoredPassword(true);
        } else {
          setPassword('');
          setSavePassword(false);
          setHasStoredPassword(false);
        }
      });
    }
  }, [account, getStoredPassword, clearError]);

  // Listen for OTP received event
  useEffect(() => {
    let unlisten: UnlistenFn | null = null;

    if (account?.use_otp) {
      listen<string>('otp_received', (event) => {
        setOtp(event.payload);
        stopOtpListener();
      }).then((fn) => {
        unlisten = fn;
      });
    }

    return () => {
      if (unlisten) {
        unlisten();
      }
      if (otpListenerActive) {
        stopOtpListener();
      }
    };
  }, [account?.use_otp, stopOtpListener, otpListenerActive]);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();

    if (!account) return;

    const result = await login(
      account.id,
      password,
      account.use_otp ? otp : undefined,
      savePassword
    );

    if (result.success) {
      onLoginSuccess?.();
    }
  };

  const handleStartOtpListener = async () => {
    try {
      await startOtpListener();
    } catch (err) {
      console.error('Failed to start OTP listener:', err);
    }
  };

  const handleCancel = () => {
    if (otpListenerActive) {
      stopOtpListener();
    }
    clearError();
    onCancel?.();
  };

  const isLoading = loginState === 'LoggingIn';
  const displayError = errorType ? ERROR_MESSAGES[errorType as LoginErrorType] : loginError;

  if (!account) return null;

  return (
    <form className="login-form" onSubmit={handleSubmit}>
      <div className="login-form-header">
        <h3 className="login-form-title">Login as {account.username}</h3>
        {onCancel && (
          <button
            type="button"
            className="secondary login-form-cancel"
            onClick={handleCancel}
            disabled={isLoading}
          >
            Cancel
          </button>
        )}
      </div>

      {displayError && (
        <div className="login-form-error">{displayError}</div>
      )}

      <div className={`login-form-fields ${account.use_otp ? 'with-otp' : ''}`}>
        <div className="login-form-field">
          <label htmlFor="login-password">Password</label>
          <input
            id="login-password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            placeholder={hasStoredPassword ? 'Using saved password' : 'Enter password'}
            disabled={isLoading}
            autoFocus
          />
        </div>

        {account.use_otp && (
          <div className="login-form-field">
            <label htmlFor="login-otp">One-Time Password</label>
            <div className="login-form-otp-row">
              <input
                id="login-otp"
                type="text"
                value={otp}
                onChange={(e) => setOtp(e.target.value)}
                placeholder={otpListenerActive ? 'Waiting...' : '6-digit code'}
                disabled={isLoading}
                maxLength={6}
                pattern="[0-9]*"
                inputMode="numeric"
              />
              <button
                type="button"
                className={`secondary otp-listener-btn ${otpListenerActive ? 'active' : ''}`}
                onClick={handleStartOtpListener}
                disabled={isLoading || otpListenerActive}
                title="Start listening for OTP from mobile app"
              >
                {otpListenerActive ? (
                  <span className="otp-waiting">
                    <span className="spinner" />
                  </span>
                ) : (
                  'Auto'
                )}
              </button>
            </div>
          </div>
        )}
      </div>

      {otpListenerActive && (
        <span className="login-form-hint">
          Open your authenticator app and tap &quot;Send OTP to Launcher&quot;
        </span>
      )}

      <div className="login-form-footer">
        <label className="login-form-checkbox">
          <input
            type="checkbox"
            checked={savePassword}
            onChange={(e) => setSavePassword(e.target.checked)}
            disabled={isLoading}
          />
          <span>Save password</span>
        </label>

        <button
          type="submit"
          className="primary login-form-submit"
          disabled={isLoading || !password}
        >
          {isLoading ? 'Logging in...' : 'Login'}
        </button>
      </div>
    </form>
  );
}
