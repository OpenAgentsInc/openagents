import { Effect, Option, Ref, Schema as S, Stream } from "effect";
import {
  decodeKhalaRuntimeEvent,
  type KhalaRuntimeFinishReason,
  type KhalaRuntimeSource,
  type KhalaRuntimeUsage,
} from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import { type HarnessToolIdentity, toolIdentity } from "./common-tool.ts";
import { KhalaRuntimeEventSchemaLiteral } from "./stream.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessPromptControl, HarnessSession, HarnessTurnResult } from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * The opencode session event stream, modelled as a small local input type. Each
 * variant mirrors one event in opencode's `session.next.*` / `session.*`
 * inventory (`~/work/projects/repos/opencode` — `packages/schema/src/
 * session-event.ts` and `session-status-event.ts`). This adapter is a
 * PROJECTION from these opencode shapes onto the neutral `KhalaRuntimeEvent`
 * ({@link HarnessStreamEvent}); it does not talk to a live opencode server. The
 * fields kept here are the public-safe, replayable subset each projection
 * actually reads — opencode's live-only stream fragments and raw tool
 * `content`/`input` payloads are intentionally NOT modelled, they never cross
 * into a neutral event.
 */
export type OpencodeEvent =
  | OpencodeTextDelta
  | OpencodeReasoningDelta
  | OpencodeToolCalled
  | OpencodeToolSuccess
  | OpencodeToolFailed
  | OpencodeStepEnded
  | OpencodeSessionIdle;

/** opencode `session.next.text.delta` — a fragment of assistant output text. */
export interface OpencodeTextDelta {
  readonly type: "session.next.text.delta";
  readonly assistantMessageID: string;
  readonly textID: string;
  readonly delta: string;
}

/** opencode `session.next.reasoning.delta` — a fragment of model reasoning. */
export interface OpencodeReasoningDelta {
  readonly type: "session.next.reasoning.delta";
  readonly assistantMessageID: string;
  readonly reasoningID: string;
  readonly delta: string;
}

/**
 * opencode `session.next.tool.called` — the runtime invoked a tool. `tool` is
 * the opencode tool id (`bash`, `read`, `edit`, `webfetch`, …) and
 * `provider.executed` in opencode becomes {@link providerExecuted} here.
 */
export interface OpencodeToolCalled {
  readonly type: "session.next.tool.called";
  readonly assistantMessageID: string;
  readonly callID: string;
  readonly tool: string;
  readonly providerExecuted: boolean;
}

/**
 * opencode `session.next.tool.success`. opencode's success event carries only
 * `callID` (not the tool id), so the projection correlates the id from the
 * earlier `tool.called` through the projection context. `providerExecuted`
 * mirrors opencode `provider.executed`.
 */
export interface OpencodeToolSuccess {
  readonly type: "session.next.tool.success";
  readonly callID: string;
  readonly providerExecuted: boolean;
}

/** opencode `session.next.tool.failed` — a tool call that errored. */
export interface OpencodeToolFailed {
  readonly type: "session.next.tool.failed";
  readonly callID: string;
  readonly messageSafe: string;
  readonly providerExecuted: boolean;
}

/**
 * opencode `session.next.step.ended` — the model step settled with a finish
 * reason and token usage (opencode `tokens: { input, output, reasoning,
 * cache: { read, write } }`). This is the neutral turn-finish carrier: opencode
 * has no explicit "turn started/finished" event, so the adapter synthesizes
 * `turn.started` and derives `turn.finished` (+usage) from this step boundary.
 */
export interface OpencodeStepEnded {
  readonly type: "session.next.step.ended";
  readonly assistantMessageID: string;
  readonly finish: string;
  readonly tokens: {
    readonly input: number;
    readonly output: number;
    readonly reasoning: number;
    readonly cache: { readonly read: number; readonly write: number };
  };
}

/**
 * opencode `session.idle` — the session returned to idle (transport-level
 * end-of-activity). The neutral turn boundary is already carried by
 * `step.ended`, so this projects to no neutral event.
 */
export interface OpencodeSessionIdle {
  readonly type: "session.idle";
}

/**
 * opencode tool ids are lowercase (`bash`, `read`, `write`, `edit`, `glob`,
 * `grep`, `websearch`, `webfetch`, `apply_patch`, …), but the shared
 * {@link commonToolName} map in `common-tool.ts` keys Claude PascalCase and
 * Codex snake_case. This adapter-local alias resolves the opencode id onto a
 * name the shared map recognizes, so `toolIdentity` still owns the
 * native->common vocabulary and a future shared-map addition wins automatically.
 * opencode ids with no shared equivalent (`webfetch`, `task`, `todo`) are left
 * unaliased and forwarded as-is with no common name.
 */
