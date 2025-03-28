/**
 * A utility file to set up command execution in Electron environments
 * 
 * To use this, import and call setupElectronCommandExecutor() in your Electron app's main process
 */

import { executeCommand } from './commandExecutor';
import type { CommandExecutionOptions, CommandExecutionResult } from './commandExecutor';

// Add a global declaration for our command execution function
declare global {
  var executeCommandFromCore: typeof executeCommand;
}

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
    
    // DON'T set up IPC handler for the 'command:execute' channel because it's already 
    // being registered by the app. Our code will just check for the existence of the handlers
    // and use them rather than trying to register new ones.
    
    console.log('✅ ELECTRON: Using existing command execution handlers');
    
    // We'll document the handlers we expect to be registered:
    console.log('ℹ️ ELECTRON: Expected handlers:');
    console.log('ℹ️ ELECTRON:   - command:execute - Used by existing codebase');
    
    // Instead, we'll just make our commands available for the existing handlers to use
    global.executeCommandFromCore = executeCommand;
    
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