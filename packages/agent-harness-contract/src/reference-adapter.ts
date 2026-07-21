import { Effect, Option, Ref, Schema as S, Stream } from "effect";
import type { KhalaRuntimeSource } from "@openagentsinc/agent-runtime-schema";
import type { AgentHarness, HarnessStartOptions } from "./adapter.ts";
import { HarnessStartError } from "./adapter.ts";
import { HarnessCapabilityUnsupported } from "./capability.ts";
import { buildTextDelta, buildTurnFinished, buildTurnStarted } from "./event-builder.ts";
import type { HarnessContinuationState, HarnessResumeState } from "./lifecycle-state.ts";
import type { HarnessPromptControl, HarnessSession, HarnessTurnResult } from "./session.ts";
import { HarnessTurnError } from "./session.ts";
import type { HarnessStreamEvent } from "./stream.ts";

/**
 * In-memory reference adapter used to exercise the contract in conformance
 * tests. It scripts each turn as `turn.started -> text.delta* -> turn.finished`
 * with contiguous session-global sequence numbers, and models lossless
 * suspend/continue: `suspendTurn` returns the cursor of the last event the
 * consumer pulled plus the remaining script, and `continueTurn` replays exactly
 * that remainder — so the next slice attaches at `cursor + 1` with no gap and no
 * duplicate.
 *
 * Capabilities are configurable so a test can build a variant that refuses a
 * verb and assert the fail-closed `HarnessCapabilityUnsupported` path.
 */
export interface ReferenceAdapterConfig {
  readonly harnessId?: string;
  /** Words emitted as `text.delta` events for each prompt turn. */
  readonly scriptWords?: ReadonlyArray<string>;
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
      detail: "reference adapter has no tool call awaiting a result",
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

/** Build the in-memory reference {@link AgentHarness}. */
export const makeReferenceAdapter = (config: ReferenceAdapterConfig = {}): AgentHarness => {
  const harnessId = config.harnessId ?? "reference";
  const scriptWords = config.scriptWords ?? ["Hello ", "world"];
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

      const nextSeq = () => Ref.getAndUpdate(sequenceRef, (n) => n + 1);

      const promptTurn = (opts: { readonly turnId: string; readonly prompt: string }) =>
        Effect.gen(function* () {
          const turnId = opts.turnId;
          const events: Array<HarnessStreamEvent> = [];
          const s0 = yield* nextSeq();
          events.push(
            buildTurnStarted({
              turnId,
              threadId: sessionId,
              sequence: s0,
              source,
            }),
          );
          const messageId = `msg.${turnId}`;
          for (const word of scriptWords) {
            const s = yield* nextSeq();
            events.push(
              buildTextDelta({
                turnId,
                threadId: sessionId,
                sequence: s,
                source,
                messageId,
                text: word,
              }),
            );
          }
          const sf = yield* nextSeq();
          events.push(
            buildTurnFinished({
              turnId,
              threadId: sessionId,
              sequence: sf,
              source,
              finishReason: "stop",
            }),
          );
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
        modelId: "reference/scripted",
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
    harnessKind: "test_fixture",
    adapterKind: "test_fixture",
    builtinTools: [{ nativeName: "Bash", commonName: "bash", description: "run a shell command" }],
    supportsBuiltinToolApprovals: false,
    supportsBuiltinToolFiltering: false,
    start,
  };
};
