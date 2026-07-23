import { Effect } from "effect";
import type {
  AgentDefinitionHarnessKind,
  AgentRuntimeAdapterKind,
} from "@openagentsinc/agent-runtime-schema";
import type { AcpAdapterEvent } from "./acp-adapter.ts";
import { makeAcpHarnessAdapter } from "./acp-adapter.ts";
import type { AcpTransport } from "./acp-adapter.ts";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import type { HarnessBuiltinTool } from "./common-tool.ts";
import type { HarnessSession } from "./session.ts";

/**
 * Named Cursor harness adapter (HW-05).
 *
 * This is the NAMED, documented productization of the generic ACP factory
 * ({@link makeAcpHarnessAdapter}) for the Cursor coding agent. The raw factory
 * admits any ACP peer, but it says nothing about what a specific peer's ACP
 * surface actually supports. A named adapter pins that: a fixed Cursor peer
 * profile (protocol version, the harness/adapter kinds a Cursor peer declares,
 * the built-in tools it exposes), a PUBLISHED capability table (which session
 * verbs are lossless, degraded, or refused), executable-discovery guidance the
 * wiring layer uses to locate the `cursor-agent` binary, and its own hermetic
 * conformance fixtures.
 *
 * The binary path is INJECTED, never hard-coded here: the module ships
 * discovery guidance ({@link CURSOR_AGENT_DISCOVERY}) so a caller can resolve
 * the executable, and {@link makeCursorHarnessAdapter} requires the resolved
 * path as config and refuses to start a session without it.
 *
 * Cursor has no dedicated `AgentDefinitionHarnessKind`, so the pinned harness
 * kind is `custom`; its ACP identity is carried by the `cursor_cli` adapter
 * kind. Approvals do NOT ride the native `submitToolApproval` channel: a Cursor
 * `session/request_permission` projects onto the durable `RuntimeInteraction`
 * model, exactly as the ACP factory documents.
 */

// ---------------------------------------------------------------------------
// Pinned Cursor ACP capability profile
// ---------------------------------------------------------------------------

/**
 * The Agent Client Protocol major version this profile is pinned to. Cursor's
 * ACP surface negotiates protocol version 1; a peer that reports a different
 * major version is a profile mismatch the wiring layer must reconcile before
 * routing work.
 */
export const CURSOR_ACP_PROTOCOL_VERSION = 1 as const;

/** A capability of the Cursor ACP surface, and whether the profile relies on it. */
export interface CursorAcpCapability {
  /** Stable capability id. */
  readonly capability: string;
  /** Human/agent-readable note on what the Cursor ACP surface does here. */
  readonly note: string;
}

/**
 * ACP capabilities the pinned Cursor peer profile relies on. These mirror the
 * `session/update` notification vocabulary and request surface a Cursor ACP
 * peer offers: streamed assistant text and reasoning, tool-call lifecycle,
 * interactive permission requests, and session load for resume.
 */
export const CURSOR_SUPPORTED_ACP_CAPABILITIES: ReadonlyArray<CursorAcpCapability> = [
  {
    capability: "session.prompt",
    note: "session/prompt drives one turn and streams session/update notifications",
  },
  {
    capability: "update.agent_message",
    note: "assistant text arrives as agent_message_chunk deltas",
  },
  {
    capability: "update.agent_thought",
    note: "reasoning arrives as agent_thought_chunk deltas",
  },
  {
    capability: "update.tool_call",
    note: "tool_call / tool_call_update carry the peer's native tool lifecycle",
  },
  {
    capability: "session.request_permission",
    note: "interactive approvals; projected onto the durable RuntimeInteraction model",
  },
  {
    capability: "session.load",
    note: "a persisted session can be re-loaded to resume, re-driving the turn",
  },
  {
    capability: "session.cancel",
    note: "an in-flight turn can be cancelled",
  },
];

/**
 * ACP behaviors the Cursor peer profile explicitly does NOT rely on, each with
 * the reason. These are the honest refusals the named adapter publishes over
 * the raw factory: a consumer reads them instead of discovering the gap at
 * runtime.
 */
export const CURSOR_REFUSED_ACP_CAPABILITIES: ReadonlyArray<CursorAcpCapability> = [
  {
    capability: "context.compact",
    note: "ACP has no context-compaction primitive; the adapter refuses `compact`",
  },
  {
    capability: "builtin_tool_filtering",
    note: "the ACP surface cannot hide inactive built-ins; the framework emulates via auto-deny",
  },
  {
    capability: "native_tool_approval_channel",
    note: "approvals route through RuntimeInteraction, not the contract submitToolApproval channel",
  },
];

