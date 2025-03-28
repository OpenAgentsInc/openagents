import { ipcMain } from 'electron';
import { executeCommand } from '@openagents/core';
import { EXECUTE_COMMAND } from './command-channels';

/**
 * Register command execution listeners on the main process
 */
export function registerCommandListeners() {
  // Handle command execution
  ipcMain.handle(EXECUTE_COMMAND, async (event, args) => {
    const { command, options } = args;
    
    try {
      const result = await executeCommand(command, options);
      return result;
    } catch (error) {
      return {
        error: error instanceof Error ? error.message : String(error),
        command
      };
    }
  });
}