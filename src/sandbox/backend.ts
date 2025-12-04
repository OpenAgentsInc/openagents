import { Context, Effect } from "effect";
import type {
  ContainerConfig,
  ContainerRunResult,
  ContainerError,
} from "./schema.js";

// ─────────────────────────────────────────────────────────────────────────────
// Run Options
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Options for running a command in a container.
 */
export interface ContainerRunOptions {
  /** Abort signal for cancellation */
  signal?: AbortSignal;
  /** Callback for streaming stdout chunks as they arrive */
  onStdout?: (chunk: string) => void;
  /** Callback for streaming stderr chunks as they arrive */
  onStderr?: (chunk: string) => void;
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend Interface
// ─────────────────────────────────────────────────────────────────────────────

export interface ContainerBackend {
  /** Human-readable name for this backend */
  readonly name: string;

  /** Check if this backend is available on the current system */
  isAvailable: () => Effect.Effect<boolean, never, never>;

  /**
   * Run a command inside a container.
   *
   * When `onStdout`/`onStderr` callbacks are provided, output chunks are
   * streamed as they arrive. The final result still contains the accumulated
   * stdout/stderr (up to size limit).
   *
   * @param command - Command and arguments to run
   * @param config - Container configuration
   * @param options - Optional abort signal and streaming callbacks
   */
  run: (
    command: string[],
    config: ContainerConfig,
    options?: ContainerRunOptions,
  ) => Effect.Effect<ContainerRunResult, ContainerError, never>;

  /**
   * Build an image from a Dockerfile/Containerfile.
   *
   * @param contextDir - Build context directory
   * @param tag - Tag for the built image
   * @param options - Build options
   */
  build: (
    contextDir: string,
    tag: string,
    options?: {
      file?: string; // Path to Dockerfile/Containerfile
      memoryLimit?: string; // Builder memory limit
      cpuLimit?: number; // Builder CPU limit
    },
  ) => Effect.Effect<void, ContainerError, never>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Context.Tag (modern class-based pattern)
// ─────────────────────────────────────────────────────────────────────────────

export class ContainerBackendTag extends Context.Tag("ContainerBackend")<
  ContainerBackendTag,
  ContainerBackend
>() {}
