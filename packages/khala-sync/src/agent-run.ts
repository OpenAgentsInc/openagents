import { Schema as S } from "effect"

/**
 * Agent run + goal entity contract for `scope.agent_run.<runId>` (KS-6.6,
 * #8416; SPEC §2.1 taxonomy, §7 invariant 9).
 *
 * `scope.agent_run.<runId>` has been part of the read-auth taxonomy since
 * KS-7.1 (#8305, `scope-auth.ts`'s `case "agent_run"`), but until this
 * module NOTHING actually produced changelog entries for it: KS-8.13/#8324
 * projected Khala Code product state onto `scope.team.<teamId>` and
 * `scope.thread.<threadId>` only (`khala-code-product-state-projection.ts`'s
 * `scopesForRow` has no `agent_run` case, and `agent_runs`/`agent_goals` are
 * not even in `KHALA_CODE_PRODUCT_STATE_TABLES`). This module is the
 * extension that gives the scope a real shape: one post-image per queued/
 * relaunched agent run, carrying the run's own public state PLUS its
 * currently-attached goal's public-safe fields (KS-6.6's "cover goal
 * updates" ask).
 *
 * The shape mirrors two ALREADY-public-safe Worker projections so this
 * module invents nothing new:
 *   - `agentRunProjection` / `agentRunMissionProjection`
 *     (`apps/openagents.com/workers/api/src/omni-runs.ts`) — the run fields
 *     already shipped over the legacy sync-worker outbox to
 *     `agentRunScope(run.id)` today.
 *   - `publicGoalContext` (same file) — the goal fields already returned in
 *     the mission-launch HTTP response body (excludes the goal's
 *     `hiddenSteering`, which stays private).
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): `goal` (the user's
 * free-text objective) and `repository.owner`/`repository.repo` are the
 * only free-content fields — bounded, but otherwise unconstrained, exactly
 * like `KhalaCodeText`/chat `body` in ./khala-code.ts. Every other field is
 * a closed literal set, a bounded ref, an ISO timestamp, or a bounded
 * count/fraction. No provider credential, auth grant ref, callback token,
 * GitHub write grant, or filesystem path can decode into this shape — those
 * fields are never mapped by the server-side projector
 * (`@openagentsinc/khala-sync-server`'s `agent-run-projection.ts`).
 *
 * This module is deliberately self-contained (imports only `effect`) so it
 * can be re-exported from ./index without a module cycle — same rule as
 * ./khala-code and ./gym.
 */

// ---------------------------------------------------------------------------
// Entity type name (changelog `entityType` value)
// ---------------------------------------------------------------------------

export const AGENT_RUN_ENTITY_TYPE = "agent_run"

// ---------------------------------------------------------------------------
// Bounded field primitives
// ---------------------------------------------------------------------------

/** A bounded structured ref/id (run ids, route ids, user/team/project ids). */
export const AgentRunRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
export type AgentRunRef = typeof AgentRunRef.Type

/** ISO-8601 UTC timestamp string (same shape the wire contracts use). */
export const AgentRunIsoTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)
export type AgentRunIsoTimestamp = typeof AgentRunIsoTimestamp.Type

/** Bounded free text: the user's goal/objective. Content field, not a ref. */
export const AgentRunGoalText = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(10_000),
)
export type AgentRunGoalText = typeof AgentRunGoalText.Type

/** Bounded repository owner/name text. Content field, not a ref. */
export const AgentRunRepositoryText = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(512),
)
export type AgentRunRepositoryText = typeof AgentRunRepositoryText.Type

export const AgentRunStatus = S.Literals([
  "queued",
  "running",
  "waiting_for_input",
  "completed",
  "failed",
  "canceled",
])
export type AgentRunStatus = typeof AgentRunStatus.Type

export const AgentRunRuntime = S.Literals(["opencode_codex", "codex"])
export type AgentRunRuntime = typeof AgentRunRuntime.Type

export const AgentRunBackend = S.Literals(["shc_vm", "gcloud_vm"])
export type AgentRunBackend = typeof AgentRunBackend.Type

/**
 * Mirrors `AgentGoalStatus` in `apps/openagents.com/workers/api/src/
 * agent-goals.ts` exactly (six literals; note "complete", not "completed" —
 * archival is a separate `archivedAt` timestamp, not a status value).
 */
export const AgentGoalStatus = S.Literals([
  "active",
  "paused",
  "blocked",
  "usage_limited",
  "budget_limited",
  "complete",
])
export type AgentGoalStatus = typeof AgentGoalStatus.Type

export const AgentGoalVisibility = S.Literals(["private", "team", "public"])
export type AgentGoalVisibility = typeof AgentGoalVisibility.Type

const nonNegativeInt = S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0))

// ---------------------------------------------------------------------------
// Nested shapes
// ---------------------------------------------------------------------------

export class AgentRunRepositoryEntity extends S.Class<AgentRunRepositoryEntity>(
  "AgentRunRepositoryEntity",
)({
  provider: S.Literal("github"),
  owner: AgentRunRepositoryText,
  repo: AgentRunRepositoryText,
  ref: AgentRunRepositoryText,
}) {}

/**
 * The public-safe subset of the goal attached to this run at launch/
 * continuation time — mirrors `publicGoalContext` in `omni-runs.ts`
 * (excludes `hiddenSteering` and the tool contract, which stay private/
 * static and are not needed for a live scope update).
 */
export class AgentRunGoalContextEntity extends S.Class<AgentRunGoalContextEntity>(
  "AgentRunGoalContextEntity",
)({
  goalId: S.NullOr(AgentRunRef),
  objective: AgentRunGoalText,
  // Nullable: `AgentGoalAssignmentContext.status` upstream is `S.NullOr(S.String)`
  // (loose), even though every current caller passes a real `AgentGoalStatus`.
  status: S.NullOr(AgentGoalStatus),
  visibility: AgentGoalVisibility,
  tokenBudget: S.NullOr(nonNegativeInt),
  tokensUsed: nonNegativeInt,
  timeUsedSeconds: nonNegativeInt,
  remainingTokens: S.NullOr(nonNegativeInt),
}) {}

// ---------------------------------------------------------------------------
// agent_run (`scope.agent_run.<runId>`)
// ---------------------------------------------------------------------------

export class AgentRunEntity extends S.Class<AgentRunEntity>("AgentRunEntity")({
  runId: AgentRunRef,
  routeId: AgentRunRef,
  userId: AgentRunRef,
  teamId: S.NullOr(AgentRunRef),
  projectId: S.NullOr(AgentRunRef),
  runtime: AgentRunRuntime,
  backend: AgentRunBackend,
  status: AgentRunStatus,
  goalId: S.NullOr(AgentRunRef),
  goal: AgentRunGoalText,
  repository: AgentRunRepositoryEntity,
  goalContext: S.optionalKey(AgentRunGoalContextEntity),
  createdAt: AgentRunIsoTimestamp,
  updatedAt: AgentRunIsoTimestamp,
  startedAt: S.NullOr(AgentRunIsoTimestamp),
  completedAt: S.NullOr(AgentRunIsoTimestamp),
  failedAt: S.NullOr(AgentRunIsoTimestamp),
  canceledAt: S.NullOr(AgentRunIsoTimestamp),
}) {}

// ---------------------------------------------------------------------------
// Boundary codecs
// ---------------------------------------------------------------------------

export const decodeAgentRunEntity = S.decodeUnknownSync(AgentRunEntity)
export const encodeAgentRunEntity = S.encodeSync(AgentRunEntity)
