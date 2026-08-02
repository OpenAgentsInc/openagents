/**
 * Framed stdio protocol between Omega Rust supervisor and omega-effectd.
 *
 * Schema: `openagents.omega.effectd.v1`
 * Transport: one JSON object per line on stdin/stdout.
 * Generation fencing: every request carries the supervisor generation;
 * stale generations are refused.
 */

import type {
  ProtocolInitializeRequest as AllWorkProtocolInitializeRequest,
  ProtocolInitializeResult as AllWorkProtocolInitializeResult,
} from "@openagentsinc/all-work-contract";

export const OMEGA_EFFECTD_PROTOCOL_SCHEMA = "openagents.omega.effectd.v1" as const;
export const OMEGA_EFFECTD_SERVICE_VERSION = "0.1.0" as const;
export const OMEGA_EFFECTD_PROTOCOL_VERSION = 1 as const;
export const OMEGA_EFFECTD_MAX_FRAME_BYTES = 64 * 1024;
export const OMEGA_EFFECTD_MAX_HOST_REQUESTS = 32;

export type OmegaEffectdProtocolErrorCode =
  | "stale_generation"
  | "unknown_method"
  | "invalid_request"
  | "not_running"
  | "run_not_found"
  | "host_unavailable"
  | "host_timeout"
  | "frame_too_large"
  | "incompatible_version"
  | "not_found"
  | "unavailable"
  | "stale_cursor"
  | "gap"
  | "internal";

export type OmegaEffectdProtocolError = Readonly<{
  code: OmegaEffectdProtocolErrorCode;
  message: string;
}>;

export type OmegaEffectdInitializeParams = Readonly<{
  generation: number;
  client?: string;
  allWork?: AllWorkProtocolInitializeRequest;
}>;

export type OmegaEffectdInitializeResult = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA;
  protocolVersion: typeof OMEGA_EFFECTD_PROTOCOL_VERSION;
  serviceVersion: typeof OMEGA_EFFECTD_SERVICE_VERSION;
  generation: number;
  capabilities: ReadonlyArray<
    | "health"
    | "list_runs"
    | "get_run"
    | "start"
    | "pause"
    | "resume"
    | "handoff"
    | "stop"
    | "retry"
    | "get_capacity"
    | "decide_attention"
    | "get_report"
    | "get_receipt"
    | "apply_control_intent"
    | "get_sync_status"
    | "publish_projection"
    | "get_native_binding"
    | "assess_native_boundary"
    | "start_agent_computer_session"
    | "refresh_agent_computer_session"
    | "run_agent_computer_turn"
    | "get_agent_computer_session"
    | "list_agent_computer_sessions"
    | "sarah_session_status"
    | "sarah_bootstrap"
    | "sarah_room_snapshot"
    | "sarah_send_message"
    | "sarah_interrupt_turn"
    | "sarah_renew_device_grant"
    | "sarah_revoke_device_grant"
    | "work.index.read"
    | "work.snapshot.read"
    | "host_bridge"
  >;
  allWork: AllWorkProtocolInitializeResult;
  dataRoot: string;
  activeRunLimit: number;
}>;

/** FA-06: native project/worktree join (public-safe digests only). */
export type OmegaEffectdNativeEvidence = Readonly<{
  projectRef: string;
  worktreeRef: string;
  worktreePathDigest: string | null;
  gitHead: string | null;
}>;

export type OmegaEffectdNativeBinding = Readonly<{
  runRef: string;
  workspaceRef: string;
  projectRef: string;
  worktreeRef: string;
  worktreePathDigest: string | null;
  gitHead: string | null;
  rebaseUnsafe: boolean;
  boundAt: string;
}>;

/** FA-05: honest Sync availability (Omega OA-04 session may be absent). */
export type OmegaEffectdSyncStatus = Readonly<{
  available: boolean;
  publishBlocksDispatch: false;
  reason: string;
}>;

export type OmegaEffectdPublishProjectionResult = Readonly<{
  ok: boolean;
  status: "published" | "sync_unavailable" | "run_not_found";
  reason?: string;
}>;
/** Per-lane capacity ledger (FA-04). Public-safe: lane refs and typed states only. */
export type OmegaEffectdLaneCapacity = Readonly<{
  lane: string;
  state: "available" | "busy" | "cooling" | "exhausted" | "unavailable";
  activeRuns: number;
  reason: string;
}>;

