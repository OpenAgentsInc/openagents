// CL-53: the Foldkit (Effect TEA) Model for the Autopilot Desktop webview.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/page/demo/model.ts):
// a tagged-struct Model built with foldkit/schema `ts`, holding opaque externally
// supplied projections (the live node-state + notification-center views) as
// `S.Unknown`. Those projections are produced and validated on the Bun side and
// pushed in over the typed RPC; the webview treats them as read-only data, so
// `S.Unknown` (decoded back to the TS row types via typed accessors) matches the
// web app's treatment of externally-owned payloads.

import { Schema as S } from "effect"
import { ts } from "foldkit/schema"

import type { NotificationCenterView } from "@openagentsinc/autopilot-control-protocol"
import type { NodeStateMessage } from "../shared/rpc"

// Which content pane is showing. The desktop equivalent of mobile's tab set
// plus the focused session-detail leaf.
export const PaneId = S.Literals([
  "nodes",
  "sessions",
  "decisions",
  "spawn",
  "settings",
  "session-detail",
])
export type PaneId = typeof PaneId.Type

// The Sessions-pane state filter ("all" is the catch-all bucket).
export const SessionFilter = S.Literals([
  "all",
  "running",
  "queued",
  "completed",
  "failed",
  "cancelled",
])
export type SessionFilter = typeof SessionFilter.Type

// Transient status for the Spawn form (validation/submit feedback). Kept in the
// Model so the view stays a pure function of state (no hidden DOM).
export const SpawnStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "idle"]),
})
export type SpawnStatus = typeof SpawnStatus.Type

// Transient status for the Ask-Autopilot card.
export const AskStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type AskStatus = typeof AskStatus.Type

// Transient status for the Deploy card.
export const DeployFeedback = S.Struct({
  state: S.Literals(["queued", "building", "deployed", "failed", "unknown"]),
  text: S.String,
})
export type DeployFeedback = typeof DeployFeedback.Type

export const Model = ts("AutopilotDesktop", {
  // Latest projections (None until the first poll lands). Stored opaque; read
  // through the typed accessors below.
  node: S.NullOr(S.Unknown),
  notifications: S.NullOr(S.Unknown),

  // Navigation.
  pane: PaneId,
  selectedSessionRef: S.NullOr(S.String),

  // Sessions-pane filter.
  sessionFilter: SessionFilter,

  // Session-detail: which event indices are expanded (click-to-expand).
  expandedEvents: S.Array(S.Number),

  // Approvals optimistically resolved this session (hidden until the next poll
  // confirms). Keyed by approvalRef.
  resolvedApprovals: S.Array(S.String),

  // Spawn form fields + transient status.
  spawnAdapter: S.Literals(["codex", "claude_agent"]),
  spawnObjective: S.String,
  spawnVerify: S.String,
  spawnStatus: SpawnStatus,
  spawnPending: S.Boolean,

  // Ask-Autopilot form fields + transient status.
  askTitle: S.String,
  askBody: S.String,
  askStatus: AskStatus,
  askPending: S.Boolean,

  // Deploy feedback (null until the card has been interacted with or a deploy
  // projection has landed).
  deployFeedback: S.NullOr(DeployFeedback),
})
export type Model = typeof Model.Type

// ── Typed accessors over the opaque projection fields ──────────────────────
//
// The Model carries `node`/`notifications` as `S.Unknown` so makeProgram's
// Schema.Codec stays happy with externally-owned payloads; these helpers re-narrow
// them at the single read boundary so the rest of the app is typed.

export const modelNode = (model: Model): NodeStateMessage | null =>
  model.node as NodeStateMessage | null

export const modelNotifications = (model: Model): NotificationCenterView | null =>
  model.notifications as NotificationCenterView | null

export const initialModel: Model = Model.make({
  node: null,
  notifications: null,
  pane: "nodes",
  selectedSessionRef: null,
  sessionFilter: "all",
  expandedEvents: [],
  resolvedApprovals: [],
  spawnAdapter: "codex",
  spawnObjective: "",
  spawnVerify: "",
  spawnStatus: { text: "", tone: "idle" },
  spawnPending: false,
  askTitle: "",
  askBody: "",
  askStatus: { text: "", tone: "idle" },
  askPending: false,
  deployFeedback: null,
})
