/**
 * Orchestrator Checkpoint System
 *
 * Provides crash recovery for the orchestrator by persisting state at key phase
 * transitions. When the orchestrator crashes, it can resume from the last checkpoint
 * instead of restarting from the beginning.
 *
 * Design principles:
 * - Atomic writes (write to temp file, then rename)
 * - Checkpoints expire after 24 hours
 * - Git state is captured for validation on resume
 * - Compatible with existing two-phase commit recovery
 *
 * @module agent/orchestrator/checkpoint
 */

import * as FileSystem from "@effect/platform/FileSystem";
import { Effect, Option } from "effect";
import type { OrchestratorPhase } from "./types.js";

// ============================================================================
// Types
// ============================================================================

/**
 * Git state captured at checkpoint time for validation on resume.
 */
export interface CheckpointGitState {
  readonly branch: string;
  readonly headCommit: string;
  readonly isDirty: boolean;
  readonly stagedFiles: readonly string[];
}

/**
 * Verification results captured after verify phase.
 */
export interface CheckpointVerification {
  readonly typecheckPassed: boolean;
  readonly testsPassed: boolean;
  readonly verifiedAt: string;
}

/**
 * Healer invocation record for session auditing.
 */
export interface CheckpointHealerInvocation {
  readonly scenario: string;
  readonly outcome: string;
  readonly timestamp: string;
}

/**
 * Main checkpoint schema.
 * Captures orchestrator state at phase boundaries for crash recovery.
 */
export interface OrchestratorCheckpoint {
  /** Schema version for future compatibility */
  readonly version: 1;
  /** Unique session identifier */
  readonly sessionId: string;
  /** When checkpoint was written */
  readonly timestamp: string;

  // Current position in workflow
  /** Current orchestrator phase */
  readonly phase: OrchestratorPhase;
  /** When current phase started */
  readonly phaseStartedAt: string;

  // Task context
  /** ID of task being worked on */
  readonly taskId: string;
  /** Title of task for human readability */
  readonly taskTitle: string;

  // Subtask progress
  /** IDs of completed subtasks */
  readonly completedSubtaskIds: readonly string[];
  /** ID of subtask currently being executed (null if not in execute phase) */
  readonly currentSubtaskId: string | null;

  // Git state
  /** Git repository state at checkpoint */
  readonly git: CheckpointGitState;

  // Optional: Verification results (populated after verify phase)
  readonly verification?: CheckpointVerification;

  // Healer audit trail
  readonly healerInvocations: readonly CheckpointHealerInvocation[];
}

/**
 * Result of checkpoint validation.
 */
export type CheckpointValidation =
  | { readonly valid: true; readonly checkpoint: OrchestratorCheckpoint }
  | { readonly valid: false; readonly reason: string };

// ============================================================================
// Constants
// ============================================================================

/** Checkpoint file name */
export const CHECKPOINT_FILENAME = "checkpoint.json";

/** Maximum age before checkpoint is considered stale (24 hours in ms) */
export const CHECKPOINT_MAX_AGE_MS = 24 * 60 * 60 * 1000;

// ============================================================================
// Path Helpers
// ============================================================================

/**
 * Get the path to the checkpoint file.
 */
export const getCheckpointPath = (openagentsDir: string): string =>
  `${openagentsDir}/${CHECKPOINT_FILENAME}`;

/**
 * Get the path to the temporary checkpoint file (used for atomic writes).
 */
const getTempCheckpointPath = (openagentsDir: string): string =>
  `${openagentsDir}/${CHECKPOINT_FILENAME}.tmp`;

// ============================================================================
// Git State Capture
// ============================================================================

/**
 * Capture current git state for checkpoint.
 */
