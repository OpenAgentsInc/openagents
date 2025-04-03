/**
 * Log Manager Utility
 * 
 * A centralized logging system for the application that:
 * 1. Handles logging in both development and production
 * 2. Stores logs for later retrieval in the debug console
 * 3. Allows different log levels (debug, info, warn, error)
 * 4. Provides ability to send logs to the console AND the in-app log storage
 */

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

export interface LogEntry {
  timestamp: number;
  level: LogLevel;
  message: string;
  module: string;
  details?: any;
}

// Maximum number of log entries to keep in memory
const MAX_LOG_ENTRIES = 1000;

// In-memory storage for logs
const logEntries: LogEntry[] = [];

// Flag to determine if console output is enabled
let consoleOutputEnabled = true;

/**
 * Add a log entry to the log storage
 */
function addLogEntry(entry: LogEntry): void {
  // Ensure we don't exceed maximum capacity
  if (logEntries.length >= MAX_LOG_ENTRIES) {
    logEntries.shift(); // Remove oldest log
  }
  
  // Add new log
  logEntries.push(entry);
  
  // Also log to console if enabled
  if (consoleOutputEnabled) {
    const logPrefix = `[${new Date(entry.timestamp).toISOString()}] [${entry.level.toUpperCase()}] [${entry.module}]`;
    
    switch (entry.level) {
      case 'debug':
        console.debug(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
        break;
      case 'info':
        console.info(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
        break;
      case 'warn':
        console.warn(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
        break;
      case 'error':
        console.error(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
        break;
    }
  }
}

/**
 * Enable or disable console output
 */
export function setConsoleOutput(enabled: boolean): void {
  consoleOutputEnabled = enabled;
}

/**
 * Get current console output state
 */
export function getConsoleOutputState(): boolean {
  return consoleOutputEnabled;
}

/**
 * Get all log entries
 */
export function getAllLogs(): LogEntry[] {
  return [...logEntries];
}

/**
 * Clear all logs
 */
export function clearLogs(): void {
  logEntries.length = 0;
}

/**
 * Filter logs by level and/or module
 */
export function filterLogs(options: { level?: LogLevel, module?: string }): LogEntry[] {
  return logEntries.filter(entry => {
    if (options.level && entry.level !== options.level) {
      return false;
    }
    if (options.module && entry.module !== options.module) {
      return false;
    }
    return true;
  });
}

/**
 * Create a logger instance for a specific module
 */
export function createLogger(module: string) {
  return {
    debug: (message: string, details?: any) => {
      addLogEntry({
        timestamp: Date.now(),
        level: 'debug',
        message,
        module,
        details
      });
    },
    
    info: (message: string, details?: any) => {
      addLogEntry({
        timestamp: Date.now(),
        level: 'info',
        message,
        module,
        details
      });
    },
    
    warn: (message: string, details?: any) => {
      addLogEntry({
        timestamp: Date.now(),
        level: 'warn',
        message,
        module,
        details
      });
    },
    
    error: (message: string, details?: any) => {
      addLogEntry({
        timestamp: Date.now(),
        level: 'error',
        message,
        module,
        details
      });
    }
  };
}

// Export a default logger for quick usage
export const logger = createLogger('app');

// Override console methods to capture all logs
export function installConsoleInterceptor() {
  if (typeof window !== 'undefined') {
    // Store original console methods
    const originalConsole = {
      log: console.log,
      debug: console.debug,
      info: console.info,
      warn: console.warn,
      error: console.error
    };

    // Override console.log
    console.log = function(...args) {
      // Call original method
      originalConsole.log.apply(console, args);
      
      // Add to our log system
      addLogEntry({
        timestamp: Date.now(),
        level: 'info',
        message: args.map(arg => 
          typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' '),
        module: 'console'
      });
    };

    // Override console.debug
    console.debug = function(...args) {
      // Call original method
      originalConsole.debug.apply(console, args);
      
      // Add to our log system
      addLogEntry({
        timestamp: Date.now(),
        level: 'debug',
        message: args.map(arg => 
          typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' '),
        module: 'console'
      });
    };

    // Override console.info
    console.info = function(...args) {
      // Call original method
      originalConsole.info.apply(console, args);
      
      // Add to our log system
      addLogEntry({
        timestamp: Date.now(),
        level: 'info',
        message: args.map(arg => 
          typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' '),
        module: 'console'
      });
    };

    // Override console.warn
    console.warn = function(...args) {
      // Call original method
      originalConsole.warn.apply(console, args);
      
      // Add to our log system
      addLogEntry({
        timestamp: Date.now(),
        level: 'warn',
        message: args.map(arg => 
          typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' '),
        module: 'console'
      });
    };

    // Override console.error
    console.error = function(...args) {
      // Call original method
      originalConsole.error.apply(console, args);
      
      // Add to our log system
      addLogEntry({
        timestamp: Date.now(),
        level: 'error',
        message: args.map(arg => 
          typeof arg === 'object' ? JSON.stringify(arg) : String(arg)
        ).join(' '),
        module: 'console'
      });
    };
  }
}