/**
 * The pinned Cursor peer profile: the fixed identity and negotiated ACP surface
 * every Cursor harness instance is built from.
 */
export interface CursorPeerProfile {
  readonly harnessId: string;
  readonly harnessKind: AgentDefinitionHarnessKind;
  readonly adapterKind: AgentRuntimeAdapterKind;
  readonly acpProtocolVersion: typeof CURSOR_ACP_PROTOCOL_VERSION;
  readonly supportedAcpCapabilities: ReadonlyArray<CursorAcpCapability>;
  readonly refusedAcpCapabilities: ReadonlyArray<CursorAcpCapability>;
}

/** The single pinned Cursor peer profile. */
export const CURSOR_PEER_PROFILE: CursorPeerProfile = {
  harnessId: "cursor",
  // Cursor has no dedicated AgentDefinitionHarnessKind; `custom` is the honest
  // kind, and the ACP identity is carried by the `cursor_cli` adapter kind.
  harnessKind: "custom",
  adapterKind: "cursor_cli",
  acpProtocolVersion: CURSOR_ACP_PROTOCOL_VERSION,
  supportedAcpCapabilities: CURSOR_SUPPORTED_ACP_CAPABILITIES,
  refusedAcpCapabilities: CURSOR_REFUSED_ACP_CAPABILITIES,
};

/**
 * Built-in tools the Cursor ACP surface exposes natively, with their common
 * vocabulary names where one exists. Cursor-specific tools (`codebase_search`)
 * have no common equivalent and keep their native name. The framework uses this
 * metadata for tool normalization and built-in filtering; the shared
 * `toolIdentity` map still owns the native->common projection at event time.
 */
export const CURSOR_BUILTIN_TOOLS: ReadonlyArray<HarnessBuiltinTool> = [
  { nativeName: "read_file", commonName: "read", description: "read a file" },
  { nativeName: "write_file", commonName: "write", description: "write a file" },
  { nativeName: "apply_patch", commonName: "edit", description: "apply an edit to a file" },
  { nativeName: "shell", commonName: "bash", description: "run a shell command" },
  { nativeName: "codebase_search", description: "semantic search across the codebase" },
];

// ---------------------------------------------------------------------------
// Executable discovery guidance (path is injected, never hard-coded)
// ---------------------------------------------------------------------------

/**
 * Guidance the wiring layer uses to LOCATE the Cursor agent executable. The
 * adapter never reads the environment or hard-codes a path itself: it publishes
 * where to look, and the caller injects the resolved path into
 * {@link CursorHarnessAdapterConfig.cursorAgentPath}.
 */
export interface CursorAgentDiscovery {
  /** Candidate executable names to resolve on PATH, in preference order. */
  readonly candidateBinaryNames: ReadonlyArray<string>;
  /** Environment variable a caller may consult for an explicit override path. */
  readonly overrideEnvVar: string;
  /** Public-safe hint printed when the executable cannot be resolved. */
  readonly installHint: string;
}

/** The published Cursor executable-discovery guidance. */
export const CURSOR_AGENT_DISCOVERY: CursorAgentDiscovery = {
  candidateBinaryNames: ["cursor-agent"],
  overrideEnvVar: "CURSOR_AGENT_PATH",
  installHint:
    "Install the Cursor CLI and ensure `cursor-agent` is on PATH, or set CURSOR_AGENT_PATH",
};

/**
 * True when the injected Cursor agent path is refused: empty/whitespace, or the
 * unresolved discovery placeholder. This proves the path is INJECTED — the
 * adapter has no built-in default to fall back to.
 */
export const isRefusedCursorAgentPath = (cursorAgentPath: string): boolean => {
  const trimmed = cursorAgentPath.trim();
  if (trimmed.length === 0) return true;
  return trimmed === "<inject-cursor-agent-path>";
};

// ---------------------------------------------------------------------------
// Published capability table
// ---------------------------------------------------------------------------

/**
 * How the Cursor adapter honors a session verb:
 *   - `lossless`: fully supported with exact fidelity.
 *   - `degraded`: supported, but with a documented fidelity loss.
 *   - `refused`: not supported; the verb fails with `HarnessCapabilityUnsupported`.
 */
export type CursorVerbDisposition = "lossless" | "degraded" | "refused";

/** One row of the published Cursor capability table. */
export interface CursorCapabilityEntry {
  /** The session verb / capability this row describes. */
  readonly verb: string;
  readonly disposition: CursorVerbDisposition;
  /** Why the verb has this disposition on Cursor's ACP surface. */
  readonly note: string;
}

