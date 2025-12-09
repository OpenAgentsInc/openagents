/**
 * TB2 Container Configuration
 *
 * Maps TB2 task configuration to ContainerConfig for sandbox infrastructure.
 */

import type { ContainerConfig } from "../sandbox/schema.js";
import type { TB2EnvironmentConfig } from "./tb2-config.js";
import { parseMemoryLimit } from "./tb2-config.js";

export interface TB2ContainerOptions {
  /** Docker image to use */
  image: string;
  /** Host workspace directory (will be mounted to /app/) */
  workspace: string;
  /** Environment config from task.toml */
  envConfig?: TB2EnvironmentConfig;
  /** Verification timeout in seconds */
  verificationTimeout?: number;
}

/**
 * Create ContainerConfig for TB2 verification
 *
 * Maps TB2 task requirements to sandbox ContainerConfig:
 * - Mounts workspace to /app/ (TB2 standard)
 * - Sets resource limits from task.toml
 * - Configures environment variables
 *
 * @param options - TB2 container options
 * @returns ContainerConfig for sandbox backend
 */
export function createTB2ContainerConfig(
  options: TB2ContainerOptions
): ContainerConfig {
  const { image, workspace, envConfig = {}, verificationTimeout = 120 } = options;

  return {
    image,
    workspaceDir: workspace,
    workdir: "/app", // TB2 standard working directory
    memoryLimit: envConfig.memory || "2G",
    cpuLimit: envConfig.cpus || 1,
    timeoutMs: verificationTimeout * 1000,
    autoRemove: true,
    env: {
      PYTHONUNBUFFERED: "1", // For pytest output
    },
  };
}

/**
 * Get memory limit in MB from ContainerConfig format
 *
 * @param config - Container config
 * @returns Memory in MB
 */
export function getMemoryLimitMB(config: ContainerConfig): number {
  return parseMemoryLimit(config.memoryLimit);
}
