import { Effect, Option, Ref, Schema as S, Stream } from "effect";
import {
  type AgentDefinitionHarnessKind,
  type AgentRuntimeAdapterKind,
  decodeKhalaRuntimeEvent,
  KhalaRuntimeEventSchemaLiteral,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
  type RuntimeInteractionPayload,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import type { HarnessBuiltinTool } from "./common-tool.ts";
import { toolIdentity } from "./common-tool.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessPromptControl, HarnessSession, HarnessTurnResult } from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * Generic ACP (Agent Client Protocol) harness adapter factory (HARN-04).
 *
 * The desktop ACP runtime bridge (`@openagentsinc/agent-client-runtime-bridge`,
 * `AcpRuntimeProjector`) already folds a Grok/Cursor peer session's native
 * updates into a canonical vocabulary — `AcpProjectionEvent`, which is
 * `KhalaRuntimeEvent` runtime events plus bounded `AcpCanonicalStateEvent`
 * snapshots. `apps/openagents-desktop/src/provider-lane-acp.ts` then hand-maps
 * that projection onto the frozen renderer `ClaudeLocalEvent` envelope.
 *
 * This module is the NEUTRAL direction: it takes the SAME ACP projection
 * vocabulary and lands it on the harness contract's neutral `HarnessStreamEvent`
 * (= `KhalaRuntimeEvent`), so ANY admitted ACP peer becomes an `AgentHarness`
 * through ONE factory instead of a bespoke lane. Approvals do NOT ride the
 * event stream here: an ACP `session/request_permission` projects into the
 * canonical durable `RuntimeInteraction` model (kind `tool_approval`), which is
 * where HARN-04's H4 point routes every approval.
 *
 * `AcpAdapterEvent` is a self-contained LOCAL union so the contract package does
 * not depend on the desktop bridge package; each member documents the bridge
 * type / projector discriminator it mirrors.
 */

// ---------------------------------------------------------------------------
// Local ACP projection input vocabulary
// ---------------------------------------------------------------------------

/**
 * Turn opened. Mirrors `AcpRuntimeProjector.begin()` (`turn.started`), which the
 * bridge emits before any peer update. Identity (turn/thread) comes from the
 * projection context, not the scripted event.
 */
export interface AcpTurnStartedEvent {
  readonly type: "acp_turn_started";
}

/**
 * Assistant text delta. Mirrors the bridge `agent_message_chunk` discriminator,
 * which `AcpRuntimeProjector.#project` folds into `text.delta`.
 */
export interface AcpTextDeltaEvent {
  readonly type: "acp_text_delta";
  readonly text: string;
  /** Groups deltas of one assistant message; defaults to a per-turn message key. */
  readonly messageKey?: string;
}

/**
 * Assistant reasoning/thought delta. Mirrors the bridge `agent_thought_chunk`
 * discriminator, which the projector folds into `reasoning.delta`.
 */
export interface AcpThoughtDeltaEvent {
  readonly type: "acp_thought_delta";
  readonly text: string;
  readonly messageKey?: string;
}

/**
 * A peer tool call started. Mirrors the bridge `tool_call` discriminator, which
 * `AcpRuntimeProjector.#projectTool` folds into `tool.call`. `toolName` is the
 * peer's native tool name and is normalized through {@link toolIdentity}.
 */
export interface AcpToolCallEvent {
  readonly type: "acp_tool_call";
  readonly toolCallId: string;
  readonly toolName: string;
  readonly inputRef?: string;
}

/**
 * A peer tool call reached a terminal state. Mirrors the bridge
 * `tool_call_update` discriminator at `status: "completed" | "failed"`, which
 * the projector folds into `tool.result` (completed) or `tool.error` (failed).
 */
export interface AcpToolResultEvent {
  readonly type: "acp_tool_result";
  readonly toolCallId: string;
  readonly toolName: string;
  readonly ok: boolean;
  readonly resultRef?: string;
  /** Public-safe failure summary for the `tool.error` projection. */
  readonly messageSafe?: string;
}

/**
 * A peer permission/approval request. Mirrors the ACP `session/request_permission`
 * request (which the bridge deliberately does NOT project as a transcript
 * `KhalaRuntimeEvent` — it records provider-reported tool state as non-authority).
 * This adapter routes it through {@link acpPermissionToRuntimeInteractionPayload}
 * instead of the event stream.
 */
export interface AcpPermissionRequestEvent {
  readonly type: "acp_permission_request";
  readonly toolCallId: string;
  readonly toolName: string;
  readonly displayText?: string;
  /**
   * True when the framework — not the peer — raised this request because the
   * peer lane has no native built-in-tool filtering and the call targets an
   * inactive built-in (see the framework-emulation note on the helper below).
   */
  readonly inactiveBuiltin?: boolean;
}

/**
 * The turn stopped. Mirrors the bridge `session/prompt` settlement
 * (`AcpRuntimeProjector.settle` / `complete`), whose `stopReason` maps onto the
 * `turn.finished` finish reason.
 */
export interface AcpTurnStopEvent {
  readonly type: "acp_turn_stop";
  /** ACP stop reason: `end_turn`, `max_tokens`, `cancelled`, `refusal`, … */
  readonly stopReason: string;
}

/** The neutral local ACP projection vocabulary this adapter consumes. */
export type AcpAdapterEvent =
  | AcpTurnStartedEvent
  | AcpTextDeltaEvent
  | AcpThoughtDeltaEvent
  | AcpToolCallEvent
  | AcpToolResultEvent
  | AcpPermissionRequestEvent
  | AcpTurnStopEvent;

/**
 * Projection context: the turn/thread identity and event source every projected
 * event carries, plus the session-global sequence allocator. Sequencing is
 * delegated to the caller so the projection stays pure and its output attaches
 * contiguously across many source events (a permission request consumes no
 * sequence because it emits no stream event).
 */
export interface AcpAdapterContext {
  readonly turnId: string;
  readonly threadId: string;
  readonly source: KhalaRuntimeSource;
  /** Returns the next session-global monotonic sequence. */
  readonly nextSequence: () => number;
}

// ---------------------------------------------------------------------------
// Projection
// ---------------------------------------------------------------------------

const base = (ctx: AcpAdapterContext, sequence: number, eventSuffix: string) => ({
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
 * Provider-reported ACP tool authority. Mirrors the bridge projector's stance:
 * a peer tool event is REPORTED state, never our authority decision, so it is
 * recorded as `denied` / not-authority with the standard blocker ref.
 */
const providerReportedAuthority = (toolCallId: string, wireName: string) => ({
  authorityRef: `authority.acp.${toolCallId}`,
  policyRef: "policy.acp_bridge",
  decisionRef: "decision.provider_reported_not_authority",
  toolRef: `toolref.acp.${wireName}`,
  status: "denied" as const,
  allowed: false,
  blockerRefs: ["blocker.provider_event_not_authority"],
});

const stopReasonToFinishReason = (stopReason: string): KhalaRuntimeFinishReason =>
  stopReason === "end_turn"
    ? "stop"
    : stopReason === "max_tokens"
      ? "length"
      : stopReason === "cancelled"
        ? "cancelled"
        : stopReason === "refusal"
          ? "content-filter"
          : "unknown";

/**
 * Project one ACP projection event onto the neutral `HarnessStreamEvent` stream.
 * Every constructed event is validated through `decodeKhalaRuntimeEvent` (via the
 * shared event builders or an inline decode), so a malformed event fails at
 * construction rather than at the stream boundary.
 *
 * An `acp_permission_request` returns NO stream events — an approval is not a
 * transcript item; it routes through {@link acpPermissionToRuntimeInteractionPayload}.
 */
export const acpEventToKhalaEvents = (
  event: AcpAdapterEvent,
  ctx: AcpAdapterContext,
): ReadonlyArray<HarnessStreamEvent> => {
  switch (event.type) {
    case "acp_turn_started":
      return [
        buildTurnStarted({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
        }),
      ];
    case "acp_text_delta": {
      const sequence = ctx.nextSequence();
      return [
        buildTextDelta({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence,
          source: ctx.source,
          messageId: event.messageKey ?? `msg.${ctx.turnId}.text`,
          text: event.text,
        }),
      ];
    }
    case "acp_thought_delta": {
      const sequence = ctx.nextSequence();
      return [
        decodeKhalaRuntimeEvent({
          ...base(ctx, sequence, "reasoning"),
          kind: "reasoning.delta",
          messageId: event.messageKey ?? `msg.${ctx.turnId}.reasoning`,
          chunkId: `chunk.${ctx.turnId}.${sequence}`,
          text: event.text,
        }),
      ];
    }
    case "acp_tool_call": {
      const sequence = ctx.nextSequence();
      const identity = toolIdentity(event.toolName, { providerExecuted: true });
      return [
        decodeKhalaRuntimeEvent({
          ...base(ctx, sequence, "toolcall"),
          kind: "tool.call",
          toolCallId: event.toolCallId,
          toolName: identity.wireName,
          ...(event.inputRef === undefined ? {} : { inputRef: event.inputRef }),
          authority: providerReportedAuthority(event.toolCallId, identity.wireName),
        }),
      ];
    }
    case "acp_tool_result": {
      const sequence = ctx.nextSequence();
      const identity = toolIdentity(event.toolName, { providerExecuted: true });
      const authority = providerReportedAuthority(event.toolCallId, identity.wireName);
      return [
        event.ok
          ? decodeKhalaRuntimeEvent({
              ...base(ctx, sequence, "toolresult"),
              kind: "tool.result",
              toolCallId: event.toolCallId,
              toolName: identity.wireName,
              resultRef: event.resultRef ?? `result.acp.${event.toolCallId}`,
              authority,
              providerExecuted: true,
            })
          : decodeKhalaRuntimeEvent({
              ...base(ctx, sequence, "toolerror"),
              kind: "tool.error",
              toolCallId: event.toolCallId,
              toolName: identity.wireName,
              errorRef: `error.acp.${event.toolCallId}`,
              messageSafe: event.messageSafe ?? "ACP tool reported failure",
              authority,
              providerExecuted: true,
            }),
      ];
    }
    case "acp_permission_request":
      // Approvals are not transcript items; they route through RuntimeInteraction.
      return [];
    case "acp_turn_stop":
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          finishReason: stopReasonToFinishReason(event.stopReason),
        }),
      ];
  }
};