const OPENCODE_TO_SHARED_NATIVE: Readonly<Record<string, string>> = {
  bash: "Bash",
  read: "Read",
  write: "Write",
  edit: "Edit",
  glob: "Glob",
  grep: "Grep",
  websearch: "WebSearch",
  apply_patch: "apply_patch",
};

/**
 * Normalized tool identity for an opencode tool id: the shared common name when
 * one exists, with the true opencode native id always preserved as `nativeName`.
 */
const opencodeToolIdentity = (
  opencodeToolId: string,
  options?: { readonly providerExecuted?: boolean },
): HarnessToolIdentity => {
  const sharedNative = OPENCODE_TO_SHARED_NATIVE[opencodeToolId] ?? opencodeToolId;
  const identity = toolIdentity(sharedNative, options);
  // Preserve opencode's real lowercase id rather than the shared-map alias.
  return { ...identity, nativeName: opencodeToolId };
};

/** opencode finish string -> neutral {@link KhalaRuntimeFinishReason}. */
const OPENCODE_FINISH_TO_KHALA: Readonly<Record<string, KhalaRuntimeFinishReason>> = {
  stop: "stop",
  length: "length",
  "tool-calls": "tool-calls",
  "content-filter": "content-filter",
  error: "error",
  cancelled: "cancelled",
  interrupted: "interrupted",
};

const mapFinishReason = (finish: string): KhalaRuntimeFinishReason =>
  OPENCODE_FINISH_TO_KHALA[finish] ?? "unknown";

/** Context threaded through {@link opencodeEventToKhalaEvents} while folding a stream. */
export interface OpencodeProjectionContext {
  readonly source: KhalaRuntimeSource;
  readonly threadId: string;
  readonly turnId: string;
  /** Allocate the next session-global sequence number. */
  readonly nextSequence: () => number;
  /** callID -> opencode tool id, populated by `tool.called`, read by success/failed. */
  readonly toolNames: Map<string, string>;
}

const base = (ctx: OpencodeProjectionContext, sequence: number, eventSuffix: string) => ({
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

/** Default allow-authority for a projected opencode tool event. */
const toolAuthority = (
  identity: HarnessToolIdentity,
  ctx: OpencodeProjectionContext,
  sequence: number,
) => ({
  authorityRef: `authority.opencode.${identity.wireName}`,
  policyRef: `policy.opencode.${identity.wireName}`,
  decisionRef: `decision.opencode.${ctx.turnId}.${sequence}`,
  toolRef: `tool.opencode.${identity.wireName}`,
  status: "allowed" as const,
  allowed: true,
  blockerRefs: [] as ReadonlyArray<string>,
});

const buildReasoningDelta = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  fields: { readonly messageId: string; readonly text: string },
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "reasoning"),
    kind: "reasoning.delta",
    messageId: fields.messageId,
    chunkId: `chunk.${ctx.turnId}.${sequence}`,
    text: fields.text,
  });

const buildToolCall = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolcall"),
    kind: "tool.call",
    toolCallId: `toolcall.${ctx.turnId}.${sequence}`,
    toolName: identity.wireName,
    inputRef: `input.opencode.${ctx.turnId}.${sequence}`,
    authority: toolAuthority(identity, ctx, sequence),
  });

const buildToolResult = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  providerExecuted: boolean,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolresult"),
    kind: "tool.result",
    toolCallId: `toolcall.${ctx.turnId}.${sequence}`,
    toolName: identity.wireName,
    resultRef: `result.opencode.${ctx.turnId}.${sequence}`,
    authority: toolAuthority(identity, ctx, sequence),
    providerExecuted,
  });

const buildToolError = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  identity: HarnessToolIdentity,
  messageSafe: string,
  providerExecuted: boolean,
): HarnessStreamEvent =>
  decodeKhalaRuntimeEvent({
    ...base(ctx, sequence, "toolerror"),
    kind: "tool.error",
    toolCallId: `toolcall.${ctx.turnId}.${sequence}`,
    toolName: identity.wireName,
    errorRef: `error.opencode.${ctx.turnId}.${sequence}`,
    messageSafe,
    authority: toolAuthority(identity, ctx, sequence),
    providerExecuted,
  });

