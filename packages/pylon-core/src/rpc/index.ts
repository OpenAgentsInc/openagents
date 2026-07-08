/**
 * Typed RPC contract (PY-1 #8578 step 6) — the Effect-Schema request/reply
 * envelope that the engine daemon and its clients (Khala Code desktop cockpit,
 * the `pylon`/`khala` CLIs) will speak end to end.
 *
 * Purpose: replace the desktop's current stringly seam in
 * `clients/khala-code-desktop/src/bun/pylon-service.ts`, which spawns the
 * Pylon CLI as a subprocess and parses lifecycle/result JSON out of **stdout**
 * (`request({ args: string[] }) -> { stdout, stderr, exitCode }`). With this
 * contract a client sends a typed `PylonRpcRequest` and receives a typed
 * `PylonRpcReply` (plus a typed `PylonRpcLifecycleFrame` stream) — no stdout
 * parsing anywhere.
 *
 * STATUS: **unconsumed seed.** This module is intentionally not wired into the
 * daemon or the desktop yet — deleting the stdout seam is PY-2's job
 * (#8579). Landing the contract alone (typed, tested, unconsumed) is a valid,
 * safe stopping point per the proposal (§5). The operation set is grounded in
 * the real surface `pylon-service.ts` exercises today (assignment runs,
 * presence heartbeat, khala closeout) plus the custody reads that
 * `pylon-core` can already back (list accounts, account health).
 *
 * Style mirrors the sibling `@openagentsinc/agent-runtime-schema`; the
 * streamed lifecycle event is REUSED from there rather than redefined.
 */

import { Schema as S } from "effect"
import { PylonAssignmentRunLifecycleEvent } from "@openagentsinc/agent-runtime-schema"

// --- versioning ------------------------------------------------------------

/** Schema literal stamped on every envelope for forward-compatible decoding. */
export const PylonRpcSchemaLiteral = "openagents.pylon.rpc.v1" as const

// --- shared field vocabulary ----------------------------------------------

export const PylonRpcWorkerKind = S.Literals(["auto", "claude", "codex"])
export type PylonRpcWorkerKind = typeof PylonRpcWorkerKind.Type

export const PylonAssignmentRunStatus = S.Literals(["accepted", "blocked", "failed", "completed"])
export type PylonAssignmentRunStatus = typeof PylonAssignmentRunStatus.Type

export const PylonAccountProvider = S.Literals(["codex", "claude_agent"])
export type PylonAccountProvider = typeof PylonAccountProvider.Type

/** Public-safe account summary (hashed ref only — never a raw account path). */
export const PylonAccountSummary = S.Struct({
  accountRefHash: S.String,
  provider: PylonAccountProvider,
  label: S.optional(S.String),
  available: S.Boolean,
  blockerRefs: S.optional(S.Array(S.String)),
})
export type PylonAccountSummary = typeof PylonAccountSummary.Type

export const PylonAccountHealth = S.Struct({
  accountRefHash: S.String,
  provider: PylonAccountProvider,
  healthy: S.Boolean,
  reason: S.optional(S.String),
  blockerRefs: S.optional(S.Array(S.String)),
  lastCheckedAt: S.optional(S.String),
})
export type PylonAccountHealth = typeof PylonAccountHealth.Type

// --- requests --------------------------------------------------------------

/** List the custody registry's connected accounts and their capacity. */
export const PylonListAccountsRequest = S.Struct({
  _tag: S.Literal("ListAccounts"),
})
export type PylonListAccountsRequest = typeof PylonListAccountsRequest.Type

/** Read one account's health (from the codex account health ledger). */
export const PylonGetAccountHealthRequest = S.Struct({
  _tag: S.Literal("GetAccountHealth"),
  accountRefHash: S.String,
})
export type PylonGetAccountHealthRequest = typeof PylonGetAccountHealthRequest.Type

/** Publish a presence heartbeat — the "go online / stay online" beat. */
export const PylonPresenceHeartbeatRequest = S.Struct({
  _tag: S.Literal("PresenceHeartbeat"),
  baseUrl: S.optional(S.String),
})
export type PylonPresenceHeartbeatRequest = typeof PylonPresenceHeartbeatRequest.Type

/** Submit a closeout for a completed assignment. */
export const PylonKhalaCloseoutRequest = S.Struct({
  _tag: S.Literal("KhalaCloseout"),
  assignmentRef: S.String,
  baseUrl: S.optional(S.String),
})
export type PylonKhalaCloseoutRequest = typeof PylonKhalaCloseoutRequest.Type

/**
 * Run a local coding-delegation assignment. Mirrors the desktop's
 * `PylonServiceAssignmentInput`. Lifecycle progress is delivered out of band
 * as `PylonRpcLifecycleFrame`s correlated by the envelope id; the terminal
 * `PylonRunAssignmentResult` arrives in the reply.
 */
export const PylonRunAssignmentRequest = S.Struct({
  _tag: S.Literal("RunAssignment"),
  objective: S.String,
  accountRef: S.optional(S.String),
  baseUrl: S.optional(S.String),
  branch: S.optional(S.String),
  commit: S.optional(S.String),
  fixture: S.optional(S.Boolean),
  pylonRef: S.optional(S.String),
  repo: S.optional(S.String),
  timeoutMs: S.optional(S.Number),
  verify: S.optional(S.String),
  workerKind: S.optional(PylonRpcWorkerKind),
})
export type PylonRunAssignmentRequest = typeof PylonRunAssignmentRequest.Type

export const PylonRpcRequest = S.Union([
  PylonListAccountsRequest,
  PylonGetAccountHealthRequest,
  PylonPresenceHeartbeatRequest,
  PylonKhalaCloseoutRequest,
  PylonRunAssignmentRequest,
])
export type PylonRpcRequest = typeof PylonRpcRequest.Type

