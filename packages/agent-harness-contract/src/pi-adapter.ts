import { Cause, Deferred, Effect, Queue, Ref, Schema as S, Stream } from "effect";
import {
  decodeKhalaRuntimeEvent,
  type AgentRuntimeAdapterKind,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
  type KhalaRuntimeUsage,
  type ModelFailureClass,
  modelFailureClassForAiErrorReasonTag,
  type RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import { type HarnessToolIdentity, toolIdentity } from "./common-tool.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import type {
  HarnessHostToolResult,
  HarnessHostToolSpec,
  HarnessToolApprovalDecision,
} from "./host-tool.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessBuiltinToolFiltering } from "./permission.ts";
import type { HarnessPromptControl, HarnessSession, HarnessTurnResult } from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import { KhalaRuntimeEventSchemaLiteral } from "./stream.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * Pi harness adapter (HW-04): the first HOST-PROCESS adapter in this family.
 * Pi (`@earendil-works/pi-coding-agent`) is an in-process Node library —
 * `createAgentSession` builds a full agent session inside the caller's process
 * with injectable session storage, settings, auth, model registry, and custom
 * tools. There is no bridge, no subprocess, and no socket, and Pi itself has no
 * permission system or sandbox: it runs with the authority of the process that
 * embeds it, which is acceptable ONLY on the owner-local lane (Desktop, Pylon)
 * under the same owner-executor invariant as local Codex work.
 *
 * This module deliberately does NOT depend on the `@mariozechner/pi-*` /
 * `@earendil-works/*` packages. It consumes a minimal STRUCTURAL interface for
 * the Pi session surface ({@link PiSessionSurface}, created through an injected
 * {@link PiSessionFactory}), so the adapter is dependency-free and hermetically
 * testable; a downstream consumer passes the real Pi modules through the seam.
 * Every structural member mirrors a verified export of the audited Pi source
 * (`createAgentSession`, `AgentSession.prompt/steer/abort/compact/subscribe/
 * dispose`, the nine-kind `AgentEvent` union with nested
 * `AssistantMessageEvent` deltas, `getSessionStats`, and the JSONL session-tree
 * file that is Pi's whole resume story).
 *
 * Fidelity posture (matching the contract's blessed forms):
 * - `promptTurn`, `interrupt` (Pi `abort`), `submitUserMessage` (Pi `steer`),
 *   and `compact` map losslessly.
 * - `suspendTurn`/`continueTurn` are an HONEST RERUN: Pi cannot freeze a live
 *   turn, so suspend aborts the in-flight turn and continuation re-drives it
 *   from the persisted JSONL journal. The continuation state is always
 *   declared `lossy: true` — the tail after the cursor is recomputed, never
 *   attached.
 * - Host tools bridge through Pi's `customTools` seam; built-in tool approvals
 *   are EMULATED through Pi's `beforeToolCall` gate, and every approval
 *   request routes through the canonical `RuntimeInteraction` payload model
 *   (see {@link piToolApprovalInteractionPayload}), never a bespoke channel.
 * - The per-account agent directory is INJECTED and validated: the adapter
 *   refuses to start against the owner's live `~/.pi` directory (mirroring the
 *   `pylon auth` isolation rule for Codex account homes).
 */

// ---------------------------------------------------------------------------
// Structural Pi surface (the injected seam)
// ---------------------------------------------------------------------------

/** Pi (`pi-ai`) stop reasons, mirrored structurally. */
export type PiStopReason = "stop" | "length" | "toolUse" | "error" | "aborted";

/**
 * The nested `pi-ai` `AssistantMessageEvent` carried inside `message_update`,
 * reduced to the members this projection reads. Real Pi events carry more
 * fields (`contentIndex`, `partial`, full messages); structural typing lets
 * them pass through untouched.
 */
export type PiAssistantMessageEvent =
  | { readonly type: "text_start" }
  | { readonly type: "text_delta"; readonly delta: string }
  | { readonly type: "text_end" }
  | { readonly type: "thinking_start" }
  | { readonly type: "thinking_delta"; readonly delta: string }
  | { readonly type: "thinking_end" }
  | { readonly type: "done"; readonly reason: "stop" | "length" | "toolUse" }
  | { readonly type: "error"; readonly reason: "aborted" | "error"; readonly messageSafe?: string };

/**
 * The Pi session event union this adapter consumes, mirroring the audited
 * `AgentEvent`/`AgentSessionEvent` kinds. `turn_start`, `turn_end`,
 * `message_start`, `message_end`, and `tool_execution_update` are received and
 * deliberately ignored (the neutral turn boundary is `agent_start`/`agent_end`
 * and tool progress never rides the neutral stream); unknown future kinds fall
 * through the projection's default arm and are ignored rather than failing the
 * stream — Pi publishes no wire schema, so loose consumption is the honest
 * posture.
 */
export type PiSessionEvent =
  | { readonly type: "agent_start" }
  | {
      readonly type: "message_update";
      readonly assistantMessageEvent: PiAssistantMessageEvent;
    }
  | {
      readonly type: "tool_execution_start";
      readonly toolCallId: string;
      readonly toolName: string;
    }
  | {
      readonly type: "tool_execution_end";
      readonly toolCallId: string;
      readonly toolName: string;
      readonly isError: boolean;
      readonly messageSafe?: string;
    }
  | { readonly type: "agent_end" }
  | {
      readonly type:
        | "turn_start"
        | "turn_end"
        | "message_start"
        | "message_end"
        | "tool_execution_update";
    };

/** Pi `getSessionStats()` token counters, reduced to the projected subset. */
export interface PiSessionStats {
  readonly tokens?: {
    readonly input: number;
    readonly output: number;
    readonly cacheRead: number;
    readonly cacheWrite: number;
  };
}

/** Result a bridged host tool hands back to the Pi runtime. */
export interface PiToolExecuteResult {
  /** Serialized text the model reads (Pi tools return text content). */
  readonly content: string;
  readonly isError?: boolean;
}

/**
 * A host tool bridged into Pi's `customTools` seam. `parameters` carries the
 * contract's JSON Schema verbatim: Pi's TypeBox parameter schemas are
 * JSON-Schema-shaped objects, so the wire form passes through and a stricter
 * TypeBox conversion (the AI SDK's `pi-typebox-adapter` is prior art) stays a
 * downstream-consumer concern, not a contract dependency.
 */
export interface PiBridgedToolDefinition {
  readonly name: string;
  readonly description: string;
  readonly parameters: unknown;
  readonly execute: (toolCallId: string, args: unknown) => Promise<PiToolExecuteResult>;
}

/** Decision Pi's `beforeToolCall` gate resolves to (block carries a typed reason). */
export type PiToolCallDecision =
  | { readonly allow: true }
  | { readonly allow: false; readonly reason: string };

/**
 * Options the adapter passes through the injected factory, mirroring the
 * audited `createAgentSession` injection points. The downstream consumer maps
 * these onto the real Pi options (agent dir, `SessionManager.open`, the
 * `tools` allowlist, `customTools`, and the agent-core `beforeToolCall` hook)
 * and MUST NOT default `agentDir` to the owner's live `~/.pi/agent`.
 */
export interface PiCreateSessionOptions {
  /** Isolated per-account agent directory — never the owner's live `~/.pi`. */
  readonly agentDir: string;
  readonly workspaceDir?: string;
  /** JSONL session-tree file to restore; absent for a fresh session. */
  readonly sessionFile?: string;
  /** Native built-in tool allowlist (Pi's `tools` option); absent = all active. */
  readonly activeTools?: ReadonlyArray<string>;
  /** Host tools bridged as Pi custom tools. */
  readonly customTools?: ReadonlyArray<PiBridgedToolDefinition>;
  /** Approval gate consulted before every tool call. */
  readonly beforeToolCall?: (call: {
    readonly toolCallId: string;
    readonly toolName: string;
  }) => Promise<PiToolCallDecision>;
}

/**
 * The live Pi session surface, structurally mirroring the audited
 * `AgentSession` verbs the adapter drives. `sessionFile` is Pi's complete
 * resume artifact (the `parentId`-linked JSONL session tree).
 */
export interface PiSessionSurface {
  readonly sessionFile?: string;
  readonly modelId?: string;
  readonly subscribe: (listener: (event: PiSessionEvent) => void) => () => void;
  readonly prompt: (text: string) => Promise<void>;
  readonly steer: (text: string) => Promise<void>;
  readonly abort: () => Promise<void>;
  readonly compact: (customInstructions?: string) => Promise<void>;
  readonly getSessionStats?: () => PiSessionStats | undefined;
  readonly dispose: () => void;
}

/** The injected `createAgentSession`-like factory. */
export type PiSessionFactory = (options: PiCreateSessionOptions) => Promise<PiSessionSurface>;

// ---------------------------------------------------------------------------
// Tool identity and failure mapping
// ---------------------------------------------------------------------------

/** Pi's native built-in tool set (the default four plus the read-only three). */
export const PI_BUILTIN_TOOL_NAMES: ReadonlyArray<string> = [
  "read",
  "bash",
  "edit",
  "write",
  "grep",
  "find",
  "ls",
];

const PI_BUILTIN_TOOL_SET: ReadonlySet<string> = new Set(PI_BUILTIN_TOOL_NAMES);

/**
 * Pi tool ids are lowercase; the shared common-name map keys Claude PascalCase
 * and Codex snake_case, so this alias resolves the Pi id onto a name the
 * shared map recognizes (the AI SDK adapter's `find` -> `glob` normalization
 * included). `ls` has no common equivalent and is forwarded as-is.
 */
const PI_TO_SHARED_NATIVE: Readonly<Record<string, string>> = {
  read: "Read",
  bash: "Bash",
  edit: "Edit",
  write: "Write",
  grep: "Grep",
  find: "Glob",
};

/**
 * Normalized tool identity for a Pi tool name: the shared common name when one
 * exists, with the true Pi native id always preserved as `nativeName`.
 */
const piToolIdentity = (
  piToolName: string,
  options?: { readonly providerExecuted?: boolean },
): HarnessToolIdentity => {
  const sharedNative = PI_TO_SHARED_NATIVE[piToolName] ?? piToolName;
  const identity = toolIdentity(sharedNative, options);
  return { ...identity, nativeName: piToolName };
};

/**
 * Classify a rejected Pi prompt onto the shared neutral model-failure
 * vocabulary (`@openagentsinc/agent-runtime-schema` `model-failure.ts`, the
 * same mapping `harness-conformance` delegates to). The injected seam reports
 * provider failures with an AiError-style `reasonTag` (`RateLimitError`,
 * `QuotaExhaustedError`, `AuthenticationError`, …) so the mandatory
 * account-capacity classes surface without this adapter inventing an ad hoc
 * keyword classifier; anything untagged is honestly `unknown`.
 */
export const piFailureClassForPromptError = (error: unknown): ModelFailureClass => {
  if (typeof error === "object" && error !== null) {
    const tag = (error as { readonly reasonTag?: unknown }).reasonTag;
    if (typeof tag === "string") return modelFailureClassForAiErrorReasonTag(tag);
  }
  return "unknown";
};

const promptFailureDetail = (error: unknown): string | undefined => {
  if (typeof error === "object" && error !== null) {
    const messageSafe = (error as { readonly messageSafe?: unknown }).messageSafe;
    if (typeof messageSafe === "string") return messageSafe;
  }
  return undefined;
};

/** Pi stop reason -> neutral {@link KhalaRuntimeFinishReason}. */
const PI_STOP_TO_KHALA: Readonly<Record<string, KhalaRuntimeFinishReason>> = {
  stop: "stop",
  length: "length",
  toolUse: "tool-calls",
  aborted: "interrupted",
  error: "error",
};

const mapPiStopReason = (reason: string | undefined): KhalaRuntimeFinishReason =>
  reason === undefined ? "stop" : (PI_STOP_TO_KHALA[reason] ?? "unknown");

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

/**
 * Context threaded through {@link piEventToKhalaEvents} while folding a live Pi
 * event feed. `stopReason` is mutable turn state: Pi carries the terminal stop
 * reason inside the nested `done`/`error` assistant-message events, and the
 * neutral `turn.finished` is synthesized later at `agent_end`.
 */
export interface PiProjectionContext {
  readonly source: KhalaRuntimeSource;
  readonly threadId: string;
  readonly turnId: string;
  /** Allocate the next session-global sequence number. */
  readonly nextSequence: () => number;
  /** toolCallId -> Pi tool name, populated by `tool_execution_start`. */
  readonly toolNames: Map<string, string>;
  /** Names of host-bridged tools this turn (`providerExecuted: false`). */
  readonly hostToolNames: ReadonlySet<string>;
  /** Last stop reason observed from a nested `done`/`error` event. */
  stopReason: PiStopReason | undefined;
  /** Session token stats read at `agent_end` for the usage projection. */
  readonly sessionStats?: () => PiSessionStats | undefined;
}

const base = (ctx: PiProjectionContext, sequence: number, eventSuffix: string) => ({
  schema: KhalaRuntimeEventSchemaLiteral,
  eventId: `evt.${ctx.turnId}.${sequence}.${eventSuffix}`,
  turnId: ctx.turnId,
  threadId: ctx.threadId,
  sequence,
  observedAt: "2026-07-20T00:00:00.000Z",
  source: ctx.source,
  visibility: "private",
  redactionClass: "private_ref",
  causalityRefs: [] as ReadonlyArray<string>,
});

/**
 * Owner-local Pi tool authority. Pi runs in-process under the owner-local
 * profile and every tool call passed the adapter's `beforeToolCall` gate
 * before executing, so the projected authority is an honest allow for this
 * lane — it is not a claim about untrusted or metered lanes, where Pi must
 * not run at all.
 */
const piToolAuthority = (toolCallId: string, wireName: string) => ({
  authorityRef: `authority.pi.${toolCallId}`,
  policyRef: "policy.pi.owner_local",
  decisionRef: `decision.pi.${toolCallId}`,
  toolRef: `toolref.pi.${wireName}`,
  status: "allowed" as const,
  allowed: true,
  blockerRefs: [] as ReadonlyArray<string>,
});

const buildReasoningDelta = (
  ctx: PiProjectionContext,
  sequence: number,
  text: string,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "reasoning"),
    kind: "reasoning.delta",
    messageId: `msg.${ctx.turnId}.reasoning`,
    chunkId: `chunk.${ctx.turnId}.${sequence}`,
    text,
  });

