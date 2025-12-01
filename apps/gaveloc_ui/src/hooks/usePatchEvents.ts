import { useEffect } from 'react';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { usePatchStore } from '../stores/patchStore';
import { useGameStore } from '../stores/gameStore';
import type { PatchProgressEvent, PatchCompletedEvent, PatchErrorEvent } from '../types';

export function usePatchEvents() {
  const { updateProgress, markCompleted, setError, reset } = usePatchStore();
  const { loadVersions } = useGameStore();

  useEffect(() => {
    const unsubscribers: UnlistenFn[] = [];

    // Subscribe to patch_progress
    listen<PatchProgressEvent>('patch_progress', (event) => {
      updateProgress(event.payload);
    }).then((fn) => unsubscribers.push(fn));

    // Subscribe to patch_completed
    listen<PatchCompletedEvent>('patch_completed', (event) => {
      markCompleted(event.payload);
    }).then((fn) => unsubscribers.push(fn));

    // Subscribe to patch_all_completed
    listen('patch_all_completed', () => {
      reset();
      // Reload game versions after patching completes
      loadVersions();
    }).then((fn) => unsubscribers.push(fn));

    // Subscribe to patch_error
    listen<PatchErrorEvent>('patch_error', (event) => {
      setError(event.payload.message);
    }).then((fn) => unsubscribers.push(fn));

    // Subscribe to patch_cancelled
    listen('patch_cancelled', () => {
      reset();
    }).then((fn) => unsubscribers.push(fn));

    return () => {
      unsubscribers.forEach((fn) => fn());
    };
  }, [updateProgress, markCompleted, setError, reset, loadVersions]);
}
