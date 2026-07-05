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

// ---------------------------------------------------------------------------
// agent_run_event (`scope.agent_run.<runId>`, companion multi-entity —
// KS-6.6 event-feed follow-up, #8416)
// ---------------------------------------------------------------------------

/**
 * Companion entity to {@link AgentRunEntity} closing the "schema gap" from
 * the 2026-07-05 client-repoint research (RUNBOOK.md, KS-6.6): the legacy
 * `agent-run:<runId>` DO-room scope multiplexes TWO D1 collections onto one
 * room — `agent_runs` (what `AgentRunEntity` mirrors) AND `agent_run_events`
 * (the individual tool-call/message events that populate `chatRun.events`,
 * i.e. the live transcript). `AgentRunEntity` is deliberately a single
 * flattened run+goal post-image with no equivalent of the event collection;
 * this entity is the "one entity per SCOPE" rule's natural extension to
 * "many entities per scope" (same convention as
 * `scope.public.gym-run-progress`'s `runRef`-keyed rows, except here every
 * event entity rides the SAME `scope.agent_run.<runId>` scope as its parent
 * run entity, keyed by the event's own id — mirroring the legacy DO room,
 * which already multiplexes both collections onto one scope/room).
 *
 * The shape mirrors the ALREADY-public-safe `agentRunEventProjection`
 * (`apps/openagents.com/workers/api/src/omni-runs.ts`) — the exact fields
 * already shipped over the legacy sync-worker outbox to
 * `agentRunScope(run.id)` (and `threadScope(routeId)`) today for every
 * runner-posted event. This module invents no new public-safe surface.
 *
 * PUBLIC-SAFE BY CONSTRUCTION (SPEC §7 invariant 9): `summary` and
 * `payloadJson` are the only free-content fields (bounded, but otherwise
 * unconstrained, exactly like `goal` above) — `payloadJson` is additionally
 * scrubbed of credential-shaped material at WRITE time
 * (`omni-runs.ts`'s `jsonOrNull`/`containsProviderSecretMaterial`) before it
 * ever reaches D1, so this contract's redaction guard
 * (`packages/khala-sync-server/src/agent-run-projection.ts`) is defense in
 * depth, not the first line.
 */

export const AGENT_RUN_EVENT_ENTITY_TYPE = "agent_run_event"

/** Bounded free text: event summaries / tool-call labels. Content field. */
export const AgentRunEventSummaryText = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(20_000),
)
export type AgentRunEventSummaryText = typeof AgentRunEventSummaryText.Type

/**
 * Bounded free text: the already-scrubbed (at D1 write time) raw event
 * payload JSON string. Content field, not a ref.
 */
export const AgentRunEventPayloadText = S.String.check(S.isMaxLength(262_144))
export type AgentRunEventPayloadText = typeof AgentRunEventPayloadText.Type

/**
 * Bounded system-controlled label (event `type`/`source`/`status`) — not
 * free user content, but not a closed literal set either (runner/provider
 * callback vocabulary evolves independently of this contract).
 */
export const AgentRunEventToken = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
)
export type AgentRunEventToken = typeof AgentRunEventToken.Type

/** Bounded external-system event ref (runner/provider callback ids). */
export const AgentRunEventExternalRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
)
export type AgentRunEventExternalRef = typeof AgentRunEventExternalRef.Type

/** Bounded artifact ref (URL, R2 key, or repo-relative path). */
export const AgentRunEventArtifactRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(1024),
)
export type AgentRunEventArtifactRef = typeof AgentRunEventArtifactRef.Type

export class AgentRunEventEntity extends S.Class<AgentRunEventEntity>(
  "AgentRunEventEntity",
)({
  id: AgentRunRef,
  runId: AgentRunRef,
  sequence: nonNegativeInt,
  type: AgentRunEventToken,
  summary: AgentRunEventSummaryText,
  status: S.NullOr(AgentRunEventToken),
  source: AgentRunEventToken,
  payloadJson: S.NullOr(AgentRunEventPayloadText),
  artifactRefs: S.Array(AgentRunEventArtifactRef),
  externalEventId: S.NullOr(AgentRunEventExternalRef),
  createdAt: AgentRunIsoTimestamp,
}) {}

export const decodeAgentRunEventEntity = S.decodeUnknownSync(AgentRunEventEntity)
export const encodeAgentRunEventEntity = S.encodeSync(AgentRunEventEntity)
