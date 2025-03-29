/**
 * Global declarations for OpenAgents core package
 */

// Define Command Execution Context interface
interface CommandExecutionContext {
  executeCommand(command: string, options?: any): Promise<any>;
}

// Define Electron IPC interface 
interface ElectronIPC {
  ipcRenderer?: {
    invoke(channel: string, ...args: any[]): Promise<any>;
  };
}

// Extend the Window interface
interface Window {
  electron?: ElectronIPC;
  commandExecution?: CommandExecutionContext;
}