import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { LoginResult, CachedSession, SessionStatus, LoginState } from '../types';

interface AuthState {
  loginState: LoginState;
  loginError: string | null;
  errorType: string | null;
  sessionStatus: SessionStatus | null;
  otpListenerActive: boolean;

  // Actions
  login: (
    accountId: string,
    password: string,
    otp?: string,
    savePassword?: boolean
  ) => Promise<LoginResult>;
  loginWithCachedSession: (accountId: string) => Promise<CachedSession>;
  logout: (accountId: string, clearPassword?: boolean) => Promise<void>;
  getStoredPassword: (accountId: string) => Promise<string | null>;
  checkSessionStatus: (accountId: string) => Promise<SessionStatus>;
  startOtpListener: () => Promise<void>;
  stopOtpListener: () => Promise<void>;
  isOtpListenerRunning: () => Promise<boolean>;
  clearError: () => void;
  setLoginState: (state: LoginState) => void;
}

export const useAuthStore = create<AuthState>((set) => ({
  loginState: 'LoggedOut',
  loginError: null,
  errorType: null,
  sessionStatus: null,
  otpListenerActive: false,

  login: async (
    accountId: string,
    password: string,
    otp?: string,
    savePassword = false
  ): Promise<LoginResult> => {
    set({ loginState: 'LoggingIn', loginError: null, errorType: null });

    try {
      const result = await invoke<LoginResult>('login', {
        accountId,
        password,
        otp: otp || null,
        savePassword,
      });

      if (result.success) {
        set({ loginState: 'LoggedIn', loginError: null, errorType: null });
      } else {
        set({
          loginState: 'Error',
          loginError: result.error || 'Login failed',
          errorType: result.error_type || null,
        });
      }

      return result;
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : String(err);
      set({
        loginState: 'Error',
        loginError: errorMessage,
        errorType: 'unknown',
      });
      return {
        success: false,
        error: errorMessage,
        error_type: 'unknown',
      };
    }
  },

  loginWithCachedSession: async (accountId: string): Promise<CachedSession> => {
    try {
      const result = await invoke<CachedSession>('login_with_cached_session', {
        accountId,
      });

      if (result.valid) {
        set({ loginState: 'LoggedIn', loginError: null, errorType: null });
      }

      return result;
    } catch (err) {
      return {
        valid: false,
      };
    }
  },

  logout: async (accountId: string, clearPassword = false): Promise<void> => {
    try {
      await invoke('logout', { accountId, clearPassword });
      set({
        loginState: 'LoggedOut',
        loginError: null,
        errorType: null,
        sessionStatus: null,
      });
    } catch (err) {
      console.error('Logout error:', err);
    }
  },

  getStoredPassword: async (accountId: string): Promise<string | null> => {
    try {
      return await invoke<string | null>('get_stored_password', { accountId });
    } catch {
      return null;
    }
  },

  checkSessionStatus: async (accountId: string): Promise<SessionStatus> => {
    try {
      const status = await invoke<SessionStatus>('get_session_status', {
        accountId,
      });
      set({ sessionStatus: status });

      if (status.is_valid) {
        set({ loginState: 'LoggedIn' });
      }

      return status;
    } catch {
      return {
        has_session: false,
        is_valid: false,
      };
    }
  },

  startOtpListener: async (): Promise<void> => {
    try {
      await invoke('start_otp_listener');
      set({ otpListenerActive: true, loginState: 'AwaitingOtp' });
    } catch (err) {
      console.error('Failed to start OTP listener:', err);
      throw err;
    }
  },

  stopOtpListener: async (): Promise<void> => {
    try {
      await invoke('stop_otp_listener');
      set({ otpListenerActive: false });
    } catch (err) {
      console.error('Failed to stop OTP listener:', err);
    }
  },

  isOtpListenerRunning: async (): Promise<boolean> => {
    try {
      const running = await invoke<boolean>('is_otp_listener_running');
      set({ otpListenerActive: running });
      return running;
    } catch {
      return false;
    }
  },

  clearError: () => {
    set({ loginError: null, errorType: null });
  },

  setLoginState: (state: LoginState) => {
    set({ loginState: state });
  },
}));