export const captureGitState = (
  cwd: string,
): Effect.Effect<CheckpointGitState, Error, never> =>
  Effect.tryPromise({
    try: async () => {
      const { execSync } = await import("node:child_process");

      // Get current branch
      const branch = execSync("git rev-parse --abbrev-ref HEAD", {
        cwd,
        encoding: "utf-8",
      }).trim();

      // Get HEAD commit
      const headCommit = execSync("git rev-parse HEAD", {
        cwd,
        encoding: "utf-8",
      }).trim();

      // Check for dirty state (uncommitted changes)
      let isDirty = false;
      try {
        execSync("git diff --quiet", { cwd, encoding: "utf-8" });
        execSync("git diff --staged --quiet", { cwd, encoding: "utf-8" });
      } catch {
        isDirty = true;
      }

      // Get staged files
      const stagedOutput = execSync("git diff --staged --name-only", {
        cwd,
        encoding: "utf-8",
      }).trim();
      const stagedFiles = stagedOutput ? stagedOutput.split("\n") : [];

      return {
        branch,
        headCommit,
        isDirty,
        stagedFiles,
      };
    },
    catch: (error: unknown) =>
      new Error(
        `Failed to capture git state: ${error instanceof Error ? error.message : String(error)}`,
      ),
  });

// ============================================================================
// Checkpoint Persistence
// ============================================================================

/**
 * Write checkpoint atomically (temp file + rename).
 * This ensures the checkpoint file is never in a partial/corrupt state.
 */
export const writeCheckpoint = (
  openagentsDir: string,
  checkpoint: OrchestratorCheckpoint,
): Effect.Effect<void, Error, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const checkpointPath = getCheckpointPath(openagentsDir);
    const tempPath = getTempCheckpointPath(openagentsDir);

    // Write to temp file
    yield* fs.writeFileString(tempPath, JSON.stringify(checkpoint, null, 2)).pipe(
      Effect.mapError(
        (e) => new Error(`Failed to write temp checkpoint: ${e.message}`),
      ),
    );

    // Atomic rename
    yield* fs.rename(tempPath, checkpointPath).pipe(
      Effect.mapError(
        (e) => new Error(`Failed to rename checkpoint: ${e.message}`),
      ),
    );
  });

/**
 * Read existing checkpoint file.
 * Returns Option.none() if checkpoint doesn't exist.
 */
export const readCheckpoint = (
  openagentsDir: string,
): Effect.Effect<Option.Option<OrchestratorCheckpoint>, Error, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const checkpointPath = getCheckpointPath(openagentsDir);

    const exists = yield* fs.exists(checkpointPath);
    if (!exists) {
      return Option.none();
    }

    const content = yield* fs.readFileString(checkpointPath).pipe(
      Effect.mapError(
        (e) => new Error(`Failed to read checkpoint: ${e.message}`),
      ),
    );

    try {
      const checkpoint = JSON.parse(content) as OrchestratorCheckpoint;
      return Option.some(checkpoint);
    } catch (e) {
      return Option.none(); // Corrupted checkpoint, treat as non-existent
    }
  });

/**
 * Clear checkpoint file (on successful completion).
 */
export const clearCheckpoint = (
  openagentsDir: string,
): Effect.Effect<void, never, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    const fs = yield* FileSystem.FileSystem;
    const checkpointPath = getCheckpointPath(openagentsDir);

    yield* fs.remove(checkpointPath).pipe(Effect.ignore);

    // Also clean up temp file if it exists
    const tempPath = getTempCheckpointPath(openagentsDir);
    yield* fs.remove(tempPath).pipe(Effect.ignore);
  });

// ============================================================================
// Checkpoint Validation
// ============================================================================

/**
 * Validate a checkpoint for resumption.
 *
 * Checks:
 * - Checkpoint is not stale (>24 hours old)
 * - Git branch matches current branch
 * - Version is compatible
 */