export type OmegaEffectdCapacityResult = Readonly<{
  activeRunLimit: number;
  activeRunCount: number;
  lanes: ReadonlyArray<OmegaEffectdLaneCapacity>;
  nonOverridableGuardrails: ReadonlyArray<string>;
  ownerConfigurableGuardrails: ReadonlyArray<string>;
  enabledThreadsNeverEvicted: true;
}>;

/** Redacted attention decision (FA-04). Never carries objective/transcript. */
export type OmegaEffectdAttentionResult = Readonly<{
  notify: boolean;
  dedupKey: string;
  title: string;
  body: string;
} | null>;

/** Redacted durable run projection for list/monitor (no objective/transcript). */
export type OmegaEffectdRunSnapshot = Readonly<{
  runRef: string;
  threadRef: string | null;
  state: string;
  title: string;
  /**
   * Formatted for display. `HH:MM:SS`-style elapsed text and any other
   * duration a viewer shows is derived from this at the surface, so it is NOT
   * a measurement anything downstream may re-parse. Use `startedAtMs`.
   */
  updatedAt: string;
  /**
   * OMEGA-MOB-31-03 (omega#47): this host's numeric record of when the run
   * began, in epoch milliseconds, or `null` when the host never recorded one.
   * Paired with a `generatedAtMs` read from the same host clock, it yields the
   * exact unattended duration by measurement rather than by parsing
   * `updatedAt`.
   */
  startedAtMs: number | null;
}>;

/** Owner-local run detail for the GPUI launcher/monitor (FA-03). */
export type OmegaEffectdRunDetail = Readonly<{
  runRef: string;
  threadRef: string | null;
  state: string;
  title: string;
  objective: string;
  doneCondition: string;
  workspaceRef: string | null;
  lane: string | null;
  turnCap: number;
  successfulAttempts: number;
  failedAttempts: number;
  stallCause: string | null;
  recoveryAction: string;
  terminalReason: string | null;
  /**
   * OMEGA-MOB-31-03 (omega#47): the TYPED reason this run is over, as a bounded
   * public-safe ref built from the run's terminal state and the actor of the
   * transition that ended it -- e.g. `terminal.full_auto.completed.control_api`.
   * Null while the run is live, and null for a terminal run whose history does
   * not name that edge; the mobile projection REFUSES such a run rather than
   * showing an invented reason. `terminalReason` beside it stays free text for
   * a human to read, and nothing downstream parses it to classify the ending.
   */
  terminalReasonRef: string | null;
  /** Formatted for display; see `OmegaEffectdRunSnapshot.updatedAt`. */
  updatedAt: string;
  /**
   * OMEGA-MOB-31-03 (omega#47): this host's numeric record of when the run
   * began, in epoch milliseconds, or `null` when the host never recorded one.
   * The mobile Full Auto adjunct REFUSES a run whose start is `null` rather
   * than projecting `unattendedMs: 0`, which on a phone reads as "just
   * started" -- a claim nothing supports.
   */
  startedAtMs: number | null;
  nativeEvidence: OmegaEffectdNativeEvidence | null;
  turns: ReadonlyArray<{
    turnRef: string;
    lane: string;
    outcomeSummary: string;
    createdAt: string;
  }>;
}>;

export type OmegaEffectdStartParams = Readonly<{
  workspaceRef: string;
  title: string;
  objective: string;
  doneCondition: string;
  lane?: string;
  model?: string;
  turnCap?: number;
  autonomy?: boolean;
  projectRef?: string;
  worktreeRef?: string;
  worktreeAbsolutePath?: string;
  gitHead?: string;
  rebaseUnsafe?: boolean;
}>;

/** AC-01: public-safe Agent Computer session projection (no bearer/objective). */
export type OmegaEffectdAgentComputerSession = Readonly<{
  sessionRef: string;
  environment: "openagents_cloud";
  controlPlaneBaseUrl: string;
  repoRef: string;
  objectiveDigest: string;
  state: string;
  adapter: string | null;
  lane: string | null;
  placementRef: string | null;
  artifactRef: string | null;
  agentComputerRef: string | null;
  agentComputerState: string | null;
  startedAt: string;
  updatedAt: string;
}>;

