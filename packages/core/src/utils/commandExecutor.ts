// Conditionally import child_process only in Node.js environment
// This prevents errors when this code is loaded in a browser context
/**
 * Helper function to detect if we're running in Electron
 */
function isElectron() {
  // Renderer process
  if (typeof window !== 'undefined' && typeof window.process === 'object' && 
      (window.process as any)?.type === 'renderer') {
    console.log('🔍 COMMAND EXECUTOR: Electron renderer process detected');
    return true;
  }

  // Main process
  if (typeof process !== 'undefined' && typeof process.versions === 'object' && 
      process.versions.electron) {
    console.log('🔍 COMMAND EXECUTOR: Electron main process detected');
    return true;
  }

  // Detect the user agent when the `nodeIntegration` option is set to false
  if (typeof navigator === 'object' && typeof (navigator as any).userAgent === 'string' && 
      (navigator as any).userAgent.indexOf('Electron') >= 0) {
    console.log('🔍 COMMAND EXECUTOR: Electron user agent detected');
    return true;
  }

  console.log('🔍 COMMAND EXECUTOR: Not running in Electron');
  return false;
}

/**
 * Helper function to execute a command via Electron IPC
 */
async function executeViaElectronIPC(command: string, options: CommandExecutionOptions = {}) {
  if (typeof window === 'undefined' || !isElectron()) {
    throw new Error('Not in an Electron renderer process');
  }

  try {
    // Try to get the electron IPC renderer
    const electron = (window as any).require('electron');
    if (!electron || !electron.ipcRenderer) {
      throw new Error('Electron IPC renderer not available');
    }

    console.log('🔌 COMMAND EXECUTOR: Executing via Electron IPC:', command);
    return await electron.ipcRenderer.invoke('execute-command', command, options);
  } catch (error) {
    console.error('❌ COMMAND EXECUTOR: Error executing via Electron IPC:', error);
    throw error;
  }
}

let childProcess: typeof import('child_process') | null = null;
try {
  // Check if we're in Electron or Node.js environment
  const isNodeEnv = typeof process !== 'undefined' && process.versions && process.versions.node;
  const isElectronEnv = isElectron();
  
  if (isNodeEnv || isElectronEnv) {
    console.log('🔍 COMMAND EXECUTOR: Node.js/Electron environment detected');
    // Dynamic import to avoid bundling Node.js modules in browser builds
    childProcess = require('child_process');
    console.log('✅ COMMAND EXECUTOR: child_process loaded successfully');
  } else {
    console.log('🔍 COMMAND EXECUTOR: Browser environment detected - command execution disabled');
  }
} catch (e) {
  // Log the error for debugging
  console.error('❌ COMMAND EXECUTOR: Error importing child_process:', e);
  console.debug('child_process not available - command execution will be disabled');
}

// Declare the global server command executor type
declare global {
  interface Window {
    electron?: {
      ipcRenderer?: {
        invoke(channel: string, ...args: any[]): Promise<any>;
      };
    };
    commandExecution?: {
      executeCommand(command: string, options?: any): Promise<any>;
    };
  }
}

export interface CommandExecutionOptions {
  /**
   * The working directory for the command
   */
  cwd?: string;
  
  /**
   * Maximum time (in milliseconds) to allow the command to run
   */
  timeout?: number;
  
  /**
   * Additional environment variables to pass to the command
   */
  env?: Record<string, string>;
  
  /**
   * Whether to ask for user confirmation before executing the command
   */
  requireConfirmation?: boolean;
}

export interface CommandExecutionResult {
  /**
   * The stdout output of the command
   */
  stdout: string;
  
  /**
   * The stderr output of the command
   */
  stderr: string;
  
  /**
   * The exit code of the command (0 for success)
   */
  exitCode: number;
  
  /**
   * The command that was executed
   */
  command: string;
}

/**
 * List of potentially dangerous command patterns that should be blocked
 */
const DANGEROUS_COMMAND_PATTERNS = [
  /\brm\s+-rf\s+[\/~]/i,        // Delete files recursively from root or home
  /\bchmod\s+777\b/i,           // Insecure permissions
  /\bcurl\s+.*\|\s*sh\b/i,      // Pipe from internet to shell
  /\bwget\s+.*\|\s*sh\b/i,      // Pipe from internet to shell
  /\bmkfs\b/i,                  // Format filesystem
  /\bdd\s+.*of=\/dev\//i,       // Direct disk writes
  /\bsudo\b/i,                  // Privileged execution
  /\bsu\b/i,                    // Switch user
];

