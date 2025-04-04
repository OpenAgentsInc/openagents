// apps/coder/src/helpers/ipc/db-status/db-status-context.ts
import { contextBridge, ipcRenderer } from 'electron';

export interface DbStatusContext {
  getDbStatus: () => Promise<{ ready: boolean; error: string | null }>;
}

const dbStatusContext: DbStatusContext = {
  getDbStatus: () => ipcRenderer.invoke('get-db-status'),
};

export function exposeDbStatusContext() {
  console.log('[Preload] Exposing DbStatusContext');
  contextBridge.exposeInMainWorld('dbStatusContext', dbStatusContext);
}