const usageFromStats = (
  ctx: PiProjectionContext,
  sequence: number,
  tokens: NonNullable<PiSessionStats["tokens"]>,
): KhalaRuntimeUsage => ({
  usageRef: `usage.pi.${ctx.turnId}.${sequence}`,
  inputTokens: tokens.input,
  outputTokens: tokens.output,
  cacheReadInputTokens: tokens.cacheRead,
  cacheWriteInputTokens: tokens.cacheWrite,
  totalTokens: tokens.input + tokens.output,
});

/**
 * Pure projection of ONE Pi session event onto zero or more neutral
 * {@link HarnessStreamEvent}s. Sequence numbers come from `ctx.nextSequence` so
 * a caller folds the live feed while keeping session-global cursors
 * contiguous. Nested `message_update` deltas land on `text.delta` /
 * `reasoning.delta`, tool execution start/end correlate through
 * `ctx.toolNames`, and `agent_end` synthesizes `usage.recorded` (once per
 * turn, from session stats — Pi does not stream usage per step) followed by
 * `turn.finished`. Unknown kinds project to nothing.
 */
export const piEventToKhalaEvents = (
  event: PiSessionEvent,
  ctx: PiProjectionContext,
): ReadonlyArray<HarnessStreamEvent> => {
  switch (event.type) {
    case "agent_start":
      return [
        buildTurnStarted({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
        }),
      ];
    case "message_update": {
      const nested = event.assistantMessageEvent;
      switch (nested.type) {
        case "text_delta":
          return [
            buildTextDelta({
              turnId: ctx.turnId,
              threadId: ctx.threadId,
              sequence: ctx.nextSequence(),
              source: ctx.source,
              messageId: `msg.${ctx.turnId}.text`,
              text: nested.delta,
            }),
          ];
        case "thinking_delta":
          return [buildReasoningDelta(ctx, ctx.nextSequence(), nested.delta)];
        case "text_end":
          return [
            decodeKhalaRuntimeEvent({
              ...base(ctx, ctx.nextSequence(), "textend"),
              kind: "text.completed",
              messageId: `msg.${ctx.turnId}.text`,
            }),
          ];
        case "thinking_end":
          return [
            decodeKhalaRuntimeEvent({
              ...base(ctx, ctx.nextSequence(), "reasoningend"),
              kind: "reasoning.completed",
              messageId: `msg.${ctx.turnId}.reasoning`,
            }),
          ];
        case "done":
          ctx.stopReason = nested.reason;
          return [];
        case "error":
          ctx.stopReason = nested.reason;
          return [];
        default:
          return [];
      }
    }
    case "tool_execution_start": {
      ctx.toolNames.set(event.toolCallId, event.toolName);
      const providerExecuted = !ctx.hostToolNames.has(event.toolName);
      const identity = piToolIdentity(event.toolName, { providerExecuted });
      const sequence = ctx.nextSequence();
      return [
        decodeKhalaRuntimeEvent({
          ...base(ctx, sequence, "toolcall"),
          kind: "tool.call",
          toolCallId: event.toolCallId,
          toolName: identity.wireName,
          inputRef: `input.pi.${ctx.turnId}.${sequence}`,
          authority: piToolAuthority(event.toolCallId, identity.wireName),
        }),
      ];
    }
    case "tool_execution_end": {
      const piToolName = ctx.toolNames.get(event.toolCallId) ?? event.toolName;
      const providerExecuted = !ctx.hostToolNames.has(piToolName);
      const identity = piToolIdentity(piToolName, { providerExecuted });
      const sequence = ctx.nextSequence();
      const authority = piToolAuthority(event.toolCallId, identity.wireName);
      return [
        event.isError
          ? decodeKhalaRuntimeEvent({
              ...base(ctx, sequence, "toolerror"),
              kind: "tool.error",
              toolCallId: event.toolCallId,
              toolName: identity.wireName,
              errorRef: `error.pi.${event.toolCallId}`,
              messageSafe: event.messageSafe ?? "Pi tool reported failure",
              authority,
              providerExecuted,
            })
          : decodeKhalaRuntimeEvent({
              ...base(ctx, sequence, "toolresult"),
              kind: "tool.result",
              toolCallId: event.toolCallId,
              toolName: identity.wireName,
              resultRef: `result.pi.${event.toolCallId}`,
              authority,
              providerExecuted,
            }),
      ];
    }
    case "agent_end": {
      const stats = ctx.sessionStats?.();
      const events: Array<HarnessStreamEvent> = [];
      let usage: KhalaRuntimeUsage | undefined;
      if (stats?.tokens !== undefined) {
        const sequence = ctx.nextSequence();
        usage = usageFromStats(ctx, sequence, stats.tokens);
        events.push(
          decodeKhalaRuntimeEvent({
            ...base(ctx, sequence, "usage"),
            kind: "usage.recorded",
            usage,
          }),
        );
      }
      events.push(
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          finishReason: mapPiStopReason(ctx.stopReason),
          ...(usage === undefined ? {} : { usage }),
        }),
      );
      return events;
    }
    default:
      // Pi has no wire schema; unknown/unprojected kinds never fail the stream.
      return [];
  }
};

