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

/** The exact attribution vocabulary from `FullAutoRunActorSchema`. */
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
]);
export type FullAutoRunClientActor = typeof FullAutoRunClientActor.Type;

export const FullAutoRunClientTransitionAttribution = S.Struct({
  actor: FullAutoRunClientActor,
  at: FullAutoRunTimestamp,
});
export type FullAutoRunClientTransitionAttribution = typeof FullAutoRunClientTransitionAttribution.Type;

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
