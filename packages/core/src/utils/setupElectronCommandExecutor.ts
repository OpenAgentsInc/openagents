/**
 * A utility file to set up command execution in Electron environments
 * 
 * To use this, import and call setupElectronCommandExecutor() in your Electron app's main process
 */

import { executeCommand } from './commandExecutor';
import type { CommandExecutionOptions, CommandExecutionResult } from './commandExecutor';

/**
 * Call this function in your Electron application's main process to set up
 * command execution for renderer processes
 */
export function setupElectronCommandExecutor() {
  // Check if we're in an Electron environment
  const isElectron = typeof process !== 'undefined' && 
                     process.versions && 
                     process.versions.electron;
  
  if (!isElectron) {
    console.warn('setupElectronCommandExecutor called outside of an Electron environment');
    return;
  }
  
  console.log('🔌 ELECTRON: Setting up command execution for Electron');
  
  // Set up IPC handlers to allow command execution from renderer processes
  try {
    const { ipcMain } = require('electron');
    
    // Set up IPC handler for standard 'execute-command' channel
    if (!ipcMain.listeners('execute-command').length) {
      ipcMain.handle('execute-command', async (_: any, command: string, options: CommandExecutionOptions = {}) => {
        console.log(`🔌 ELECTRON: IPC execute-command received:`, command);
        
        try {
          return await executeCommand(command, options);
        } catch (error) {
          console.error('🔌 ELECTRON: Error executing command via IPC:', error);
          return { 
            error: error instanceof Error ? error.message : String(error) 
          };
        }
      });
    }
    
    // Set up IPC handler for 'command:execute' channel used by existing codebase
    const EXECUTE_COMMAND = 'command:execute';
    if (!ipcMain.listeners(EXECUTE_COMMAND).length) {
      ipcMain.handle(EXECUTE_COMMAND, async (_: any, args: any) => {
        const { command, options } = args || {};
        console.log(`🔌 ELECTRON: IPC ${EXECUTE_COMMAND} received:`, command);
        
        try {
          return await executeCommand(command, options);
        } catch (error) {
          console.error('🔌 ELECTRON: Error executing command via IPC:', error);
          return { 
            error: error instanceof Error ? error.message : String(error),
            command
          };
        }
      });
    }
    
    console.log('✅ ELECTRON: Command execution handlers registered');
  } catch (error) {
    console.error('❌ ELECTRON: Failed to set up IPC handlers:', error);
  }
}

/**
 * Call this function in your Electron application's renderer process to check
 * if command execution is available
 */
export async function checkElectronCommandExecutionAvailable(): Promise<boolean> {
  if (typeof window === 'undefined' || !window.electron?.ipcRenderer) {
    console.log('🔌 ELECTRON: Not in an Electron renderer process or IPC not available');
    return false;
  }
  
  try {
    // Send a test command to see if execution works
    const result = await window.electron.ipcRenderer.invoke('execute-command', 'echo "Test"');
    
    const success = !('error' in result);
    console.log('🔌 ELECTRON: Command execution available:', success);
    return success;
  } catch (error) {
    console.error('❌ ELECTRON: Error checking command execution:', error);
    return false;
  }
}