// ---------------------------------------------------------------------------
// Approval -> RuntimeInteraction
// ---------------------------------------------------------------------------

/**
 * Project a Pi built-in tool call awaiting approval onto a canonical
 * `RuntimeInteractionPayload` of kind `tool_approval` — the durable,
 * provider-neutral approval model every harness approval routes through. Pi
 * has no native approval events; the adapter emulates them with the
 * `beforeToolCall` gate, and an inactive built-in (framework filtering
 * emulation would apply on adapters without native filtering; here it guards
 * against allowlist drift) is refused through this same audited path rather
 * than silently executing.
 */
export const piToolApprovalInteractionPayload = (request: {
  readonly toolCallId: string;
  readonly toolName: string;
  readonly displayText?: string;
  readonly inactiveBuiltin?: boolean;
}): RuntimeInteractionPayload => {
  const identity = piToolIdentity(request.toolName, { providerExecuted: true });
  const displayText =
    request.displayText ??
    (request.inactiveBuiltin === true
      ? `Inactive built-in tool ${identity.wireName} was requested; approval is required.`
      : `Allow the Pi agent to run ${identity.wireName}?`);
  return {
    kind: "tool_approval",
    displayText,
    toolCallId: request.toolCallId,
    toolName: identity.wireName,
    authority: {
      ...piToolAuthority(request.toolCallId, identity.wireName),
      decisionRef: `decision.pi.approval.${request.toolCallId}`,
      status: "operator_escalation_required",
      allowed: false,
      blockerRefs:
        request.inactiveBuiltin === true
          ? ["blocker.inactive_builtin_tool", "blocker.owner_approval"]
          : ["blocker.owner_approval"],
    },
  };
};

