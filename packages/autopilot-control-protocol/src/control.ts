import { Schema as S } from "effect"

// The shared Autopilot control protocol — the typed vocabulary every client
// (web / desktop / mobile) speaks to a Pylon node. This is the package-level
// home for what the Pylon control server emits today; the wire schema tag stays
// `openagents.pylon.control.v0.3` (Pylon is the internal node/runtime name).

export const CONTROL_SCHEMA_TAG = "openagents.pylon.control.v0.3" as const

export const Adapter = S.Literals(["codex", "claude_agent"])
export type Adapter = typeof Adapter.Type

export const SessionState = S.Literals([
  "queued",
  "running",
  "completed",
  "failed",
  "cancelled",
])
export type SessionState = typeof SessionState.Type

// Projection levels gate how much detail a client may see. Public-safe is the
// default for remote clients; private requires an explicit per-run grant.
export const ProjectionLevel = S.Literals(["public_safe", "team", "private"])
export type ProjectionLevel = typeof ProjectionLevel.Type

// A public-safe session row. Identity is refs only — no raw paths/prompts.
export const SessionSummary = S.Struct({
  sessionRef: S.String,
  adapter: Adapter,
  state: SessionState,
  objectiveRef: S.optional(S.String),
  workspaceRef: S.optional(S.String),
  accountRefHash: S.NullOr(S.String),
  lastProgressRef: S.optional(S.String),
  // One-line latest action (what the agent is doing now) for the session list.
  latestActivity: S.optional(S.String),
  // #4951 external/host agent sessions: nest children under their parent + badge.
  parentRef: S.optional(S.String),
  agentKind: S.optional(S.String),
  pylonManaged: S.optional(S.Boolean),
  updatedAt: S.String,
})
export type SessionSummary = typeof SessionSummary.Type

// One ordered event on a session stream. `sequence` is the resume cursor;
// `eventId` is the duplicate-detection key.
export const SessionEventPhase = S.Literals([
  "started",
  "progress",
  "decision_requested",
  "decision_resolved",
  "decision_cancelled",
  "artifact_available",
  "completed",
  "failed",
  "cancelled",
])
export type SessionEventPhase = typeof SessionEventPhase.Type

export const SessionEvent = S.Struct({
  schema: S.Literal(CONTROL_SCHEMA_TAG),
  sessionRef: S.String,
  eventId: S.String,
  sequence: S.Number,
  phase: SessionEventPhase,
  projectionLevel: ProjectionLevel,
  observedAt: S.String,
  detailRef: S.optional(S.String),
})
export type SessionEvent = typeof SessionEvent.Type

// Control commands the node accepts today (POST /command). Bounded session
// spawn rejects danger modes server-side; this is the typed request shape.
export const SpawnCommand = S.Struct({
  type: S.Literal("session.spawn"),
  adapter: Adapter,
  objective: S.String,
  verify: S.Array(S.String),
  worktreePath: S.optional(S.String),
  timeoutSeconds: S.optional(S.Number),
})
export const ListCommand = S.Struct({ type: S.Literal("session.list") })
export const EventsCommand = S.Struct({
  type: S.Literal("session.events"),
  sessionRef: S.String,
})
export const CancelCommand = S.Struct({
  type: S.Literal("session.cancel"),
  sessionRef: S.String,
})

export const ControlCommand = S.Union([
  SpawnCommand,
  ListCommand,
  EventsCommand,
  CancelCommand,
])
export type ControlCommand = typeof ControlCommand.Type

export const HealthResponse = S.Struct({
  ok: S.Boolean,
  schema: S.String,
})
export type HealthResponse = typeof HealthResponse.Type

export const decodeSessionEvent = S.decodeUnknownSync(SessionEvent)
export const decodeSessionSummary = S.decodeUnknownSync(SessionSummary)
export const decodeControlCommand = S.decodeUnknownSync(ControlCommand)