// --- replies ---------------------------------------------------------------

export const PylonListAccountsReply = S.Struct({
  _tag: S.Literal("ListAccounts"),
  accounts: S.Array(PylonAccountSummary),
})
export type PylonListAccountsReply = typeof PylonListAccountsReply.Type

export const PylonGetAccountHealthReply = S.Struct({
  _tag: S.Literal("GetAccountHealth"),
  health: PylonAccountHealth,
})
export type PylonGetAccountHealthReply = typeof PylonGetAccountHealthReply.Type

export const PylonPresenceHeartbeatReply = S.Struct({
  _tag: S.Literal("PresenceHeartbeat"),
  acknowledgedAt: S.String,
  onlineCapacityRefs: S.optional(S.Array(S.String)),
})
export type PylonPresenceHeartbeatReply = typeof PylonPresenceHeartbeatReply.Type

export const PylonKhalaCloseoutReply = S.Struct({
  _tag: S.Literal("KhalaCloseout"),
  closeoutRef: S.optional(S.String),
  status: S.optional(S.String),
})
export type PylonKhalaCloseoutReply = typeof PylonKhalaCloseoutReply.Type

/** Terminal result of a RunAssignment (mirrors PylonServiceAssignmentResult). */
export const PylonRunAssignmentResult = S.Struct({
  assignmentRef: S.optional(S.String),
  status: PylonAssignmentRunStatus,
  summary: S.String,
  lifecycle: S.Array(PylonAssignmentRunLifecycleEvent),
})
export type PylonRunAssignmentResult = typeof PylonRunAssignmentResult.Type

export const PylonRunAssignmentReply = S.Struct({
  _tag: S.Literal("RunAssignment"),
  result: PylonRunAssignmentResult,
})
export type PylonRunAssignmentReply = typeof PylonRunAssignmentReply.Type

/** Typed failure reply — replaces reading a non-zero exit + stderr text. */
export const PylonRpcErrorReply = S.Struct({
  _tag: S.Literal("Error"),
  code: S.Literals([
    "bad_request",
    "not_found",
    "account_unavailable",
    "timed_out",
    "internal",
  ]),
  message: S.String,
  blockerRefs: S.optional(S.Array(S.String)),
})
export type PylonRpcErrorReply = typeof PylonRpcErrorReply.Type

export const PylonRpcReply = S.Union([
  PylonListAccountsReply,
  PylonGetAccountHealthReply,
  PylonPresenceHeartbeatReply,
  PylonKhalaCloseoutReply,
  PylonRunAssignmentReply,
  PylonRpcErrorReply,
])
export type PylonRpcReply = typeof PylonRpcReply.Type

// --- envelopes (correlation) ----------------------------------------------

/** Client -> daemon. `id` correlates the reply and any lifecycle frames. */
export const PylonRpcRequestEnvelope = S.Struct({
  schema: S.Literal(PylonRpcSchemaLiteral),
  id: S.String,
  request: PylonRpcRequest,
})
export type PylonRpcRequestEnvelope = typeof PylonRpcRequestEnvelope.Type

/** Daemon -> client, terminal reply for `id`. */
export const PylonRpcReplyEnvelope = S.Struct({
  schema: S.Literal(PylonRpcSchemaLiteral),
  id: S.String,
  reply: PylonRpcReply,
})
export type PylonRpcReplyEnvelope = typeof PylonRpcReplyEnvelope.Type

/**
 * Daemon -> client streamed progress for `id`, carrying the SAME typed
 * lifecycle event the desktop today scrapes out of stdout lines.
 */
export const PylonRpcLifecycleFrame = S.Struct({
  schema: S.Literal(PylonRpcSchemaLiteral),
  id: S.String,
  event: PylonAssignmentRunLifecycleEvent,
})
export type PylonRpcLifecycleFrame = typeof PylonRpcLifecycleFrame.Type

// --- codecs (mirror agent-runtime-schema helpers) --------------------------

export const decodePylonRpcRequest = S.decodeUnknownSync(PylonRpcRequest)
export const decodePylonRpcReply = S.decodeUnknownSync(PylonRpcReply)
export const decodePylonRpcRequestEnvelope = S.decodeUnknownSync(PylonRpcRequestEnvelope)
export const decodePylonRpcReplyEnvelope = S.decodeUnknownSync(PylonRpcReplyEnvelope)
export const decodePylonRpcLifecycleFrame = S.decodeUnknownSync(PylonRpcLifecycleFrame)

export const encodePylonRpcRequestEnvelope = S.encodeUnknownSync(PylonRpcRequestEnvelope)
export const encodePylonRpcReplyEnvelope = S.encodeUnknownSync(PylonRpcReplyEnvelope)
export const encodePylonRpcLifecycleFrame = S.encodeUnknownSync(PylonRpcLifecycleFrame)

export const PylonRpcRequestEnvelopeFromJsonString = S.fromJsonString(PylonRpcRequestEnvelope)
export const PylonRpcReplyEnvelopeFromJsonString = S.fromJsonString(PylonRpcReplyEnvelope)
export const PylonRpcLifecycleFrameFromJsonString = S.fromJsonString(PylonRpcLifecycleFrame)

export const decodePylonRpcRequestEnvelopeJson = S.decodeUnknownSync(PylonRpcRequestEnvelopeFromJsonString)
export const decodePylonRpcReplyEnvelopeJson = S.decodeUnknownSync(PylonRpcReplyEnvelopeFromJsonString)
export const decodePylonRpcLifecycleFrameJson = S.decodeUnknownSync(PylonRpcLifecycleFrameFromJsonString)
