import { contextBridge, ipcRenderer } from 'electron';
import { EXECUTE_COMMAND } from './command-channels';

/**
 * Expose command execution functionality to the renderer process
 */
export function exposeCommandContext() {
  contextBridge.exposeInMainWorld('commandExecution', {
    executeCommand: (command: string, options?: any) => {
      return ipcRenderer.invoke(EXECUTE_COMMAND, { command, options });
    }
  });
}