// Re-export types
export {
  ContainerError,
  ContainerConfigSchema,
  type ContainerConfig,
  type ContainerRunResult,
  type ContainerErrorReason,
} from "./schema.js";
export {
  ContainerBackendTag,
  type ContainerBackend,
  type ContainerRunOptions,
} from "./backend.js";

// Re-export credential utilities
export {
  extractCredentialsFromKeychain,
  createCredentialMount,
  cleanupCredentialMount,
  CredentialError,
  type CredentialMount,
  type CredentialErrorReason,
} from "./credentials.js";

// Re-export implementations
export { macOSContainerLayer, macOSContainerLive } from "./macos-container.js";
export { dockerBackendLayer, dockerBackendLive } from "./docker.js";
export { detectBackend, autoDetectLayer } from "./detect.js";
export { createSandboxHudAdapter, type SandboxHudAdapter } from "./hud-adapter.js";

// Re-export bootstrap utilities
export {
  checkStatus,
  bootstrap,
  downloadInstaller,
  installFromPkg,
  installSilent,
  startSystem,
  stopSystem,
  runBootstrap,
  runCheckStatus,
  type BootstrapStatus,
  type BootstrapResult,
} from "./bootstrap.js";

// ─────────────────────────────────────────────────────────────────────────────
// Convenience Functions
// ─────────────────────────────────────────────────────────────────────────────

import { Effect } from "effect";
import { ContainerBackendTag, type ContainerRunOptions } from "./backend.js";
import type { ContainerConfig } from "./schema.js";

/**
 * Run a command in a sandboxed container.
 *
 * When `onStdout`/`onStderr` callbacks are provided, output chunks are
 * streamed as they arrive. The final result still contains the accumulated
 * stdout/stderr (up to size limit).
 *
 * @example
 * ```typescript
 * const result = yield* runInContainer(
 *   ["bun", "test"],
 *   { image: "mechacoder:latest", workspaceDir: "/path/to/project" },
 *   {
 *     onStdout: (chunk) => console.log(chunk),
 *     onStderr: (chunk) => console.error(chunk),
 *   }
 * );
 * ```
 */
export const runInContainer = (
  command: string[],
  config: ContainerConfig,
  options?: ContainerRunOptions,
) =>
  Effect.gen(function* () {
    const backend = yield* ContainerBackendTag;
    return yield* backend.run(command, config, options);
  });

/**
 * Build a container image.
 *
 * @example
 * ```typescript
 * yield* buildImage("/path/to/project", "mechacoder:latest");
 * ```
 */
export const buildImage = (
  contextDir: string,
  tag: string,
  options?: {
    file?: string;
    memoryLimit?: string;
    cpuLimit?: number;
  },
) =>
  Effect.gen(function* () {
    const backend = yield* ContainerBackendTag;
    return yield* backend.build(contextDir, tag, options);
  });

/**
 * Check if any container backend is available.
 */
export const isContainerAvailable = () =>
  Effect.gen(function* () {
    const backend = yield* ContainerBackendTag;
    return yield* backend.isAvailable();
  });
