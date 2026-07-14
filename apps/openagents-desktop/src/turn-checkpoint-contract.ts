/**
 * Hidden-ref turn checkpoint contract (GIT-1, #8781).
 *
 * Desktop coding turns mutate the active workspace with no cheap per-turn
 * restore point. This contract types the lean middle demonstrated by the T3
 * Code teardown: workspace state is captured at turn boundaries as HIDDEN Git
 * refs (`refs/openagents/checkpoints/<thread>/<turn>`) built through an
 * isolated temporary `GIT_INDEX_FILE` — no commits on user branches, no
 * user-index writes, no worktree pollution. Revert is an explicit staged
 * command (stage -> inspect -> commit/clear) that carries an irreversible-
 * effects statement per the OpenCode V2 teardown §17 staged-rewind lessons.
 *
 * Privacy boundary: checkpoint snapshots can contain secrets. Refs and their
 * objects stay in the LOCAL repository only — records carry public-safe refs
 * and counts, never file bytes, and nothing here lands in Sync projections.
 */

export const TURN_CHECKPOINT_CONTRACT =
  "openagents_desktop.workbench.turn_checkpoints.v1" as const

/** Root namespace for hidden checkpoint refs. Never under refs/heads or refs/tags. */
export const TURN_CHECKPOINT_REF_ROOT = "refs/openagents/checkpoints" as const

/** Per-file capture bound: larger files are excluded from the snapshot. */
export const TURN_CHECKPOINT_MAX_FILE_BYTES = 2_000_000

/** Total candidate-file bound: larger workspaces refuse capture honestly. */
export const TURN_CHECKPOINT_MAX_FILES = 50_000

/** Bounded patch bytes returned by the typed diff query. */
export const TURN_CHECKPOINT_MAX_DIFF_BYTES = 120_000

/** The two reactor-observed turn boundaries a checkpoint can record. */
export const turnCheckpointBoundaries = ["turn_start", "turn_completed"] as const
export type TurnCheckpointBoundary = (typeof turnCheckpointBoundaries)[number]

/**
 * Typed failure classes. The service maps every real failure onto one of
 * these; callers render the class, never raw stderr.
 */
export const turnCheckpointErrorCodes = [
  "no_workspace",
  "not_a_repo",
  "git_unavailable",
  "workspace_too_large",
  "checkpoint_missing",
  "revert_already_staged",
  "no_staged_revert",
  "dirty_conflicting_state",
  "operation_failed",
] as const
export type TurnCheckpointErrorCode = (typeof turnCheckpointErrorCodes)[number]

export type TurnCheckpointError = Readonly<{
  ok: false
  error: TurnCheckpointErrorCode
  /** Bounded, repo-relative conflicting paths for dirty_conflicting_state. */
  conflictingPaths?: ReadonlyArray<string>
}>

export const turnCheckpointError = (
  error: TurnCheckpointErrorCode,
  conflictingPaths?: ReadonlyArray<string>,
): TurnCheckpointError =>
  conflictingPaths === undefined
    ? { ok: false, error }
    : { ok: false, error, conflictingPaths }

/**
 * The irreversible-effects statement every staged revert carries (OpenCode V2
 * teardown §17). Committing a revert restores checkpointed file bytes and
 * NOTHING else; these effects are outside what any checkpoint can undo.
 */
export const TURN_CHECKPOINT_IRREVERSIBLE_EFFECTS: ReadonlyArray<string> = [
  "Database writes, network requests, and processes started by past turns are not reverted.",
  "Git metadata — branches, the user index, stashes, remotes, and config — is not touched or reverted.",
  "Ignored files, files over the capture size bound, and paths outside this workspace are not captured or restored.",
  "Work from interrupted turns or concurrent edits may be only partially captured.",
  "Checkpoint snapshots may contain secrets; revert is not erasure. Snapshots stay in local hidden refs until their thread's checkpoints are deleted, and never enter Sync projections.",
]

/** One captured checkpoint: public-safe record facts only, never file bytes. */
export type TurnCheckpointRecord = Readonly<{
  schema: "openagents.desktop.turn_checkpoint.v1"
  threadRef: string
  turnRef: string
  boundary: TurnCheckpointBoundary
  /** Full hidden ref name, e.g. refs/openagents/checkpoints/<thread>/<turn>. */
  refName: string
  commit: string
  capturedAt: string
  /** Files admitted into the snapshot tree. */
  fileCount: number
  /** Candidate files excluded by the per-file size bound. */
  excludedOversizeCount: number
}>

/** Typed completion signal (local SIG-1-shaped event until that seam lands). */
export type TurnCheckpointSignal =
  | Readonly<{ kind: "checkpoint_captured"; record: TurnCheckpointRecord }>
  | Readonly<{
      kind: "revert_committed"
      threadRef: string
      turnRef: string
      restoredCount: number
      deletedCount: number
      committedAt: string
    }>

export type TurnCheckpointCaptureInput = Readonly<{
  threadRef: string
  turnRef: string
  boundary: TurnCheckpointBoundary
}>

export type TurnCheckpointCaptureResult =
  | Readonly<{ ok: true; record: TurnCheckpointRecord }>
  | TurnCheckpointError

export type TurnCheckpointDiffInput = Readonly<{
  threadRef: string
  fromTurnRef: string
  toTurnRef: string
}>

export type TurnCheckpointDiffFile = Readonly<{
  path: string
  additions: number
  deletions: number
  binary: boolean
}>

export type TurnCheckpointDiffResult =
  | Readonly<{
      ok: true
      threadRef: string
      fromTurnRef: string
      toTurnRef: string
      files: ReadonlyArray<TurnCheckpointDiffFile>
      /** Unified patch, bounded to TURN_CHECKPOINT_MAX_DIFF_BYTES. */
      patch: string
      truncated: boolean
    }>
  | TurnCheckpointError

export type TurnCheckpointRevertAction = "restore" | "delete"

export type TurnCheckpointRevertPlanEntry = Readonly<{
  path: string
  action: TurnCheckpointRevertAction
}>

/** A staged (not yet applied) revert: inspect it, then commit or clear. */
export type StagedTurnCheckpointRevert = Readonly<{
  threadRef: string
  turnRef: string
  targetCommit: string
  /** Snapshot of the pre-revert worktree, retained as the redo baseline. */
  baselineCommit: string
  plan: ReadonlyArray<TurnCheckpointRevertPlanEntry>
  irreversibleEffects: ReadonlyArray<string>
  stagedAt: string
}>

export type TurnCheckpointStageResult =
  | Readonly<{ ok: true; staged: StagedTurnCheckpointRevert }>
  | TurnCheckpointError

export type TurnCheckpointInspectResult =
  | Readonly<{
      ok: true
      staged: StagedTurnCheckpointRevert
      /** Patch from the staged baseline to the revert target, bounded. */
      patch: string
      truncated: boolean
    }>
  | TurnCheckpointError

export type TurnCheckpointCommitRevertResult =
  | Readonly<{ ok: true; restoredCount: number; deletedCount: number }>
  | TurnCheckpointError

export type TurnCheckpointClearRevertResult =
  | Readonly<{ ok: true }>
  | TurnCheckpointError

export type TurnCheckpointDeleteThreadResult =
  | Readonly<{ ok: true; deletedRefCount: number }>
  | TurnCheckpointError
