/**
 * HillClimber Logger
 *
 * Simple file logger that writes to logs/hillclimber.log
 * Also outputs to console for real-time monitoring.
 */

import { mkdirSync, appendFileSync } from "node:fs";
import { join } from "node:path";

const LOG_DIR = "logs";
const LOG_FILE = "hillclimber.log";

// Ensure log directory exists
try {
  mkdirSync(LOG_DIR, { recursive: true });
} catch {
  // Directory might already exist, ignore
}

const LOG_PATH = join(LOG_DIR, LOG_FILE);

/**
 * Write a log message to both console and file.
 */
export const log = (message: string): void => {
  const timestamp = new Date().toISOString();
  const logLine = `[${timestamp}] ${message}\n`;

  // Write to console
  console.log(message);

  // Append to log file
  try {
    appendFileSync(LOG_PATH, logLine, "utf8");
  } catch (error) {
    // If file write fails, at least we have console output
    console.error(`[HillClimber] Failed to write to log file: ${error}`);
  }
};

/**
 * Write an error message to both console and file.
 */
export const logError = (message: string, error?: Error): void => {
  const fullMessage = error
    ? `${message}: ${error.message}${error.stack ? `\n${error.stack}` : ""}`
    : message;
  const timestamp = new Date().toISOString();
  const logLine = `[${timestamp}] ERROR: ${fullMessage}\n`;

  // Write to console
  console.error(`[HillClimber] ${fullMessage}`);

  // Append to log file
  try {
    appendFileSync(LOG_PATH, logLine, "utf8");
  } catch (writeError) {
    // If file write fails, at least we have console output
    console.error(`[HillClimber] Failed to write to log file: ${writeError}`);
  }
};

/**
 * Get the path to the log file (for reference in docs/help).
 */
export const getLogPath = (): string => LOG_PATH;

