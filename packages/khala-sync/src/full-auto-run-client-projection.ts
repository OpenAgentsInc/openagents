import { Schema as S } from "effect";

/**
 * FA-RUN-05 (#8981): the public-safe, cross-device projection of the
 * signed-in user's currently active `FullAutoRun` (if any) -- published by
 * Desktop (`apps/openagents-desktop/src/full-auto-run-registry.ts`, FA-RUN-01
 * #8969) and consumed by mobile (#8982) via the same `khala-sync-client`
 * fetch-projection ergonomics `FleetRunClientProjection` already established
 * (`fleet-run-client-projection.ts`).
 *
 * This is deliberately a STANDALONE v1 schema, not a reuse of
 * `FleetRunClientProjection` (different domain: Sarah fleet orchestration vs.
 * Full Auto's single-thread objective/lifecycle model) and not yet the
 * canonical bounded run report/receipt shape #8972 (FA-RUN-04) will define
 * once it lands. Per #8980/#8981's explicit disposition, this v1 projection
 * should be reconciled with #8972's schema once that lands rather than left
 * as a permanent parallel format.
 *
 * Public-safe boundary (matches the FullAutoRun objective/receipt privacy
 * boundary already established by ProductSpec rev 10 and
 * `full-auto-run-registry.ts`'s own header comment): NEVER include raw
 * prompts, tool output, local file paths, or credentials. `workspaceLabel` is
 * deliberately NOT the raw `workspaceRef` (which can be a local filesystem
 * path) -- callers must derive a short public-safe label (e.g. a directory
 * basename) before publishing.
 */
export const FULL_AUTO_RUN_CLIENT_PROJECTION_SCHEMA = "full_auto_run.mobile_projection.v1" as const;

const FullAutoRunRef = S.String.check(S.isMinLength(1), S.isMaxLength(180));
const FullAutoThreadRef = S.String.check(S.isMinLength(1), S.isMaxLength(180));
const FullAutoRunObjectiveText = S.String.check(S.isMinLength(1), S.isMaxLength(4000));
const FullAutoRunDoneConditionText = S.String.check(S.isMinLength(1), S.isMaxLength(2000));
/** A short public-safe label ONLY -- never a raw local filesystem path. */
const FullAutoRunWorkspaceLabel = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(200),
  S.isPattern(/^[^/\\]*$/u),
);
const FullAutoRunTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/u),
);
/** A short public-safe lane/account ref (e.g. "codex-local") -- never a raw
 * credential, model name, or account email. */
const FullAutoRunShortRef = S.String.check(S.isMinLength(1), S.isMaxLength(80));
const FullAutoRunCount = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const FullAutoRunDigest = S.String.check(S.isLengthBetween(64, 64));

/**
 * The exact lifecycle enumeration from `full-auto-run-registry.ts`'s
 * `FullAutoRunStateSchema` (FA-RUN-01 #8969) -- kept in lockstep by the
 * co-located test that round-trips every registry state through this schema.
 */
export const FullAutoRunClientLifecycleState = S.Literals([
  "draft",
  "running",
  "pausing",
  "paused",
  "retrying",
  "stalled",
  "completed",
  "failed",
  "stopped",
  "cap_reached",
]);
export type FullAutoRunClientLifecycleState = typeof FullAutoRunClientLifecycleState.Type;

/**
 * The exact attribution vocabulary from `FullAutoRunActorSchema`. `mobile`
 * (MOB-FA-02 #8994) is the phone-originated Pause/Resume/Stop actor -- a
 * mobile-dispatched control intent that Desktop applies is attributed here,
 * never silently folded into `control_api`.
 */
export const FullAutoRunClientActor = S.Literals([
  "owner_ui",
  "control_api",
  "cli",
  "mcp",
  "workspace_guard",
  "continuation_cap",
  "dispatch_failure_limit",
  "turn_resolution",
  "thread_state_sync",
  "legacy_migration",
  "liveness_monitor",
  "guardrail",
  "mobile",
]);
export type FullAutoRunClientActor = typeof FullAutoRunClientActor.Type;

export const FullAutoRunClientTransitionAttribution = S.Struct({
  actor: FullAutoRunClientActor,
  at: FullAutoRunTimestamp,
});
export type FullAutoRunClientTransitionAttribution = typeof FullAutoRunClientTransitionAttribution.Type;

/**
 * MOB-FA-02 (#8994): the bounded, public-safe run-report summary surfaced
 * once a run reaches a terminal lifecycle state. Field-for-field mirror of
 * `FullAutoRunReceiptSchema` (`apps/openagents-desktop/src/full-auto-run-report.ts`,
 * FA-RUN-04 #8972 -- itself already the redaction-tested public-safe
 * derivation of the private `FullAutoRunReport`). Mirrored rather than
 * imported so this shared schema package never takes a dependency on the
 * Desktop app package; kept in lockstep by a co-located coupling test.
 */