export const validateCheckpoint = (
  checkpoint: OrchestratorCheckpoint,
  currentGitState: CheckpointGitState,
): CheckpointValidation => {
  // Check version
  if (checkpoint.version !== 1) {
    return {
      valid: false,
      reason: `Unsupported checkpoint version: ${checkpoint.version}`,
    };
  }

  // Check age
  const age = Date.now() - new Date(checkpoint.timestamp).getTime();
  if (age > CHECKPOINT_MAX_AGE_MS) {
    const hoursOld = Math.round(age / (60 * 60 * 1000));
    return {
      valid: false,
      reason: `Checkpoint is stale (${hoursOld} hours old, max 24 hours)`,
    };
  }

  // Check branch matches
  if (checkpoint.git.branch !== currentGitState.branch) {
    return {
      valid: false,
      reason: `Branch mismatch: checkpoint on '${checkpoint.git.branch}', now on '${currentGitState.branch}'`,
    };
  }

  return { valid: true, checkpoint };
};

/**
 * Check for and validate existing checkpoint.
 * Returns None if no valid checkpoint exists.
 */
export const maybeResumeCheckpoint = (
  openagentsDir: string,
  cwd: string,
): Effect.Effect<Option.Option<OrchestratorCheckpoint>, Error, FileSystem.FileSystem> =>
  Effect.gen(function* () {
    // Try to read existing checkpoint
    const maybeCheckpoint = yield* readCheckpoint(openagentsDir);

    if (Option.isNone(maybeCheckpoint)) {
      return Option.none();
    }

    const checkpoint = maybeCheckpoint.value;

    // Capture current git state
    const currentGitState = yield* captureGitState(cwd);

    // Validate checkpoint
    const validation = validateCheckpoint(checkpoint, currentGitState);

    if (!validation.valid) {
      // Invalid checkpoint - clear it and return none
      yield* clearCheckpoint(openagentsDir);
      return Option.none();
    }

    return Option.some(checkpoint);
  });

// ============================================================================
// Checkpoint Creation Helpers
// ============================================================================

/**
 * Create a new checkpoint with the given parameters.
 */
export const createCheckpoint = (params: {
  sessionId: string;
  phase: OrchestratorPhase;
  taskId: string;
  taskTitle: string;
  completedSubtaskIds: readonly string[];
  currentSubtaskId: string | null;
  git: CheckpointGitState;
  verification?: CheckpointVerification;
  healerInvocations?: readonly CheckpointHealerInvocation[];
}): OrchestratorCheckpoint => {
  const now = new Date().toISOString();
  const base = {
    version: 1 as const,
    sessionId: params.sessionId,
    timestamp: now,
    phase: params.phase,
    phaseStartedAt: now,
    taskId: params.taskId,
    taskTitle: params.taskTitle,
    completedSubtaskIds: params.completedSubtaskIds,
    currentSubtaskId: params.currentSubtaskId,
    git: params.git,
    healerInvocations: params.healerInvocations ?? [],
  };

  // Only include verification if explicitly provided (exactOptionalPropertyTypes)
  if (params.verification !== undefined) {
    return { ...base, verification: params.verification };
  }
  return base;
};

/**
 * Update an existing checkpoint with new phase.
 */
export const updateCheckpointPhase = (
  checkpoint: OrchestratorCheckpoint,
  phase: OrchestratorPhase,
  updates?: Partial<
    Pick<
      OrchestratorCheckpoint,
      "completedSubtaskIds" | "currentSubtaskId" | "verification" | "git"
    >
  >,
): OrchestratorCheckpoint => {
  const now = new Date().toISOString();
  return {
    ...checkpoint,
    timestamp: now,
    phase,
    phaseStartedAt: now,
    ...(updates?.completedSubtaskIds !== undefined && {
      completedSubtaskIds: updates.completedSubtaskIds,
    }),
    ...(updates?.currentSubtaskId !== undefined && {
      currentSubtaskId: updates.currentSubtaskId,
    }),
    ...(updates?.verification !== undefined && {
      verification: updates.verification,
    }),
    ...(updates?.git !== undefined && { git: updates.git }),
  };
};

/**
 * Add a healer invocation to the checkpoint.
 */
export const addHealerInvocation = (
  checkpoint: OrchestratorCheckpoint,
  invocation: CheckpointHealerInvocation,
): OrchestratorCheckpoint => ({
  ...checkpoint,
  timestamp: new Date().toISOString(),
  healerInvocations: [...checkpoint.healerInvocations, invocation],
});
