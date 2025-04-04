// apps/coder/src/helpers/ipc/db-status/db-status-context.ts
import { contextBridge, ipcRenderer } from 'electron';

export interface DbStatusContext {
  getDbStatus: () => Promise<{ ready: boolean; error: string | null }>;
}

// Create a mock implementation for cases where IPC isn't available yet
function createDbStatusMock(): DbStatusContext {
  console.warn('[Preload] Creating DB status mock - IPC may not be available');
  return {
    getDbStatus: async () => {
      // Return a default status
      console.log('[DB Status Mock] Returning mock status');
      return { ready: false, error: null };
    }
  };
}

// Create the real implementation using IPC
function createDbStatusImplementation(): DbStatusContext {
  console.log('[Preload] Creating real DB status implementation via IPC');
  return {
    getDbStatus: async () => {
      try {
        console.log('[DB Status] Invoking IPC get-db-status');
        return await ipcRenderer.invoke('get-db-status');
      } catch (error) {
        console.error('[DB Status] Error getting DB status via IPC:', error);
        return { ready: false, error: error.message || 'Failed to communicate with main process' };
      }
    }
  };
}

export function exposeDbStatusContext() {
  console.log('[Preload] Exposing DbStatusContext');
  try {
    // Create real implementation and expose it
    const dbStatusContext = createDbStatusImplementation();
    contextBridge.exposeInMainWorld('dbStatusContext', dbStatusContext);
    console.log('[Preload] DbStatusContext exposed successfully');
  } catch (error) {
    // Fallback to mock implementation if something goes wrong
    console.error('[Preload] Error exposing DbStatusContext:', error);
    const mockContext = createDbStatusMock();
    try {
      contextBridge.exposeInMainWorld('dbStatusContext', mockContext);
      console.log('[Preload] Mock DbStatusContext exposed as fallback');
    } catch (fallbackError) {
      console.error('[Preload] Fatal: Could not expose DB status context:', fallbackError);
    }
  }
}