export const FullAutoRunClientReceiptSchema = "full_auto_run.mobile_receipt.v1" as const;

export const FullAutoRunClientProviderHandoffDisposition = S.Literals([
  "complete_within_bounds",
  "truncated_with_confirmation",
  "refused",
]);
export type FullAutoRunClientProviderHandoffDisposition =
  typeof FullAutoRunClientProviderHandoffDisposition.Type;

export const FullAutoRunClientRecoveryAction = S.Literals(["retry_now", "stop_only", "none"]);
export type FullAutoRunClientRecoveryAction = typeof FullAutoRunClientRecoveryAction.Type;

export const FullAutoRunClientReceiptSummary = S.Struct({
  schema: S.Literal(FullAutoRunClientReceiptSchema),
  runRef: FullAutoRunRef,
  threadRef: S.optional(FullAutoThreadRef),
  objectiveDigest: FullAutoRunDigest,
  doneConditionDigest: FullAutoRunDigest,
  workspaceRefDigest: S.NullOr(FullAutoRunDigest),
  state: FullAutoRunClientLifecycleState,
  startedAt: S.optional(FullAutoRunTimestamp),
  endedAt: S.optional(FullAutoRunTimestamp),
  turnCap: FullAutoRunCount,
  successfulAttempts: FullAutoRunCount,
  failedAttempts: FullAutoRunCount,
  providerIdentities: S.Array(FullAutoRunShortRef).check(S.isMaxLength(32)),
  providerTransitionCount: FullAutoRunCount,
  providerTransitionDispositions: S.Array(FullAutoRunClientProviderHandoffDisposition).check(
    S.isMaxLength(64),
  ),
  livenessGapCount: FullAutoRunCount,
  recoveryActionsUsed: S.Array(FullAutoRunClientRecoveryAction).check(S.isMaxLength(3)),
  verifiedRefCount: FullAutoRunCount,
  claimedRefCount: FullAutoRunCount,
  progressDisposition: S.Literal("unknown"),
  usageKnown: S.Boolean,
  reportRevision: FullAutoRunCount,
  createdAt: FullAutoRunTimestamp,
});
export type FullAutoRunClientReceiptSummary = typeof FullAutoRunClientReceiptSummary.Type;

export const FullAutoRunClientRunProjection = S.Struct({
  runRef: FullAutoRunRef,
  threadRef: S.NullOr(FullAutoThreadRef),
  objective: FullAutoRunObjectiveText,
  doneCondition: FullAutoRunDoneConditionText,
  lifecycleState: FullAutoRunClientLifecycleState,
  workspaceLabel: S.NullOr(FullAutoRunWorkspaceLabel),
  startedAt: S.NullOr(FullAutoRunTimestamp),
  updatedAt: FullAutoRunTimestamp,
  lastTransition: FullAutoRunClientTransitionAttribution,
  /** MOB-FA-02 (#8994): the run's currently bound provider lane/account, or
   * `null` when not yet bound. Short public-safe refs only (e.g.
   * "codex-local"), never a raw credential or account email. */
  laneRef: S.NullOr(FullAutoRunShortRef),
  accountRef: S.NullOr(FullAutoRunShortRef),
  /** MOB-FA-02 (#8994): continuations-vs-cap so mobile can render "7 / 20"
   * without a second fetch. Mirrors `FullAutoRun.turnCap`/`successfulAttempts`/
   * `failedAttempts` (`full-auto-run-registry.ts`). */
  turnCap: FullAutoRunCount,
  successfulAttempts: FullAutoRunCount,
  failedAttempts: FullAutoRunCount,
  /** MOB-FA-02 (#8994): count of typed same-pass provider-lane rotations
   * (FA-RT-01 #8987's `rotationHistory` on the bound thread record) --
   * always the bounded count, never the raw history. */
  rotationCount: FullAutoRunCount,
  /** MOB-FA-02 (#8994): present only once the run reaches a terminal
   * lifecycle state; `null` for every non-terminal state. */
  receiptSummary: S.NullOr(FullAutoRunClientReceiptSummary),
});
export type FullAutoRunClientRunProjection = typeof FullAutoRunClientRunProjection.Type;

export const FullAutoRunClientProjection = S.Struct({
  schema: S.Literal(FULL_AUTO_RUN_CLIENT_PROJECTION_SCHEMA),
  privateMaterialExcluded: S.Literal(true),
  generatedAt: FullAutoRunTimestamp,
  /** The single active (or last-published) run for this account, or `null`
   * when nothing has ever been published / the owner has no active run. */
  run: S.NullOr(FullAutoRunClientRunProjection),
});
export type FullAutoRunClientProjection = typeof FullAutoRunClientProjection.Type;

export const decodeFullAutoRunClientProjection = (
  value: unknown,
): FullAutoRunClientProjection =>
  S.decodeUnknownSync(FullAutoRunClientProjection)(value, {
    onExcessProperty: "error",
  });