/**
 * The PUBLISHED Cursor capability table — the point of a named adapter over the
 * raw ACP factory. It states, per session verb, exactly what a Cursor peer
 * supports losslessly, supports in a degraded form, or refuses. The factory
 * capability flags are DERIVED from this table (see
 * {@link cursorFactoryCapabilityFlags}) so the published table and the adapter's
 * runtime behavior can never drift.
 */
export const CURSOR_CAPABILITY_TABLE: ReadonlyArray<CursorCapabilityEntry> = [
  {
    verb: "prompt_turn",
    disposition: "lossless",
    note: "session/prompt streams deltas and tool calls, projected losslessly onto KhalaRuntimeEvent",
  },
  {
    verb: "suspend_turn",
    disposition: "lossless",
    note: "the cursor of the last pulled event is pinned exactly against the buffered projection",
  },
  {
    verb: "continue_turn",
    disposition: "degraded",
    note: "cross-process resume uses ACP session/load, which re-drives the turn; labeled lossy",
  },
  {
    verb: "detach",
    disposition: "lossless",
    note: "ACP session persistence lets a detached session be re-loaded without loss",
  },
  {
    verb: "compact",
    disposition: "refused",
    note: "ACP has no context-compaction primitive",
  },
  {
    verb: "builtin_tool_approvals",
    disposition: "refused",
    note: "approvals ride session/request_permission -> RuntimeInteraction, not the native channel",
  },
  {
    verb: "builtin_tool_filtering",
    disposition: "refused",
    note: "the ACP surface cannot hide inactive built-ins; the framework auto-denies them",
  },
];

/** The disposition recorded for one verb, or `undefined` when the table omits it. */
export const cursorVerbDisposition = (verb: string): CursorVerbDisposition | undefined =>
  CURSOR_CAPABILITY_TABLE.find((entry) => entry.verb === verb)?.disposition;

/** The factory capability flags that realize the published capability table. */
export interface CursorFactoryCapabilityFlags {
  readonly supportsSuspend: boolean;
  readonly supportsContinue: boolean;
  readonly continueIsLossy: boolean;
  readonly supportsCompact: boolean;
  readonly supportsDetach: boolean;
  readonly supportsBuiltinToolApprovals: boolean;
  readonly supportsBuiltinToolFiltering: boolean;
}

/**
 * Derive the ACP factory capability flags from the published capability table,
 * so a change to the table is the single source of truth for adapter behavior.
 * A `refused` verb turns its capability off; a `degraded` `continue_turn` sets
 * `continueIsLossy` so a suspend/continue reports the honest lossy label.
 */
export const cursorFactoryCapabilityFlags = (
  table: ReadonlyArray<CursorCapabilityEntry> = CURSOR_CAPABILITY_TABLE,
): CursorFactoryCapabilityFlags => {
  const disposition = (verb: string): CursorVerbDisposition | undefined =>
    table.find((entry) => entry.verb === verb)?.disposition;
  return {
    supportsSuspend: disposition("suspend_turn") !== "refused",
    supportsContinue: disposition("continue_turn") !== "refused",
    continueIsLossy: disposition("continue_turn") === "degraded",
    supportsCompact: disposition("compact") !== "refused",
    supportsDetach: disposition("detach") !== "refused",
    supportsBuiltinToolApprovals: disposition("builtin_tool_approvals") !== "refused",
    supportsBuiltinToolFiltering: disposition("builtin_tool_filtering") !== "refused",
  };
};

// ---------------------------------------------------------------------------
// Hermetic conformance fixtures (scripted ACP projection sequences)
// ---------------------------------------------------------------------------

/**
 * A representative Cursor turn: reasoning + text deltas, two correlated tool
 * calls (a common-vocabulary `read_file` and a Cursor-specific
 * `codebase_search`), and a clean stop. No live `cursor-agent` process is
 * involved — the sequence is replayed through the ACP factory's scripted seam.
 */
export const cursorAcpTurnScript: ReadonlyArray<AcpAdapterEvent> = [
  { type: "acp_turn_started" },
  { type: "acp_thought_delta", text: "Reviewing the request." },
  { type: "acp_text_delta", text: "Reading the file. " },
  { type: "acp_tool_call", toolCallId: "toolcall.cursor.1", toolName: "read_file" },
  { type: "acp_tool_result", toolCallId: "toolcall.cursor.1", toolName: "read_file", ok: true },
  { type: "acp_tool_call", toolCallId: "toolcall.cursor.2", toolName: "codebase_search" },
  {
    type: "acp_tool_result",
    toolCallId: "toolcall.cursor.2",
    toolName: "codebase_search",
    ok: true,
  },
  { type: "acp_text_delta", text: "Done." },
  { type: "acp_turn_stop", stopReason: "end_turn" },
];

