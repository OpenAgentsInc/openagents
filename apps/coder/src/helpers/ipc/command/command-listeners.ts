import { ipcMain } from 'electron';
import { EXECUTE_COMMAND } from './command-channels';

// Get the executeCommand function from either global or @openagents/core
// This allows us to avoid duplicate handlers

/**
 * Register command execution listeners on the main process
 */
// Define a type for an executeCommand function to handle both sources
type ExecuteCommandFunction = (command: string, options?: any) => Promise<any>;

export function registerCommandListeners() {
  // Check if we already have handlers
  if (ipcMain.listeners(EXECUTE_COMMAND).length > 0) {
    console.log('Command execution listeners already registered, skipping');
    return;
  }
  
  // Get the executeCommand function, prefer the one from core if available
  const getExecuteCommand = (): ExecuteCommandFunction => {
    if (typeof global.executeCommandFromCore === 'function') {
      console.log('Using executeCommandFromCore from @openagents/core');
      return global.executeCommandFromCore;
    }
    
    console.log('Using local command execution implementation');
    // Fallback to a simple execute function that just returns an error
    return async (command: string) => {
      return {
        error: 'Command execution not properly initialized',
        command
      };
    };
  };
  
  const executeCommand = getExecuteCommand();
  
  // Handle command execution
  ipcMain.handle(EXECUTE_COMMAND, async (event, args) => {
    const { command, options } = args;
    
    try {
      console.log(`Executing command via registered listener: ${command}`);
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