// ---------------------------------------------------------------------------
// Lifecycle state
// ---------------------------------------------------------------------------

/**
 * Resume payload for `detach`/`stop`: the JSONL session-tree file is Pi's
 * complete resume artifact — whoever owns the file can reconstruct the
 * transcript and continue.
 */
const PiResumeData = S.Struct({
  sessionFile: S.optionalKey(S.String),
});

/**
 * Continuation payload for `suspendTurn`: the interrupted turn's identity and
 * prompt so the next slice can re-drive it from the restored journal. Always
 * paired with `lossy: true` — partial assistant output and running tools die
 * with the abort and are recomputed.
 */
const PiContinuationData = S.Struct({
  turnId: S.NonEmptyString,
  promptText: S.String,
  sessionFile: S.optionalKey(S.String),
});

/** Schema for the adapter-defined `data` of Pi resume/continuation state. */
export const PiLifecycleState = S.Union([PiContinuationData, PiResumeData]);

// ---------------------------------------------------------------------------
// Adapter factory
// ---------------------------------------------------------------------------

/** Configuration for {@link makePiHarnessAdapter}. */
export interface PiHarnessAdapterConfig {
  /** The injected `createAgentSession`-like factory (the real Pi, or a fixture). */
  readonly createSession: PiSessionFactory;
  /**
   * Isolated per-account Pi agent directory. Validated at `start`: the
   * owner's live `~/.pi` tree is refused outright (a `.pi` path segment),
   * mirroring the Codex account-home isolation rule.
   */
  readonly agentDir: string;
  readonly harnessId?: string;
  /**
   * The dispatch adapter kind. Pi has no dedicated member in
   * `AgentRuntimeAdapterKind` yet; it runs in-process inside the OpenAgents
   * host, so it defaults to the native in-process lane.
   */
  readonly adapterKind?: AgentRuntimeAdapterKind;
  readonly workspaceDir?: string;
  /**
   * Injected session locator: resolve a session id to its JSONL session-tree
   * file when resume state does not pin one (e.g. adopting a session tree the
   * host provisioned out of band).
   */
  readonly locateSessionFile?: (sessionId: string) => string | undefined;
  /**
   * RuntimeInteraction seam: every emulated approval request (pending owner
   * decisions AND auto-denials for rejected/inactive built-ins) is surfaced
   * here as a canonical `tool_approval` payload for the host's durable
   * interaction model.
   */
  readonly onApprovalRequest?: (payload: RuntimeInteractionPayload) => void;
}