const stepUsage = (
  ctx: OpencodeProjectionContext,
  sequence: number,
  ev: OpencodeStepEnded,
): KhalaRuntimeUsage => ({
  usageRef: `usage.opencode.${ctx.turnId}.${sequence}`,
  inputTokens: ev.tokens.input,
  outputTokens: ev.tokens.output,
  reasoningTokens: ev.tokens.reasoning,
  cacheReadInputTokens: ev.tokens.cache.read,
  cacheWriteInputTokens: ev.tokens.cache.write,
  totalTokens: ev.tokens.input + ev.tokens.output + ev.tokens.reasoning,
});

/**
 * Pure projection of ONE opencode event onto zero or more neutral
 * {@link HarnessStreamEvent}s. Sequence numbers come from `ctx.nextSequence`, so
 * a caller folds a whole opencode stream while keeping session-global cursors
 * contiguous. Tool ids are normalized through {@link opencodeToolIdentity} so
 * `bash`/`read`/… land on the shared common vocabulary.
 */
export const opencodeEventToKhalaEvents = (
  event: OpencodeEvent,
  ctx: OpencodeProjectionContext,
): ReadonlyArray<HarnessStreamEvent> => {
  switch (event.type) {
    case "session.next.text.delta":
      return [
        buildTextDelta({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence: ctx.nextSequence(),
          source: ctx.source,
          messageId: event.assistantMessageID,
          text: event.delta,
        }),
      ];
    case "session.next.reasoning.delta":
      return [
        buildReasoningDelta(ctx, ctx.nextSequence(), {
          messageId: event.assistantMessageID,
          text: event.delta,
        }),
      ];
    case "session.next.tool.called": {
      ctx.toolNames.set(event.callID, event.tool);
      const identity = opencodeToolIdentity(event.tool, {
        providerExecuted: event.providerExecuted,
      });
      return [buildToolCall(ctx, ctx.nextSequence(), identity)];
    }
    case "session.next.tool.success": {
      const opencodeToolId = ctx.toolNames.get(event.callID) ?? event.callID;
      const identity = opencodeToolIdentity(opencodeToolId, {
        providerExecuted: event.providerExecuted,
      });
      return [buildToolResult(ctx, ctx.nextSequence(), identity, event.providerExecuted)];
    }
    case "session.next.tool.failed": {
      const opencodeToolId = ctx.toolNames.get(event.callID) ?? event.callID;
      const identity = opencodeToolIdentity(opencodeToolId, {
        providerExecuted: event.providerExecuted,
      });
      return [
        buildToolError(
          ctx,
          ctx.nextSequence(),
          identity,
          event.messageSafe,
          event.providerExecuted,
        ),
      ];
    }
    case "session.next.step.ended": {
      const sequence = ctx.nextSequence();
      return [
        buildTurnFinished({
          turnId: ctx.turnId,
          threadId: ctx.threadId,
          sequence,
          source: ctx.source,
          finishReason: mapFinishReason(event.finish),
          usage: stepUsage(ctx, sequence, event),
        }),
      ];
    }
    case "session.idle":
      // The neutral turn boundary is carried by `step.ended`; idle is transport-level.
      return [];
  }
};

/** Configuration for {@link makeOpencodeAdapter}. */
export interface OpencodeAdapterConfig {
  readonly harnessId?: string;
  /** The scripted opencode event stream each prompt turn replays (fixture, no live server). */
  readonly script?: ReadonlyArray<OpencodeEvent>;
  readonly supportsSuspend?: boolean;
  readonly supportsContinue?: boolean;
  readonly supportsCompact?: boolean;
  readonly supportsDetach?: boolean;
  /** When true, `continueTurn` reports the continuation as a re-driven (lossy) tail. */
  readonly continueIsLossy?: boolean;
}

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
      detail: "opencode adapter has no tool call awaiting a result",
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
 * Build an opencode {@link AgentHarness}. It scripts each turn as
 * `turn.started -> <projected opencode events> -> turn.finished` with
 * session-global contiguous sequence numbers, projecting a fixture opencode
 * event stream through {@link opencodeEventToKhalaEvents} so the unit test needs
 * no live opencode server. Suspend/continue is lossless and cursor-exact,
 * mirroring the reference adapter. Capabilities are declared honestly and a
 * refused verb fails with {@link HarnessCapabilityUnsupported}.
 */
