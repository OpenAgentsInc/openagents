// API port listeners for main process
import { ipcMain } from 'electron';
import { GET_API_PORT } from './api-port-context';

// Store the API port globally
let apiPort: number = 3001; // Default port

/**
 * Set the API port for the application
 */
export function setApiPort(port: number) {
  apiPort = port;
  console.log(`[API Port] Set API port to ${port}`);
}

/**
 * Register API port listeners in the main process
 */
export function registerApiPortListeners() {
  // Handle requests for the API port
  ipcMain.handle(GET_API_PORT, async () => {
    return apiPort;
  });

  console.log('[API Port] Registered API port listeners');
}