/**
 * Checks if a command might be dangerous
 */
export function isDangerousCommand(command: string): boolean {
  return DANGEROUS_COMMAND_PATTERNS.some(pattern => pattern.test(command));
}

/**
 * Executes a shell command and returns its result
 */
export async function executeCommand(
  command: string,
  options: CommandExecutionOptions = {}
): Promise<CommandExecutionResult> {
  return new Promise((resolve, reject) => {
    // Check if command is potentially dangerous
    if (isDangerousCommand(command)) {
      reject(new Error(`Potentially dangerous command detected: ${command}`));
      return;
    }

    // Check if childProcess is available first
    if (!childProcess) {
      reject(new Error('Command execution is not available in this environment'));
      return;
    }
    
    // Set default options
    const timeoutMs = options.timeout || 30000; // Default 30 second timeout
    const cwd = options.cwd || (typeof process !== 'undefined' ? process.cwd() : '.');
    const env = { ...(typeof process !== 'undefined' ? process.env : {}), ...(options.env || {})}
    
    // Spawn process with shell
    const proc = childProcess.spawn('bash', ['-c', command], {
      cwd,
      env,
      shell: true
    });
    
    let stdout = '';
    let stderr = '';
    let timeoutId: NodeJS.Timeout | null = null;
    
    // Collect stdout
    proc.stdout.on('data', (data) => {
      stdout += data.toString();
    });
    
    // Collect stderr
    proc.stderr.on('data', (data) => {
      stderr += data.toString();
    });
    
    // Handle error
    proc.on('error', (error) => {
      if (timeoutId) clearTimeout(timeoutId);
      reject(error);
    });
    
    // Handle process completion
    proc.on('close', (code) => {
      if (timeoutId) clearTimeout(timeoutId);
      resolve({
        stdout,
        stderr,
        exitCode: code ?? 0,
        command
      });
    });
    
    // Set timeout
    if (timeoutMs > 0) {
      timeoutId = setTimeout(() => {
        proc.kill();
        reject(new Error(`Command timed out after ${timeoutMs}ms: ${command}`));
      }, timeoutMs);
    }
  });
}

/**
 * Command execution handler that checks for availability of Node.js APIs
 * This function works in Electron, but gracefully fails in web environments
 */
export async function safeExecuteCommand(
  command: string,
  options: CommandExecutionOptions = {}
): Promise<CommandExecutionResult | { error: string }> {
  try {
    // First, check if commandExecution is available in the window object
    if (typeof window !== 'undefined' && window.commandExecution) {
      try {
        console.log('🔌 COMMAND EXECUTOR: Executing via commandExecution API:', command);
        return await window.commandExecution.executeCommand(command, options);
      } catch (cmdError) {
        console.error('❌ COMMAND EXECUTOR: commandExecution API failed:', cmdError);
        return {
          error: `Command execution failed: ${cmdError instanceof Error ? cmdError.message : String(cmdError)}`
        };
      }
    }
    
    // Next, try window.electron if available
    if (isElectron() && typeof window !== 'undefined' && window.electron?.ipcRenderer) {
      try {
        console.log('🔌 COMMAND EXECUTOR: Executing via Electron IPC:', command);
        return await window.electron.ipcRenderer.invoke('execute-command', command, options);
      } catch (ipcError) {
        console.error('❌ COMMAND EXECUTOR: IPC execution failed:', ipcError);
        return {
          error: `IPC execution failed: ${ipcError instanceof Error ? ipcError.message : String(ipcError)}`
        };
      }
    }
    
    // Check if childProcess is available directly (Node.js/Electron main process)
    if (childProcess) {
      console.log('🔨 COMMAND EXECUTOR: Executing command directly with child_process');
      return await executeCommand(command, options);
    }
    
    // If we get here, no command execution methods are available
    console.log('📌 COMMAND EXECUTOR: Command execution only available in Electron');
    return { 
      error: 'Command execution is only available in the Electron app'
    };
  } catch (error) {
    console.error('❌ COMMAND EXECUTOR: Error executing command:', error);
    return {
      error: error instanceof Error ? error.message : String(error)
    };
  }
}