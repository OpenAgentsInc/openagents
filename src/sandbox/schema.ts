import * as S from "@effect/schema/Schema";

// ─────────────────────────────────────────────────────────────────────────────
// Error Types (following ToolExecutionError pattern from src/tools/schema.ts)
// ─────────────────────────────────────────────────────────────────────────────

export type ContainerErrorReason =
  | "not_available" // Container runtime not installed/running
  | "image_not_found" // Specified image doesn't exist
  | "start_failed" // Container failed to start
  | "execution_failed" // Command inside container failed
  | "timeout" // Operation timed out
  | "aborted"; // User/signal aborted

export class ContainerError extends Error {
  readonly _tag = "ContainerError";
  constructor(
    readonly reason: ContainerErrorReason,
    message: string,
    readonly exitCode?: number,
  ) {
    super(message);
  }
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration Schema
// ─────────────────────────────────────────────────────────────────────────────

export const ContainerConfigSchema = S.Struct({
  /** Image to run (e.g., "mechacoder:latest" or "oven/bun:latest") */
  image: S.String,

  /** Host directory to mount as /workspace inside container */
  workspaceDir: S.String,

  /** Working directory inside container (default: /workspace) */
  workdir: S.optional(S.String),

  /** Memory limit with optional suffix K/M/G (e.g., "4G") */
  memoryLimit: S.optional(S.String),

  /** Number of CPUs to allocate */
  cpuLimit: S.optional(S.Number),

  /** Environment variables to set */
  env: S.optional(S.Record({ key: S.String, value: S.String })),

  /** Timeout in milliseconds for the entire operation */
  timeoutMs: S.optional(S.Number),

  /** Remove container after it exits (default: true) */
  autoRemove: S.optional(S.Boolean),
});

export type ContainerConfig = S.Schema.Type<typeof ContainerConfigSchema>;

// ─────────────────────────────────────────────────────────────────────────────
// Execution Result
// ─────────────────────────────────────────────────────────────────────────────

export interface ContainerRunResult {
  exitCode: number;
  stdout: string;
  stderr: string;
  /** Container ID (useful for debugging) */
  containerId?: string;
}