// ---------------------------------------------------------------------------
// Approval → RuntimeInteraction
// ---------------------------------------------------------------------------

/**
 * Project an ACP permission request onto a canonical `RuntimeInteractionPayload`
 * of kind `tool_approval` — the durable, provider-neutral approval model HARN-04
 * routes every approval through. The peer's own tool state is provider-reported,
 * so the carried authority is `operator_escalation_required` (owner decision
 * pending), never a self-granted allow.
 *
 * Framework emulation of native built-in-tool filtering: a peer lane that
 * declares `supportsBuiltinToolFiltering === false` cannot hide inactive
 * built-ins from its runtime. When such a peer calls an inactive built-in, the
 * framework raises a permission request with `inactiveBuiltin: true`, projects
 * it here, and auto-DENIES it — so an unavailable built-in is refused through the
 * same audited approval path a human approval would use, rather than silently
 * executing. The auto-deny decision is applied downstream via
 * `applyRuntimeInteractionDecision`; this factory only produces the request
 * payload.
 */
export const acpPermissionToRuntimeInteractionPayload = (
  event: AcpPermissionRequestEvent,
): RuntimeInteractionPayload => {
  const identity = toolIdentity(event.toolName, { providerExecuted: true });
  const displayText =
    event.displayText ??
    (event.inactiveBuiltin
      ? `Inactive built-in tool ${identity.wireName} was requested; approval is required.`
      : `Allow the ACP agent to run ${identity.wireName}?`);
  return {
    kind: "tool_approval",
    displayText,
    toolCallId: event.toolCallId,
    toolName: identity.wireName,
    authority: {
      ...providerReportedAuthority(event.toolCallId, identity.wireName),
      // An approval request is pending owner escalation, not a denial of record.
      status: "operator_escalation_required",
      blockerRefs: event.inactiveBuiltin
        ? ["blocker.inactive_builtin_tool", "blocker.owner_approval"]
        : ["blocker.owner_approval"],
    },
  };
};