export const makeOpencodeAdapter = (config: OpencodeAdapterConfig = {}): AgentHarness => {
  const harnessId = config.harnessId ?? "opencode";
  const script: ReadonlyArray<OpencodeEvent> = config.script ?? [
    {
      type: "session.next.text.delta",
      assistantMessageID: "msg_opencode_1",
      textID: "text_1",
      delta: "Hello ",
    },
    {
      type: "session.next.text.delta",
      assistantMessageID: "msg_opencode_1",
      textID: "text_1",
      delta: "world",
    },
    {
      type: "session.next.step.ended",
      assistantMessageID: "msg_opencode_1",
      finish: "stop",
      tokens: { input: 12, output: 5, reasoning: 0, cache: { read: 0, write: 0 } },
    },
  ];
  const supportsSuspend = config.supportsSuspend ?? true;
  const supportsContinue = config.supportsContinue ?? true;
  const supportsCompact = config.supportsCompact ?? true;
  const supportsDetach = config.supportsDetach ?? true;
  const continueIsLossy = config.continueIsLossy ?? false;

  const start = (options: HarnessStartOptions): Effect.Effect<HarnessSession, HarnessStartError> =>
    Effect.gen(function* () {
      const source: KhalaRuntimeSource = options.source;
      const sessionId = options.sessionId;

      // Session-global monotonic sequence. A resumed session keeps counting from
      // where the export left off so cursors stay globally ordered.
      const seedSequence = options.continueFrom?.cursor ?? -1;
      const sequenceRef = yield* Ref.make(seedSequence + 1);
      const cursorRef = yield* Ref.make(seedSequence);
      const activeRef = yield* Ref.make<Option.Option<ActiveTurn>>(Option.none());

      // A continuation-started session pre-loads the remaining turn script.
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
          const events: Array<HarnessStreamEvent> = [];

          // opencode has no explicit turn-started event; synthesize it.
          const s0 = yield* Ref.getAndUpdate(sequenceRef, (n) => n + 1);
          events.push(buildTurnStarted({ turnId, threadId: sessionId, sequence: s0, source }));

          // Fold the fixture opencode stream through the pure projection, drawing
          // contiguous sequence numbers from the session-global allocator.
          const seqBox = { value: yield* Ref.get(sequenceRef) };
          const ctx: OpencodeProjectionContext = {
            source,
            threadId: sessionId,
            turnId,
            nextSequence: () => seqBox.value++,
            toolNames: new Map<string, string>(),
          };
          for (const opencodeEvent of script) {
            events.push(...opencodeEventToKhalaEvents(opencodeEvent, ctx));
          }
          yield* Ref.set(sequenceRef, seqBox.value);

          yield* Ref.set(activeRef, Option.some({ turnId, remaining: events }));
          return makeControl({ harnessId, sessionId, turnId, events, cursorRef, activeRef });
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
              new HarnessCapabilityUnsupported({ harnessId, capability: "continue_turn" }),
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
              new HarnessCapabilityUnsupported({ harnessId, capability: "suspend_turn" }),
            );

      const compact = () =>
        supportsCompact
          ? Effect.void
          : Effect.fail(new HarnessCapabilityUnsupported({ harnessId, capability: "compact" }));

      const detach = (): Effect.Effect<HarnessResumeState, HarnessCapabilityUnsupported> =>
        supportsDetach
          ? Effect.gen(function* () {
              const cursor = yield* Ref.get(cursorRef);
              return { harnessId, sessionId, data: { detachedAt: cursor } };
            })
          : Effect.fail(new HarnessCapabilityUnsupported({ harnessId, capability: "detach" }));

      const stop = (): Effect.Effect<HarnessResumeState> =>
        Effect.gen(function* () {
          const cursor = yield* Ref.get(cursorRef);
          return { harnessId, sessionId, data: { stoppedAt: cursor } };
        });

      const session: HarnessSession = {
        sessionId,
        isResume: options.resumeFrom !== undefined || options.continueFrom !== undefined,
        modelId: "opencode/scripted",
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
    harnessKind: "opencode",
    adapterKind: "opencode",
    // opencode's native tool ids (lowercase) with their shared common names.
    builtinTools: [
      { nativeName: "bash", commonName: "bash", description: "run a shell command" },
      { nativeName: "read", commonName: "read", description: "read a file" },
      { nativeName: "write", commonName: "write", description: "write a file" },
      { nativeName: "edit", commonName: "edit", description: "edit a file" },
      { nativeName: "glob", commonName: "glob", description: "match paths by glob" },
      { nativeName: "grep", commonName: "grep", description: "search file contents" },
      { nativeName: "websearch", commonName: "webSearch", description: "search the web" },
      { nativeName: "webfetch", description: "fetch a URL" },
    ],
    supportsBuiltinToolApprovals: false,
    supportsBuiltinToolFiltering: false,
    start,
  };
};