const hasLivePiSegment = (dir: string): boolean =>
  dir.split(/[\\/]+/).some((segment) => segment === ".pi");

const activeBuiltinsFor = (
  filtering: HarnessBuiltinToolFiltering | undefined,
): ReadonlyArray<string> | undefined => {
  if (filtering === undefined) return undefined;
  const inactive = new Set(filtering.inactiveTools ?? []);
  const candidates =
    filtering.activeTools === undefined
      ? PI_BUILTIN_TOOL_NAMES
      : PI_BUILTIN_TOOL_NAMES.filter((name) => filtering.activeTools!.includes(name));
  return candidates.filter((name) => !inactive.has(name));
};

const serializeHostToolOutput = (output: unknown): string =>
  typeof output === "string" ? output : (JSON.stringify(output) ?? "null");

interface TurnHandle {
  readonly turnId: string;
  readonly promptText: string;
}

interface SettledTurn {
  readonly finishReason: KhalaRuntimeFinishReason;
  readonly usage?: KhalaRuntimeUsage;
}

/**
 * Build the Pi {@link AgentHarness} over an injected {@link PiSessionFactory}.
 * `start` never touches the factory — the underlying Pi session is built
 * lazily at the first `promptTurn`, because Pi binds custom tools at
 * `createAgentSession` time: a turn whose host-tool signature differs from the
 * live session's forces a session rebuild over the same JSONL journal (the
 * documented Pi lifecycle sharp edge, handled here instead of leaking to
 * callers).
 */