// ---------------------------------------------------------------------------
// Adapter factory
// ---------------------------------------------------------------------------

/** Configuration for {@link makeAcpHarnessAdapter}. */
export interface AcpHarnessAdapterConfig {
  /** Stable kebab-case slug for the peer (`grok`, `cursor`). */
  readonly harnessId: string;
  /**
   * The canonical harness kind. `grok_cli` is a member of
   * `AgentDefinitionHarnessKind`; a peer with no dedicated kind (Cursor) uses
   * `custom` — `cursor_cli` exists only in `AgentRuntimeAdapterKind`, carried by
   * {@link adapterKind}.
   */
  readonly harnessKind: AgentDefinitionHarnessKind;
  /** The dispatch adapter kind; defaults to `agent_client_protocol`. */
  readonly adapterKind?: AgentRuntimeAdapterKind;
  /** Scripted ACP projection sequence replayed for each prompt turn (no live peer). */
  readonly script?: ReadonlyArray<AcpAdapterEvent>;
  /** Built-in tools the peer exposes natively, for normalization/filtering. */
  readonly builtinTools?: ReadonlyArray<HarnessBuiltinTool>;
  readonly supportsBuiltinToolApprovals?: boolean;
  readonly supportsBuiltinToolFiltering?: boolean;
  readonly supportsSuspend?: boolean;
  readonly supportsContinue?: boolean;
  readonly supportsCompact?: boolean;
  readonly supportsDetach?: boolean;
  /** When true, `continueTurn` reports the continuation as a re-driven (lossy) tail. */
  readonly continueIsLossy?: boolean;
}

