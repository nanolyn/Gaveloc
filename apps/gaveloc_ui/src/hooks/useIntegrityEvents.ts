import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { useIntegrityStore } from '../stores/integrityStore';
import type { IntegrityProgress, IntegrityResult } from '../types';

export function useIntegrityEvents() {
  const { updateProgress, setResult, setError, reset } = useIntegrityStore();

  useEffect(() => {
    const unsubscribers: UnlistenFn[] = [];

    // Subscribe to integrity_progress
    listen<IntegrityProgress>('integrity_progress', (event) => {
      updateProgress(event.payload);
    }).then((fn) => unsubscribers.push(fn));

    // Subscribe to integrity_complete
    listen<IntegrityResult>('integrity_complete', (event) => {
      setResult(event.payload);
    }).then((fn) => unsubscribers.push(fn));

    // Subscribe to integrity_error
    listen<string>('integrity_error', (event) => {
      setError(event.payload);
    }).then((fn) => unsubscribers.push(fn));

    // Subscribe to integrity_cancelled
    listen('integrity_cancelled', () => {
      reset();
    }).then((fn) => unsubscribers.push(fn));

    return () => {
      unsubscribers.forEach((fn) => fn());
    };
  }, [updateProgress, setResult, setError, reset]);
}