/**
 * A Cursor turn that requests permission to run a shell command. The permission
 * request emits NO transcript event — it routes through the RuntimeInteraction
 * model — so the projected stream stays contiguous around it.
 */
export const cursorAcpApprovalTurnScript: ReadonlyArray<AcpAdapterEvent> = [
  { type: "acp_turn_started" },
  { type: "acp_tool_call", toolCallId: "toolcall.cursor.9", toolName: "shell" },
  { type: "acp_permission_request", toolCallId: "toolcall.cursor.9", toolName: "shell" },
  { type: "acp_tool_result", toolCallId: "toolcall.cursor.9", toolName: "shell", ok: true },
  { type: "acp_turn_stop", stopReason: "end_turn" },
];

/**
 * A Cursor turn where a tool fails and the turn stops on a refusal — the error
 * and finish-reason mapping fixture.
 */
export const cursorAcpFailureTurnScript: ReadonlyArray<AcpAdapterEvent> = [
  { type: "acp_turn_started" },
  { type: "acp_tool_call", toolCallId: "toolcall.cursor.5", toolName: "apply_patch" },
  {
    type: "acp_tool_result",
    toolCallId: "toolcall.cursor.5",
    toolName: "apply_patch",
    ok: false,
    messageSafe: "patch did not apply",
  },
  { type: "acp_turn_stop", stopReason: "refusal" },
];

// ---------------------------------------------------------------------------
// Adapter factory
// ---------------------------------------------------------------------------

/** Configuration for {@link makeCursorHarnessAdapter}. */
export interface CursorHarnessAdapterConfig {
  /** Live ACP peer transport; overrides the scripted turn when present. */
  readonly transport?: AcpTransport;
  /**
   * The resolved absolute (or PATH-resolvable) `cursor-agent` executable,
   * INJECTED by the caller after resolving it via {@link CURSOR_AGENT_DISCOVERY}.
   * There is no built-in default; a refused path fails session start.
   */
  readonly cursorAgentPath: string;
  /** Override the pinned harness slug (defaults to the profile's `cursor`). */
  readonly harnessId?: string;
  /**
   * The scripted ACP projection sequence each prompt turn replays for hermetic
   * conformance (defaults to {@link cursorAcpTurnScript}). No live peer.
   */
  readonly script?: ReadonlyArray<AcpAdapterEvent>;
}

/**
 * Build a named Cursor {@link AgentHarness}. It configures the generic ACP
 * factory with the pinned {@link CURSOR_PEER_PROFILE} and the capability flags
 * DERIVED from {@link CURSOR_CAPABILITY_TABLE}, then wraps `start` to enforce the
 * injected `cursorAgentPath`: a refused path fails with `HarnessStartError`
 * (`failureClass: "cursor_agent_path_required"`) rather than silently falling
 * back to a hard-coded binary.
 *
 * All turn projection, tool correlation, lossless suspend/degraded continue, and
 * approval-to-RuntimeInteraction routing are inherited unchanged from the ACP
 * factory — the named adapter's job is to PIN and PUBLISH the profile, not to
 * re-implement the projection.
 */
export const makeCursorHarnessAdapter = (config: CursorHarnessAdapterConfig): AgentHarness => {
  const harnessId = config.harnessId ?? CURSOR_PEER_PROFILE.harnessId;
  const cursorAgentPath = config.cursorAgentPath;
  const flags = cursorFactoryCapabilityFlags();

  const base = makeAcpHarnessAdapter({
    harnessId,
    harnessKind: CURSOR_PEER_PROFILE.harnessKind,
    adapterKind: CURSOR_PEER_PROFILE.adapterKind,
    builtinTools: CURSOR_BUILTIN_TOOLS,
    script: config.script ?? cursorAcpTurnScript,
    ...(config.transport === undefined ? {} : { transport: config.transport }),
    supportsSuspend: flags.supportsSuspend,
    supportsContinue: flags.supportsContinue,
    continueIsLossy: flags.continueIsLossy,
    supportsCompact: flags.supportsCompact,
    supportsDetach: flags.supportsDetach,
    supportsBuiltinToolApprovals: flags.supportsBuiltinToolApprovals,
    supportsBuiltinToolFiltering: flags.supportsBuiltinToolFiltering,
  });

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    isRefusedCursorAgentPath(cursorAgentPath)
      ? Effect.fail(
          new HarnessStartError({
            harnessId,
            sessionId: options.sessionId,
            failureClass: "cursor_agent_path_required",
            detail:
              "cursorAgentPath must be an injected cursor-agent executable; there is no hard-coded default",
          }),
        )
      : base.start(options);

  return {
    ...base,
    harnessId,
    start,
  };
};