/** Default scripted turn: a representative contiguous ACP projection sequence. */
const DEFAULT_SCRIPT: ReadonlyArray<AcpAdapterEvent> = [
  { type: "acp_turn_started" },
  { type: "acp_thought_delta", text: "Planning the change." },
  { type: "acp_text_delta", text: "Reading the file." },
  { type: "acp_tool_call", toolCallId: "toolcall.acp.1", toolName: "read_file" },
  { type: "acp_tool_result", toolCallId: "toolcall.acp.1", toolName: "read_file", ok: true },
  { type: "acp_text_delta", text: " Done." },
  { type: "acp_turn_stop", stopReason: "end_turn" },
];

interface ContinuationData {
  readonly turnId: string;
  readonly remaining: ReadonlyArray<HarnessStreamEvent>;
}

const ContinuationDataSchema = S.Struct({
  turnId: S.NonEmptyString,
  remaining: S.Array(S.Unknown),
});

interface ActiveTurn {
  readonly turnId: string;
  readonly remaining: ReadonlyArray<HarnessStreamEvent>;
}

const makeControl = (params: {
  readonly harnessId: string;
  readonly sessionId: string;
  readonly turnId: string;
  readonly events: ReadonlyArray<HarnessStreamEvent>;
  readonly cursorRef: Ref.Ref<number>;
  readonly activeRef: Ref.Ref<Option.Option<ActiveTurn>>;
}): HarnessPromptControl => {
  const { turnId, events, cursorRef, activeRef } = params;

  const stream: Stream.Stream<HarnessStreamEvent, HarnessTurnError> = Stream.fromIterable(
    events,
  ).pipe(
    Stream.tap((event) =>
      Effect.gen(function* () {
        yield* Ref.set(cursorRef, event.sequence);
        yield* Ref.set(
          activeRef,
          Option.some({
            turnId,
            remaining: events.filter((e) => e.sequence > event.sequence),
          }),
        );
      }),
    ),
  );

  const done: Effect.Effect<HarnessTurnResult, HarnessTurnError> = Effect.gen(function* () {
    const cursor = yield* Ref.get(cursorRef);
    const active = yield* Ref.get(activeRef);
    const remaining = Option.match(active, {
      onNone: () => [] as ReadonlyArray<HarnessStreamEvent>,
      onSome: (a) => a.remaining,
    });
    return {
      turnId,
      finishReason: remaining.length === 0 ? "stop" : "interrupted",
      lastCursor: cursor,
    } satisfies HarnessTurnResult;
  });

  const notActive = () =>
    new HarnessTurnError({
      harnessId: params.harnessId,
      sessionId: params.sessionId,
      turnId,
      failureClass: "no_active_tool_call",
      detail: "ACP adapter routes approvals through RuntimeInteraction, not the tool channel",
    });

  return {
    turnId,
    events: stream,
    done,
    submitToolResult: () => Effect.fail(notActive()),
    submitToolApproval: () => Effect.fail(notActive()),
    submitUserMessage: () => Effect.void,
    interrupt: () => Effect.void,
  };
};

