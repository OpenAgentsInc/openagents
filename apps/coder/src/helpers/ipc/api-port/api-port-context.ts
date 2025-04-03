// This file sets up the context bridge for the API port
import { contextBridge, ipcRenderer } from 'electron';

// Channel names for API port communication
export const API_PORT_CHANNEL = 'api-port';
export const GET_API_PORT = 'get-api-port';

/**
 * Expose the API port to the renderer process
 */
export function exposeApiPortContext() {
  contextBridge.exposeInMainWorld('API_PORT', {
    // Get the current API port
    getPort: async () => {
      return await ipcRenderer.invoke(GET_API_PORT);
    }
  });
}