export const makePiHarnessAdapter = (config: PiHarnessAdapterConfig): AgentHarness => {
  const harnessId = config.harnessId ?? "pi";
  const adapterKind: AgentRuntimeAdapterKind = config.adapterKind ?? "openagents_native";

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    Effect.gen(function* () {
      const source: KhalaRuntimeSource = options.source;
      const sessionId = options.sessionId;

      // Agent-dir isolation is a hard start gate, not a convention. Clobbering
      // the owner's live ~/.pi (credentials, sessions, settings) is the Pi
      // equivalent of running `codex login` against the default ~/.codex home.
      if (config.agentDir.trim() === "" || hasLivePiSegment(config.agentDir)) {
        return yield* Effect.fail(
          new HarnessStartError({
            harnessId,
            sessionId,
            failureClass: "unsafe_agent_dir",
            detail:
              "the Pi agent directory must be an isolated per-account directory, never the owner's live ~/.pi tree",
          }),
        );
      }

      // Decode lifecycle payloads through the adapter's own schema so a
      // corrupt or cross-adapter payload fails at start, not inside a turn.
      let resumeSessionFile: string | undefined;
      let seededRerun: TurnHandle | undefined;
      if (options.continueFrom !== undefined) {
        const decoded = yield* Effect.try({
          try: () => S.decodeUnknownSync(PiContinuationData)(options.continueFrom!.data),
          catch: (cause) =>
            new HarnessStartError({
              harnessId,
              sessionId,
              failureClass: "invalid_lifecycle_state",
              detail: "continuation data did not match the Pi continuation schema",
              cause,
            }),
        });
        seededRerun = { turnId: decoded.turnId, promptText: decoded.promptText };
        resumeSessionFile = decoded.sessionFile ?? config.locateSessionFile?.(sessionId);
      } else if (options.resumeFrom !== undefined) {
        const decoded = yield* Effect.try({
          try: () => S.decodeUnknownSync(PiResumeData)(options.resumeFrom!.data),
          catch: (cause) =>
            new HarnessStartError({
              harnessId,
              sessionId,
              failureClass: "invalid_lifecycle_state",
              detail: "resume data did not match the Pi resume schema",
              cause,
            }),
        });
        resumeSessionFile = decoded.sessionFile ?? config.locateSessionFile?.(sessionId);
      }

      const isResume = options.resumeFrom !== undefined || options.continueFrom !== undefined;
      const permissionMode = options.permissionMode ?? "allow-all";
      const activeBuiltins = activeBuiltinsFor(options.builtinToolFiltering);
      const isInactiveBuiltin = (toolName: string): boolean =>
        PI_BUILTIN_TOOL_SET.has(toolName) &&
        activeBuiltins !== undefined &&
        !activeBuiltins.includes(toolName);

      // Session-global monotonic sequence; a continuation-started session
      // keeps counting from the suspended cursor so cursors stay ordered.
      const seq = { value: (options.continueFrom?.cursor ?? -1) + 1 };
      const cursorRef = yield* Ref.make(options.continueFrom?.cursor ?? -1);

      // Host-process adapter state. Plain mutable slots are deliberate here:
      // they are touched from Pi's promise/callback world (subscribe
      // listeners, custom-tool executes, the approval gate), which runs on
      // the same single-threaded loop as the Effect runtime.
      const surfaceState: {
        surface: PiSessionSurface | undefined;
        signature: string | undefined;
      } = { surface: undefined, signature: undefined };
      const activeTurn: { current: TurnHandle | undefined } = { current: undefined };
      const rerun: { current: TurnHandle | undefined } = { current: seededRerun };
      let currentHostToolNames: ReadonlySet<string> = new Set();
      const pendingHostCalls = new Map<string, (result: PiToolExecuteResult) => void>();
      const pendingApprovals = new Map<string, (decision: HarnessToolApprovalDecision) => void>();
      const sessionAllowedTools = new Set<string>();
      let firstPromptDone = isResume;
      let destroyed = false;

      const guardToolCall = async (call: {
        readonly toolCallId: string;
        readonly toolName: string;
      }): Promise<PiToolCallDecision> => {
        // Host tools are dispatched by the host itself; the result path is
        // the control surface, not a pre-execution approval.
        if (currentHostToolNames.has(call.toolName)) return { allow: true };
        if (isInactiveBuiltin(call.toolName)) {
          config.onApprovalRequest?.(
            piToolApprovalInteractionPayload({ ...call, inactiveBuiltin: true }),
          );
          return {
            allow: false,
            reason: "inactive built-in tool denied through the approval path",
          };
        }
        if (permissionMode === "allow-all") return { allow: true };
        if (permissionMode === "reject-all") {
          config.onApprovalRequest?.(piToolApprovalInteractionPayload(call));
          return { allow: false, reason: "session permission mode rejects built-in tools" };
        }
        // `default`: emulated approval through the RuntimeInteraction seam,
        // resolved by the host via `submitToolApproval`.
        if (sessionAllowedTools.has(call.toolName)) return { allow: true };
        const decision = await new Promise<HarnessToolApprovalDecision>((resolve) => {
          pendingApprovals.set(call.toolCallId, resolve);
          config.onApprovalRequest?.(piToolApprovalInteractionPayload(call));
        });
        if (decision === "deny") {
          return { allow: false, reason: "the owner denied the tool call" };
        }
        if (decision === "allow-session") sessionAllowedTools.add(call.toolName);
        return { allow: true };
      };

      const bridgeHostTool = (spec: HarnessHostToolSpec): PiBridgedToolDefinition => ({
        name: spec.name,
        description: spec.description,
        parameters: spec.inputJsonSchema,
        execute: (toolCallId) =>
          new Promise<PiToolExecuteResult>((resolve) => {
            pendingHostCalls.set(toolCallId, resolve);
          }),
      });

      const ensureSurface = (
        tools: ReadonlyArray<HarnessHostToolSpec>,
        turnId: string,
      ): Effect.Effect<PiSessionSurface, HarnessTurnError> =>
        Effect.gen(function* () {
          const signature = tools
            .map((tool) => tool.name)
            .sort()
            .join(",");
          if (surfaceState.surface !== undefined && surfaceState.signature === signature) {
            return surfaceState.surface;
          }
          if (surfaceState.surface !== undefined) {
            // A changed host-tool signature forces a Pi session rebuild over
            // the SAME JSONL journal (custom tools bind at creation time).
            resumeSessionFile = surfaceState.surface.sessionFile ?? resumeSessionFile;
            surfaceState.surface.dispose();
            surfaceState.surface = undefined;
          }
          currentHostToolNames = new Set(tools.map((tool) => tool.name));
          const surface = yield* Effect.tryPromise({
            try: () =>
              config.createSession({
                agentDir: config.agentDir,
                ...(config.workspaceDir === undefined ? {} : { workspaceDir: config.workspaceDir }),
                ...(resumeSessionFile === undefined ? {} : { sessionFile: resumeSessionFile }),
                ...(activeBuiltins === undefined ? {} : { activeTools: activeBuiltins }),
                customTools: tools.map(bridgeHostTool),
                beforeToolCall: guardToolCall,
              }),
            catch: (cause) =>
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId,
                failureClass: "session_create_failed",
                cause,
              }),
          });
          surfaceState.surface = surface;
          surfaceState.signature = signature;
          return surface;
        });

      const driveTurn = (
        turnId: string,
        promptText: string,
        tools: ReadonlyArray<HarnessHostToolSpec>,
      ): Effect.Effect<HarnessPromptControl, HarnessTurnError> =>
        Effect.gen(function* () {
          if (activeTurn.current !== undefined) {
            return yield* Effect.fail(
              new HarnessTurnError({
                harnessId,
                sessionId,
                turnId,
                failureClass: "turn_already_active",
                detail: `turn ${activeTurn.current.turnId} is still in flight`,
              }),
            );
          }
          const surface = yield* ensureSurface(tools, turnId);
          const queue = yield* Queue.unbounded<HarnessStreamEvent, HarnessTurnError | Cause.Done>();
          const settled = yield* Deferred.make<SettledTurn, HarnessTurnError>();

          const ctx: PiProjectionContext = {
            source,
            threadId: sessionId,
            turnId,
            nextSequence: () => seq.value++,
            toolNames: new Map<string, string>(),
            hostToolNames: currentHostToolNames,
            stopReason: undefined,
            sessionStats: () => surface.getSessionStats?.(),
          };

          let unsubscribe: () => void = () => {};
          const failTurn = (error: HarnessTurnError): void => {
            activeTurn.current = undefined;
            unsubscribe();
            Deferred.doneUnsafe(settled, Effect.fail(error));
            Queue.failCauseUnsafe(queue, Cause.fail(error));
          };
          const listener = (event: PiSessionEvent): void => {
            let projected: ReadonlyArray<HarnessStreamEvent>;
            try {
              projected = piEventToKhalaEvents(event, ctx);
            } catch (cause) {
              failTurn(
                new HarnessTurnError({
                  harnessId,
                  sessionId,
                  turnId,
                  failureClass: "event_projection_failed",
                  cause,
                }),
              );
              return;
            }
            for (const projectedEvent of projected) {
              Queue.offerUnsafe(queue, projectedEvent);
            }
            if (event.type === "agent_end") {
              activeTurn.current = undefined;
              unsubscribe();
              const finished = projected.find(
                (candidate): candidate is Extract<HarnessStreamEvent, { kind: "turn.finished" }> =>
                  candidate.kind === "turn.finished",
              );
              Deferred.doneUnsafe(
                settled,
                Effect.succeed({
                  finishReason: finished?.finishReason ?? mapPiStopReason(ctx.stopReason),
                  ...(finished?.usage === undefined ? {} : { usage: finished.usage }),
                }),
              );
              Queue.endUnsafe(queue);
            }
          };
          unsubscribe = surface.subscribe(listener);

          activeTurn.current = { turnId, promptText };
          rerun.current = undefined;
          yield* Effect.sync(() => {
            // Failures are otherwise encoded in Pi's event stream; a rejected
            // prompt is the transport-level failure path (provider capacity,
            // auth health) and must carry the shared failure class.
            surface.prompt(promptText).catch((cause: unknown) => {
              const detail = promptFailureDetail(cause);
              failTurn(
                new HarnessTurnError({
                  harnessId,
                  sessionId,
                  turnId,
                  failureClass: piFailureClassForPromptError(cause),
                  ...(detail === undefined ? {} : { detail }),
                  cause,
                }),
              );
            });
          });

          const control: HarnessPromptControl = {
            turnId,
            events: Stream.fromQueue(queue).pipe(
              Stream.tap((event) => Ref.set(cursorRef, event.sequence)),
            ),
            done: Deferred.await(settled).pipe(
              Effect.flatMap((outcome) =>
                Ref.get(cursorRef).pipe(
                  Effect.map(
                    (cursor): HarnessTurnResult => ({
                      turnId,
                      finishReason: outcome.finishReason,
                      ...(outcome.usage === undefined ? {} : { usage: outcome.usage }),
                      lastCursor: cursor,
                    }),
                  ),
                ),
              ),
            ),
            submitToolResult: (result: HarnessHostToolResult) =>
              Effect.gen(function* () {
                const resolve = pendingHostCalls.get(result.toolCallId);
                if (resolve === undefined) {
                  return yield* Effect.fail(
                    new HarnessTurnError({
                      harnessId,
                      sessionId,
                      turnId,
                      failureClass: "no_active_tool_call",
                      detail: `no host tool call is awaiting a result for ${result.toolCallId}`,
                    }),
                  );
                }
                pendingHostCalls.delete(result.toolCallId);
                resolve({
                  content: serializeHostToolOutput(result.output),
                  ...(result.isError === undefined ? {} : { isError: result.isError }),
                });
              }),
            submitToolApproval: (toolCallId, decision) =>
              Effect.gen(function* () {
                const resolve = pendingApprovals.get(toolCallId);
                if (resolve === undefined) {
                  return yield* Effect.fail(
                    new HarnessTurnError({
                      harnessId,
                      sessionId,
                      turnId,
                      failureClass: "no_pending_approval",
                      detail: `no built-in tool call is awaiting approval for ${toolCallId}`,
                    }),
                  );
                }
                pendingApprovals.delete(toolCallId);
                resolve(decision);
              }),
            submitUserMessage: (text) =>
              Effect.tryPromise({
                try: () => surface.steer(text),
                catch: (cause) =>
                  new HarnessTurnError({
                    harnessId,
                    sessionId,
                    turnId,
                    failureClass: "steer_failed",
                    cause,
                  }),
              }),
            interrupt: () =>
              Effect.promise(() =>
                surface.abort().catch(() => {
                  // Interrupt is idempotent and best-effort by contract.
                }),
              ),
          };
          return control;
        });

      const promptTurn = (opts: {
        readonly turnId: string;
        readonly prompt: string;
        readonly instructions?: string;
        readonly tools?: ReadonlyArray<HarnessHostToolSpec>;
      }): Effect.Effect<HarnessPromptControl, HarnessTurnError> => {
        // Instructions apply once, prepended to the first user message of a
        // fresh session, and never re-apply on a resumed session.
        const promptText =
          !firstPromptDone && opts.instructions !== undefined
            ? `${opts.instructions}\n\n${opts.prompt}`
            : opts.prompt;
        firstPromptDone = true;
        return driveTurn(opts.turnId, promptText, opts.tools ?? []);
      };

      const continueTurn = (opts: {
        readonly tools?: ReadonlyArray<HarnessHostToolSpec>;
      }): Effect.Effect<HarnessPromptControl, HarnessTurnError> => {
        const target = rerun.current;
        if (target === undefined) {
          return Effect.fail(
            new HarnessTurnError({
              harnessId,
              sessionId,
              turnId: "unknown",
              failureClass: "no_turn_to_continue",
            }),
          );
        }
        // Honest degraded rerun: re-drive the recorded prompt against the
        // journal-restored session. The recomputed tail attaches at
        // cursor + 1 but its CONTENT is recomputed, exactly as the
        // `lossy: true` continuation state declared.
        return driveTurn(target.turnId, target.promptText, opts.tools ?? []);
      };

      const sessionFileNow = (): string | undefined =>
        surfaceState.surface?.sessionFile ?? resumeSessionFile;

      const suspendTurn = (): Effect.Effect<HarnessContinuationState> =>
        Effect.gen(function* () {
          // Capture the turn BEFORE aborting: the abort settles the turn and
          // clears the active slot through the agent_end listener.
          const turn = activeTurn.current ?? rerun.current;
          const surface = surfaceState.surface;
          if (activeTurn.current !== undefined && surface !== undefined) {
            yield* Effect.promise(() =>
              surface.abort().catch(() => {
                // Suspend still returns honest continuation state.
              }),
            );
          }
          if (turn !== undefined) rerun.current = turn;
          const cursor = yield* Ref.get(cursorRef);
          const sessionFile = sessionFileNow();
          return {
            harnessId,
            sessionId,
            turnId: turn?.turnId ?? "unknown",
            cursor,
            // Pi cannot freeze a live turn: the continuation re-drives from
            // the JSONL journal and the tail after `cursor` is recomputed.
            lossy: true,
            data: {
              turnId: turn?.turnId ?? "unknown",
              promptText: turn?.promptText ?? "",
              ...(sessionFile === undefined ? {} : { sessionFile }),
            },
          };
        });

      const resumeState = (): Effect.Effect<HarnessResumeState> =>
        Effect.sync(() => {
          const sessionFile = sessionFileNow();
          return {
            harnessId,
            sessionId,
            data: sessionFile === undefined ? {} : { sessionFile },
          };
        });

      const compact = (customInstructions?: string): Effect.Effect<void> =>
        Effect.promise(async () => {
          // Native lossless passthrough: Pi owns compaction (manual,
          // threshold, overflow) and records it in the session journal.
          const surface = surfaceState.surface;
          if (surface !== undefined) await surface.compact(customInstructions);
        });

      const stop = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          const state = yield* resumeState();
          surfaceState.surface?.dispose();
          surfaceState.surface = undefined;
          surfaceState.signature = undefined;
          return state;
        });

      const destroy = (): Effect.Effect<void> =>
        Effect.sync(() => {
          if (destroyed) return;
          destroyed = true;
          surfaceState.surface?.dispose();
          surfaceState.surface = undefined;
          surfaceState.signature = undefined;
        });

      const session: HarnessSession = {
        sessionId,
        isResume,
        promptTurn,
        continueTurn,
        suspendTurn,
        compact,
        // Detach parks: the JSONL session tree is the cross-process resume
        // artifact, and the surface stays alive for a same-process reattach.
        detach: resumeState,
        stop,
        destroy,
      };
      return session;
    });

  return {
    specificationVersion: "agent-harness-v1",
    harnessId,
    // Pi has no dedicated member in the harness-kind vocabulary yet.
    harnessKind: "custom",
    adapterKind,
    // Pi's native tool ids (lowercase) with their shared common names;
    // `find` normalizes to the common `glob`, `ls` has no common equivalent.
    builtinTools: [
      { nativeName: "read", commonName: "read", description: "read a file" },
      { nativeName: "bash", commonName: "bash", description: "run a shell command" },
      { nativeName: "edit", commonName: "edit", description: "edit a file" },
      { nativeName: "write", commonName: "write", description: "write a file" },
      { nativeName: "grep", commonName: "grep", description: "search file contents" },
      { nativeName: "find", commonName: "glob", description: "match paths by glob" },
      { nativeName: "ls", description: "list directory entries" },
    ],
    // Emulated through Pi's beforeToolCall gate, routed via RuntimeInteraction.
    supportsBuiltinToolApprovals: true,
    // Native: Pi's `tools` allowlist filters built-ins at session creation.
    supportsBuiltinToolFiltering: true,
    lifecycleStateSchema: PiLifecycleState,
    start,
  };
};