/**
 * Build a generic ACP {@link AgentHarness} for one admitted peer. `promptTurn`
 * projects the scripted ACP event sequence through {@link acpEventToKhalaEvents}
 * with session-global contiguous sequences, and models lossless suspend/continue
 * exactly like the reference adapter: `suspendTurn` pins the cursor of the last
 * event the consumer pulled plus the remaining projected tail, and `continueTurn`
 * replays exactly that remainder so the next slice attaches at `cursor + 1` with
 * no gap and no duplicate.
 *
 * Capabilities are declared honestly. `supportsBuiltinToolApprovals` defaults to
 * false: an ACP peer's approvals ride the durable `RuntimeInteraction` model
 * (see {@link acpPermissionToRuntimeInteractionPayload}), not the contract's
 * native `submitToolApproval` channel.
 */
export const makeAcpHarnessAdapter = (config: AcpHarnessAdapterConfig): AgentHarness => {
  const harnessId = config.harnessId;
  const adapterKind: AgentRuntimeAdapterKind = config.adapterKind ?? "agent_client_protocol";
  const script = config.script ?? DEFAULT_SCRIPT;
  const builtinTools = config.builtinTools ?? [];
  const supportsSuspend = config.supportsSuspend ?? true;
  const supportsContinue = config.supportsContinue ?? true;
  const supportsCompact = config.supportsCompact ?? true;
  const supportsDetach = config.supportsDetach ?? true;
  const continueIsLossy = config.continueIsLossy ?? false;

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    Effect.gen(function* () {
      const source: KhalaRuntimeSource = options.source;
      const sessionId = options.sessionId;

      const seedSequence = options.continueFrom?.cursor ?? -1;
      const sequenceRef = yield* Ref.make(seedSequence + 1);
      const cursorRef = yield* Ref.make(seedSequence);
      const activeRef = yield* Ref.make<Option.Option<ActiveTurn>>(Option.none());

      let pendingContinuation: ActiveTurn | undefined;
      if (options.continueFrom !== undefined) {
        const data = S.decodeUnknownSync(ContinuationDataSchema)(
          options.continueFrom.data,
        ) as unknown as ContinuationData;
        pendingContinuation = {
          turnId: options.continueFrom.turnId,
          remaining: data.remaining as ReadonlyArray<HarnessStreamEvent>,
        };
        yield* Ref.set(activeRef, Option.some(pendingContinuation));
      }

      const promptTurn = (opts: { readonly turnId: string; readonly prompt: string }) =>
        Effect.gen(function* () {
          const turnId = opts.turnId;
          // Allocate session-global sequences with a local counter, then advance
          // the shared Ref by exactly the count the projection consumed.
          const startSequence = yield* Ref.get(sequenceRef);
          let counter = startSequence;
          const ctx: AcpAdapterContext = {
            turnId,
            threadId: sessionId,
            source,
            nextSequence: () => counter++,
          };
          const events = script.flatMap((event) => acpEventToKhalaEvents(event, ctx));
          yield* Ref.set(sequenceRef, counter);
          yield* Ref.set(activeRef, Option.some({ turnId, remaining: events }));
          return makeControl({
            harnessId,
            sessionId,
            turnId,
            events,
            cursorRef,
            activeRef,
          });
        });

      const continueTurn = () =>
        supportsContinue
          ? Effect.gen(function* () {
              const active = yield* Ref.get(activeRef);
              const turn = Option.getOrUndefined(active) ?? pendingContinuation;
              if (turn === undefined) {
                return yield* Effect.fail(
                  new HarnessTurnError({
                    harnessId,
                    sessionId,
                    turnId: "unknown",
                    failureClass: "no_turn_to_continue",
                  }),
                );
              }
              return makeControl({
                harnessId,
                sessionId,
                turnId: turn.turnId,
                events: turn.remaining,
                cursorRef,
                activeRef,
              });
            })
          : Effect.fail(
              new HarnessCapabilityUnsupported({
                harnessId,
                capability: "continue_turn",
              }),
            );

      const suspendTurn = (): Effect.Effect<
        HarnessContinuationState,
        HarnessCapabilityUnsupported
      > =>
        supportsSuspend
          ? Effect.gen(function* () {
              const cursor = yield* Ref.get(cursorRef);
              const active = yield* Ref.get(activeRef);
              const turn = Option.getOrUndefined(active);
              const turnId = turn?.turnId ?? "unknown";
              const remaining = turn?.remaining ?? [];
              return {
                harnessId,
                sessionId,
                turnId,
                cursor,
                lossy: continueIsLossy,
                data: { turnId, remaining } satisfies ContinuationData,
              };
            })
          : Effect.fail(
              new HarnessCapabilityUnsupported({
                harnessId,
                capability: "suspend_turn",
              }),
            );

      const compact = () =>
        supportsCompact
          ? Effect.void
          : Effect.fail(
              new HarnessCapabilityUnsupported({
                harnessId,
                capability: "compact",
              }),
            );

      const detach = (): Effect.Effect<HarnessResumeState, HarnessCapabilityUnsupported> =>
        supportsDetach
          ? Effect.gen(function* () {
              const cursor = yield* Ref.get(cursorRef);
              return { harnessId, sessionId, data: { detachedAt: cursor } };
            })
          : Effect.fail(
              new HarnessCapabilityUnsupported({
                harnessId,
                capability: "detach",
              }),
            );

      const stop = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          const cursor = yield* Ref.get(cursorRef);
          return { harnessId, sessionId, data: { stoppedAt: cursor } };
        });

      const session: HarnessSession = {
        sessionId,
        isResume: options.resumeFrom !== undefined || options.continueFrom !== undefined,
        modelId: `${harnessId}/acp`,
        promptTurn,
        continueTurn,
        suspendTurn,
        compact,
        detach,
        stop,
        destroy: () => Effect.void,
      };
      return session;
    });

  return {
    specificationVersion: "agent-harness-v1",
    harnessId,
    harnessKind: config.harnessKind,
    adapterKind,
    builtinTools,
    // ACP approvals route through RuntimeInteraction, not the native channel.
    supportsBuiltinToolApprovals: config.supportsBuiltinToolApprovals ?? false,
    supportsBuiltinToolFiltering: config.supportsBuiltinToolFiltering ?? false,
    start,
  };
};
