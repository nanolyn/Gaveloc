import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Account, CreateAccountRequest } from '../types';

interface AccountState {
  // State
  accounts: Account[];
  currentAccount: Account | null;
  loading: boolean;
  error: string | null;

  // Actions
  loadAccounts: () => Promise<void>;
  addAccount: (request: CreateAccountRequest) => Promise<Account>;
  updateAccount: (request: CreateAccountRequest) => Promise<Account>;
  removeAccount: (accountId: string) => Promise<void>;
  setCurrentAccount: (accountId: string) => Promise<void>;
  hasStoredPassword: (accountId: string) => Promise<boolean>;
  storePassword: (accountId: string, password: string) => Promise<void>;
  deletePassword: (accountId: string) => Promise<void>;
  setError: (error: string | null) => void;
}

export const useAccountStore = create<AccountState>((set, get) => ({
  accounts: [],
  currentAccount: null,
  loading: false,
  error: null,

  loadAccounts: async () => {
    set({ loading: true, error: null });
    try {
      const [accounts, currentAccount] = await Promise.all([
        invoke<Account[]>('list_accounts'),
        invoke<Account | null>('get_default_account'),
      ]);
      set({ accounts, currentAccount, loading: false });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error, loading: false });
    }
  },

  addAccount: async (request: CreateAccountRequest) => {
    set({ error: null });
    try {
      const account = await invoke<Account>('add_account', { request });
      const { accounts, currentAccount } = get();
      set({
        accounts: [...accounts, account],
        // Set as current if it's the first account
        currentAccount: currentAccount ?? account,
      });
      // If this is the first account, set it as default
      if (!currentAccount) {
        await invoke('set_default_account', { accountId: account.id });
      }
      return account;
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error });
      throw err;
    }
  },

  updateAccount: async (request: CreateAccountRequest) => {
    set({ error: null });
    try {
      const account = await invoke<Account>('update_account', { request });
      const { accounts, currentAccount } = get();
      set({
        accounts: accounts.map((a) => (a.id === account.id ? account : a)),
        currentAccount:
          currentAccount?.id === account.id ? account : currentAccount,
      });
      return account;
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error });
      throw err;
    }
  },

  removeAccount: async (accountId: string) => {
    set({ error: null });
    try {
      await invoke('remove_account', { accountId });
      const { accounts, currentAccount } = get();
      const newAccounts = accounts.filter((a) => a.id !== accountId);
      set({
        accounts: newAccounts,
        currentAccount:
          currentAccount?.id === accountId
            ? newAccounts[0] ?? null
            : currentAccount,
      });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error });
      throw err;
    }
  },

  setCurrentAccount: async (accountId: string) => {
    set({ error: null });
    try {
      await invoke('set_default_account', { accountId });
      const { accounts } = get();
      const account = accounts.find((a) => a.id === accountId) ?? null;
      set({ currentAccount: account });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error });
      throw err;
    }
  },

  hasStoredPassword: async (accountId: string) => {
    try {
      return await invoke<boolean>('has_stored_password', { accountId });
    } catch {
      return false;
    }
  },

  storePassword: async (accountId: string, password: string) => {
    set({ error: null });
    try {
      await invoke('store_password', { accountId, password });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error });
      throw err;
    }
  },

  deletePassword: async (accountId: string) => {
    set({ error: null });
    try {
      await invoke('delete_password', { accountId });
    } catch (err) {
      const error = err instanceof Error ? err.message : String(err);
      set({ error });
      throw err;
    }
  },

  setError: (error: string | null) => {
    set({ error });
  },
}));
