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

// Flag to prevent recursive console logging
let isLoggingToConsole = false;

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
  
  // Only log to console if enabled and we're not already in a logging operation
  if (consoleOutputEnabled && !isLoggingToConsole) {
    try {
      isLoggingToConsole = true; // Set flag to prevent recursion
      
      const logPrefix = `[${new Date(entry.timestamp).toISOString()}] [${entry.level.toUpperCase()}] [${entry.module}]`;
      
      // Use the original console methods directly to avoid recursion
      const originalConsole = (window as any).__originalConsole__;
      if (originalConsole) {
        switch (entry.level) {
          case 'debug':
            originalConsole.debug(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
            break;
          case 'info':
            originalConsole.info(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
            break;
          case 'warn':
            originalConsole.warn(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
            break;
          case 'error':
            originalConsole.error(logPrefix, entry.message, entry.details !== undefined ? entry.details : '');
            break;
        }
      }
    } finally {
      isLoggingToConsole = false; // Reset flag
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

// Track if interceptor is already installed
let interceptorInstalled = false;

// Override console methods to capture all logs
export function installConsoleInterceptor() {
  if (typeof window !== 'undefined' && !interceptorInstalled) {
    interceptorInstalled = true;
    
    // Store original console methods globally to avoid recursion issues
    const originalConsole = {
      log: console.log,
      debug: console.debug,
      info: console.info,
      warn: console.warn,
      error: console.error
    };
    
    // Store original console methods in window for access in other functions
    (window as any).__originalConsole__ = originalConsole;

    // Helper function to process arguments
    const processArgs = (args: any[]): string => {
      return args.map(arg => {
        if (arg === undefined) return 'undefined';
        if (arg === null) return 'null';
        if (typeof arg === 'object') {
          try {
            return JSON.stringify(arg);
          } catch (e) {
            return '[Object]';
          }
        }
        return String(arg);
      }).join(' ');
    };

    // Override console.log
    console.log = function(...args) {
      // Call original method first
      originalConsole.log.apply(console, args);
      
      // If we're already logging, don't create a recursive loop
      if (!isLoggingToConsole) {
        // Add to our log system
        addLogEntry({
          timestamp: Date.now(),
          level: 'info',
          message: processArgs(args),
          module: 'console'
        });
      }
    };

    // Override console.debug
    console.debug = function(...args) {
      // Call original method first
      originalConsole.debug.apply(console, args);
      
      // If we're already logging, don't create a recursive loop
      if (!isLoggingToConsole) {
        // Add to our log system
        addLogEntry({
          timestamp: Date.now(),
          level: 'debug',
          message: processArgs(args),
          module: 'console'
        });
      }
    };

    // Override console.info
    console.info = function(...args) {
      // Call original method first
      originalConsole.info.apply(console, args);
      
      // If we're already logging, don't create a recursive loop
      if (!isLoggingToConsole) {
        // Add to our log system
        addLogEntry({
          timestamp: Date.now(),
          level: 'info',
          message: processArgs(args),
          module: 'console'
        });
      }
    };

    // Override console.warn
    console.warn = function(...args) {
      // Call original method first
      originalConsole.warn.apply(console, args);
      
      // If we're already logging, don't create a recursive loop
      if (!isLoggingToConsole) {
        // Add to our log system
        addLogEntry({
          timestamp: Date.now(),
          level: 'warn',
          message: processArgs(args),
          module: 'console'
        });
      }
    };

    // Override console.error
    console.error = function(...args) {
      // Call original method first
      originalConsole.error.apply(console, args);
      
      // If we're already logging, don't create a recursive loop
      if (!isLoggingToConsole) {
        // Add to our log system
        addLogEntry({
          timestamp: Date.now(),
          level: 'error',
          message: processArgs(args),
          module: 'console'
        });
      }
    };
  }
}