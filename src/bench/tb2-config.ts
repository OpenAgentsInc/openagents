/**
 * TB2 Task Configuration Loader
 *
 * Loads task.toml files from Terminal-Bench 2 tasks to extract
 * environment configuration (Docker image, resource limits, etc.)
 */

import { parse as parseToml } from "smol-toml";
import { existsSync } from "node:fs";
import { join } from "node:path";

/**
 * Environment configuration from TB2 task.toml
 */
export interface TB2EnvironmentConfig {
  /** Docker image to use (e.g., "alexgshaw/regex-log:20251031") */
  docker_image?: string;
  /** Number of CPUs (default: 1) */
  cpus?: number;
  /** Memory limit (e.g., "2G", "4096M") */
  memory?: string;
  /** Storage limit (e.g., "10G") */
  storage?: string;
  /** Build timeout in seconds (default: 600) */
  build_timeout_sec?: number;
}

/**
 * Agent configuration from TB2 task.toml
 */
export interface TB2AgentConfig {
  /** Agent timeout in seconds */
  timeout_sec?: number;
}

/**
 * Verifier configuration from TB2 task.toml
 */
export interface TB2VerifierConfig {
  /** Verifier timeout in seconds */
  timeout_sec?: number;
}

/**
 * Full TB2 task.toml structure
 */
export interface TB2TaskConfig {
  version?: string;
  metadata?: Record<string, unknown>;
  environment?: TB2EnvironmentConfig;
  agent?: TB2AgentConfig;
  verifier?: TB2VerifierConfig;
}

/**
 * Load task environment configuration from task.toml
 *
 * @param sourcePath - Path to TB2 task directory (e.g., /path/to/terminal-bench-2/regex-log)
 * @returns Environment config or empty object if task.toml doesn't exist
 */
export async function loadTaskEnvironment(sourcePath: string): Promise<TB2EnvironmentConfig> {
  const config = await loadTaskConfig(sourcePath);
  return config.environment || {};
}

/**
 * Load full task configuration from task.toml
 *
 * @param sourcePath - Path to TB2 task directory
 * @returns Parsed task config or empty object if task.toml doesn't exist
 */
export async function loadTaskConfig(sourcePath: string): Promise<TB2TaskConfig> {
  const tomlPath = join(sourcePath, "task.toml");

  if (!existsSync(tomlPath)) {
    return {};
  }

  try {
    const content = await Bun.file(tomlPath).text();
    const parsed = parseToml(content) as TB2TaskConfig;
    return parsed;
  } catch (error) {
    console.warn(`[TB2] Failed to parse task.toml at ${tomlPath}:`, error);
    return {};
  }
}

/**
 * Get memory limit in megabytes from string format
 *
 * @param memory - Memory string (e.g., "2G", "4096M")
 * @returns Memory in MB
 */
export function parseMemoryLimit(memory?: string): number {
  if (!memory) return 2048; // Default 2GB

  const match = memory.match(/^(\d+(?:\.\d+)?)(G|M|K)?$/i);
  if (!match) return 2048;

  const value = parseFloat(match[1]);
  const unit = (match[2] || "M").toUpperCase();

  switch (unit) {
    case "G":
      return Math.floor(value * 1024);
    case "M":
      return Math.floor(value);
    case "K":
      return Math.floor(value / 1024);
    default:
      return Math.floor(value);
  }
}