export type OmegaEffectdAgentComputerTurnResult = Readonly<{
  session: OmegaEffectdAgentComputerSession;
  finishReason: string;
  eventKinds: ReadonlyArray<string>;
}>;

export type OmegaEffectdHealthResult = Readonly<{
  ok: true;
  status: "running" | "stopped";
  generation: number;
  dataRoot: string;
  activeRunCount: number;
}>;

export type OmegaEffectdRequest = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA;
  kind: "request";
  id: string;
  generation: number;
  method: string;
  params?: unknown;
}>;

export type OmegaEffectdResponse = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA;
  kind: "response";
  id: string;
  generation: number;
  ok: boolean;
  result?: unknown;
  error?: OmegaEffectdProtocolError;
}>;

export type OmegaEffectdEvent = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA;
  kind: "event";
  generation: number;
  name: string;
  payload?: unknown;
}>;

export type OmegaEffectdHostMethod =
  | "resolve_workspace"
  | "resolve_sync_session"
  | "create_thread"
  | "lane_readiness"
  | "dispatch_turn"
  | "refresh_evidence"
  | "interrupt_turn"
  | "append_system_note"
  | import("./sarah-host-contract.ts").OmegaEffectdSarahHostMethod;

export type OmegaEffectdHostRequest = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA;
  kind: "host_request";
  id: string;
  generation: number;
  method: OmegaEffectdHostMethod;
  params: unknown;
}>;

export type OmegaEffectdHostResponse = Readonly<{
  schema: typeof OMEGA_EFFECTD_PROTOCOL_SCHEMA;
  kind: "host_response";
  id: string;
  generation: number;
  ok: boolean;
  result?: unknown;
  error?: Readonly<{
    code: "stale_generation" | "invalid_request" | "unsupported" | "unavailable" | "internal";
    message: string;
  }>;
}>;

export type OmegaEffectdFrame =
  | OmegaEffectdRequest
  | OmegaEffectdResponse
  | OmegaEffectdEvent
  | OmegaEffectdHostRequest
  | OmegaEffectdHostResponse;

const isBoundedFrameIdentity = (value: unknown): value is string =>
  typeof value === "string" && value.length > 0 && value.length <= 180;

const isGeneration = (value: unknown): value is number =>
  typeof value === "number" && Number.isSafeInteger(value) && value >= 0;

export const isOmegaEffectdRequest = (value: unknown): value is OmegaEffectdRequest => {
  if (value === null || typeof value !== "object") return false;
  const frame = value as Record<string, unknown>;
  return (
    frame.schema === OMEGA_EFFECTD_PROTOCOL_SCHEMA &&
    frame.kind === "request" &&
    isBoundedFrameIdentity(frame.id) &&
    isGeneration(frame.generation) &&
    isBoundedFrameIdentity(frame.method)
  );
};

export const isOmegaEffectdHostResponse = (value: unknown): value is OmegaEffectdHostResponse => {
  if (value === null || typeof value !== "object") return false;
  const frame = value as Record<string, unknown>;
  if (
    frame.schema !== OMEGA_EFFECTD_PROTOCOL_SCHEMA ||
    frame.kind !== "host_response" ||
    !isBoundedFrameIdentity(frame.id) ||
    !isGeneration(frame.generation) ||
    typeof frame.ok !== "boolean"
  )
    return false;
  if (frame.ok) return "result" in frame;
  if (frame.error === null || typeof frame.error !== "object") return false;
  const error = frame.error as Record<string, unknown>;
  return (
    isBoundedFrameIdentity(error.code) &&
    typeof error.message === "string" &&
    error.message.length <= 1_024
  );
};

export const redactDiagnosticText = (text: string): string =>
  text
    .replace(/Bearer\s+[A-Za-z0-9._-]+/gi, "Bearer [redacted]")
    .replace(/\b(?:sk|xox[baprs]|gh[pousr])-[A-Za-z0-9_-]{16,}\b/gi, "[redacted-token]")
    .replace(/\/Users\/[^/\s]+/g, "/Users/[redacted]")
    .replace(/\/home\/[^/\s]+/g, "/home/[redacted]")
    .replace(/[A-Za-z0-9+/]{32,}={0,2}/g, "[redacted-token]");
