// Conditionally import child_process only in Node.js environment
// This prevents errors when this code is loaded in a browser context
let childProcess: typeof import('child_process') | null = null;
try {
  // Check if we're in a Node.js environment
  if (typeof process !== 'undefined' && process.versions && process.versions.node) {
    // Dynamic import to avoid bundling Node.js modules in browser builds
    childProcess = require('child_process');
  }
} catch (e) {
  // Silently fail - we'll handle the lack of child_process later
  console.debug('child_process not available - command execution will be disabled');
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
    // Check if we are in a browser environment
    if (typeof window !== 'undefined' && !childProcess) {
      return { 
        error: 'Command execution is only available in the Electron app'
      };
    }
    
    // Execute command
    return await executeCommand(command, options);
  } catch (error) {
    return {
      error: error instanceof Error ? error.message : String(error)
    };
  }
}