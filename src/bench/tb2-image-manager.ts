/**
 * TB2 Image Manager
 *
 * Handles Docker image availability for TB2 tasks:
 * 1. Try to use prebuilt image from docker_image field
 * 2. Build from Dockerfile if available
 * 3. Fall back to generic python:3.11-slim
 */

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import type { TB2EnvironmentConfig } from "./tb2-config.js";

export interface ImageOptions {
  /** Memory limit for build (e.g., "2G") */
  memoryLimit?: string;
  /** Build timeout in milliseconds */
  timeout?: number;
  /** Whether to force rebuild even if image exists */
  forceBuild?: boolean;
}

/**
 * Ensure Docker image is available for TB2 task
 *
 * Strategy:
 * 1. If docker_image specified, check if it exists locally (skip pull to save time)
 * 2. Build from Dockerfile if available
 * 3. Fall back to python:3.11-slim
 *
 * @param taskId - Task ID (for building local image tag)
 * @param sourcePath - Path to TB2 task directory
 * @param envConfig - Environment config from task.toml
 * @param options - Image options
 * @returns Docker image name/tag to use
 */
export async function ensureTaskImage(
  taskId: string,
  sourcePath: string,
  envConfig: TB2EnvironmentConfig,
  options: ImageOptions = {}
): Promise<string> {
  const { timeout = 600000, forceBuild = false } = options;

  // 1. If prebuilt image specified, use it (assume it exists or will be pulled on demand)
  if (envConfig.docker_image && !forceBuild) {
    const exists = await imageExists(envConfig.docker_image);
    if (exists) {
      console.log(`[TB2] Using existing image: ${envConfig.docker_image}`);
      return envConfig.docker_image;
    }

    // Try to pull it
    console.log(`[TB2] Pulling image: ${envConfig.docker_image}`);
    const pulled = await pullImage(envConfig.docker_image, timeout);
    if (pulled) {
      return envConfig.docker_image;
    }

    console.warn(`[TB2] Failed to pull ${envConfig.docker_image}, will try building from Dockerfile`);
  }

  // 2. Build from Dockerfile if available
  const dockerfilePath = join(sourcePath, "environment", "Dockerfile");
  if (existsSync(dockerfilePath)) {
    const imageTag = `tb2-${taskId}:local`;
    console.log(`[TB2] Building image from Dockerfile: ${imageTag}`);

    const built = await buildImage(
      join(sourcePath, "environment"),
      imageTag,
      {
        memoryLimit: envConfig.memory || "2G",
        timeout,
      }
    );

    if (built) {
      return imageTag;
    }

    console.warn(`[TB2] Failed to build from Dockerfile`);
  }

  // 3. Fall back to generic image
  console.log(`[TB2] Using fallback image: python:3.11-slim`);
  return "python:3.11-slim";
}

/**
 * Check if Docker image exists locally
 */
async function imageExists(imageName: string): Promise<boolean> {
  return new Promise((resolve) => {
    const proc = spawn("docker", ["image", "inspect", imageName], {
      stdio: ["ignore", "pipe", "pipe"],
    });

    proc.on("close", (code) => {
      resolve(code === 0);
    });

    proc.on("error", () => {
      resolve(false);
    });
  });
}

/**
 * Pull Docker image
 */
async function pullImage(imageName: string, timeout: number): Promise<boolean> {
  return new Promise((resolve) => {
    const proc = spawn("docker", ["pull", imageName], {
      stdio: ["ignore", "inherit", "inherit"],
    });

    const timer = setTimeout(() => {
      proc.kill("SIGTERM");
      resolve(false);
    }, timeout);

    proc.on("close", (code) => {
      clearTimeout(timer);
      resolve(code === 0);
    });

    proc.on("error", () => {
      clearTimeout(timer);
      resolve(false);
    });
  });
}

/**
 * Build Docker image from Dockerfile
 */
async function buildImage(
  contextDir: string,
  tag: string,
  options: ImageOptions
): Promise<boolean> {
  const { memoryLimit, timeout = 600000 } = options;

  return new Promise((resolve) => {
    const args = ["build", "-t", tag];

    if (memoryLimit) {
      args.push(`--memory=${memoryLimit}`);
    }

    args.push(contextDir);

    const proc = spawn("docker", args, {
      stdio: ["ignore", "inherit", "inherit"],
    });

    const timer = setTimeout(() => {
      proc.kill("SIGTERM");
      resolve(false);
    }, timeout);

    proc.on("close", (code) => {
      clearTimeout(timer);
      resolve(code === 0);
    });

    proc.on("error", () => {
      clearTimeout(timer);
      resolve(false);
    });
  });
}
