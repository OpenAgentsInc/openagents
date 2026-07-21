import {
  Context,
  Deferred,
  Effect,
  Layer,
  PubSub,
  Ref,
  Schema as S,
  Stream,
} from "effect";

import {
  RouteDecisionRef,
  type OwnerBoundCandidateSet,
  type RouteRecommendation,
  type SafeMessageChainEntry,
  type SafeTurnProjection,
  type TurnCandidate,
  type TurnDataDestination,
  type TurnIntent,
  type TurnProviderCandidate,
  type TurnReceipt,
  type TurnRefusalReason,
  type TurnRequestRef,
  type TurnThreadRef,
  type TurnUsageTruth,
} from "@openagentsinc/agent-runtime-schema";

import { makeTurnEventGateway } from "./event-gateway.js";
import {
  ActionBroker,
  ContextSource,
  ProviderRegistry,
  ProviderStreamEvent,
  TurnJournal,
  TurnPolicy,
  ThreadRepository,
} from "./ports.js";
import { buildTurnReceipt, deriveSafeProjection, safeFailureReasonText } from "./projection.js";
import {
  applyTurnEvent,
  bumpTurnGeneration,
  initialTurnState,
  TurnTransition,
  type TurnStateRecord,
} from "./turn-state.js";

/**
 * AFS-01 `TurnService` — the shared, UI-neutral turn kernel.
 *
 * It owns the canonical turn lifecycle: decode intent, resolve context, request a
 * host route decision, start one admitted provider, fold bounded advisory events
 * through a generation-fenced gateway, record terminal state, and publish a safe
 * projection. It owns no provider credentials, no file mutation, no task/debug/Git
 * execution, no artifact promotion, and no release. Electron main composes it as
 * the first production composition; web and mobile consume its safe facts.
 */

/** Bounded capacity of the main-owned provider event gateway. */
export const TURN_EVENT_GATEWAY_CAPACITY = 256 as const;

const decodeRouteDecisionRef = S.decodeUnknownSync(RouteDecisionRef);

/** Input the host hands the kernel to start one turn. */
export interface TurnStartInput {
  readonly requestRef: TurnRequestRef;
  readonly threadRef: TurnThreadRef;
  readonly intent: TurnIntent;
  readonly candidateSet: OwnerBoundCandidateSet;
  readonly recommendation?: RouteRecommendation | null;
}

/** The terminal result of one kernel turn. */
export interface TurnKernelResult {
  readonly candidate: TurnCandidate | null;
  readonly receipt: TurnReceipt;
  readonly projection: SafeTurnProjection;
  readonly refusal: TurnRefusalReason | null;
  readonly generation: number;
}

/** A published progress frame. It carries the generation for renderer fencing. */
export interface TurnProgressFrame {
  readonly requestRef: TurnRequestRef;
  readonly generation: number;
  readonly projection: SafeTurnProjection;
}

export interface TurnServiceInterface {
  readonly start: (input: TurnStartInput) => Effect.Effect<TurnKernelResult>;
  readonly cancel: (requestRef: TurnRequestRef) => Effect.Effect<void>;
  readonly status: (requestRef: TurnRequestRef) => Effect.Effect<SafeTurnProjection | null>;
  readonly progress: Stream.Stream<TurnProgressFrame>;
}

export class TurnService extends Context.Service<TurnService, TurnServiceInterface>()(
  "agent-turn-runtime.TurnService",
) {}

interface EffectiveFacts {
  readonly candidate: TurnProviderCandidate | undefined;
  readonly dataDestination: TurnDataDestination;
  readonly usageTruth: TurnUsageTruth;
  readonly localOnly: boolean;
}

const DEFAULT_FACTS: EffectiveFacts = {
  candidate: undefined,
  dataDestination: "on_device_local",
  usageTruth: "unknown",
  localOnly: true,
};

const providerStartReason = (
  reason: "unavailable" | "unauthorized" | "unadmitted" | "not_ready" | "helper_missing",
): TurnRefusalReason => {
  switch (reason) {
    case "unavailable":
      return "provider_unavailable";
    case "unauthorized":
      return "provider_unauthorized";
    case "unadmitted":
      return "provider_unadmitted";
    case "not_ready":
      return "not_ready";
    case "helper_missing":
      return "helper_missing";
  }
};

export const layer = Layer.effect(
  TurnService,
  Effect.gen(function* () {
    const context = yield* ContextSource;
    const policy = yield* TurnPolicy;
    const registry = yield* ProviderRegistry;
    const journal = yield* TurnJournal;
    const threads = yield* ThreadRepository;
    const broker = yield* ActionBroker;

    const cancelSignals = yield* Ref.make(new Map<string, Deferred.Deferred<void>>());
    const latest = yield* Ref.make(new Map<string, SafeTurnProjection>());
    const progress = yield* PubSub.unbounded<TurnProgressFrame>();

    const timestamp = Effect.map(
      Effect.clockWith((clock) => clock.currentTimeMillis),
      (millis) => new Date(millis).toISOString(),
    );

    /** Persist (best effort) and publish a projection for the current record. */
    const publish = (
      record: TurnStateRecord,
      facts: EffectiveFacts,
      messageChain: ReadonlyArray<SafeMessageChainEntry>,
    ) =>
      Effect.gen(function* () {
        yield* journal.record(record).pipe(Effect.catch(() => Effect.void));
        const updatedAt = yield* timestamp;
        const projection = deriveSafeProjection({
          record,
          dataDestination: facts.dataDestination,
          usageTruth: facts.usageTruth,
          localOnly: facts.localOnly,
          ...(facts.candidate === undefined ? {} : { candidate: facts.candidate }),
          messageChain,
          updatedAt,
        });
        yield* Ref.update(latest, (map) => new Map(map).set(record.requestRef, projection));
        yield* PubSub.publish(progress, {
          requestRef: record.requestRef,
          generation: record.generation,
          projection,
        });
        return projection;
      });

    const start = Effect.fn("TurnService.start")(function* (input: TurnStartInput) {
      const requestKey = input.requestRef;
      const cancelDeferred = yield* Deferred.make<void>();
      yield* Ref.update(cancelSignals, (map) => new Map(map).set(requestKey, cancelDeferred));

      const stateRef = yield* Ref.make(initialTurnState(input.requestRef, input.threadRef));
      const lastCandidate = yield* Ref.make<TurnCandidate | null>(null);
      const factsRef = yield* Ref.make<EffectiveFacts>(DEFAULT_FACTS);
      // The latest redacted safe message chain the provider adapter reported.
      const chainRef = yield* Ref.make<ReadonlyArray<SafeMessageChainEntry>>([]);

      const applyAt = (generation: number, transition: TurnTransition) =>
        Effect.gen(function* () {
          const record = yield* Ref.get(stateRef);
          const outcome = applyTurnEvent(record, { generation, transition });
          if (outcome.ok) {
            yield* Ref.set(stateRef, outcome.record);
            const facts = yield* Ref.get(factsRef);
            const chain = yield* Ref.get(chainRef);
            yield* publish(outcome.record, facts, chain);
          }
          return outcome;
        });

      const applyNow = (transition: TurnTransition) =>
        Ref.get(stateRef).pipe(Effect.flatMap((record) => applyAt(record.generation, transition)));

      const terminalResult = (routeDecisionRef: RouteDecisionRef): Effect.Effect<TurnKernelResult> =>
        Effect.gen(function* () {
          const record = yield* Ref.get(stateRef);
          const facts = yield* Ref.get(factsRef);
          const messageChain = yield* Ref.get(chainRef);
          const updatedAt = yield* timestamp;
          const projection = deriveSafeProjection({
            record,
            dataDestination: facts.dataDestination,
            usageTruth: facts.usageTruth,
            localOnly: facts.localOnly,
            ...(facts.candidate === undefined ? {} : { candidate: facts.candidate }),
            messageChain,
            updatedAt,
          });
          const receipt = buildTurnReceipt({
            record,
            routeDecisionRef,
            usageTruth: facts.usageTruth,
          });
          const candidate = yield* Ref.get(lastCandidate);
          return { candidate, receipt, projection, refusal: record.refusalReason, generation: record.generation };
        });

      const foldProviderEvent = (generation: number, event: ProviderStreamEvent) =>
        ProviderStreamEvent.$match(event, {
          Progress: () => applyAt(generation, TurnTransition.Progress()),
          // A chain snapshot never advances the lifecycle; it republishes the
          // current record with the latest redacted message chain.
          Chain: ({ entries }) =>
            Effect.gen(function* () {
              yield* Ref.set(chainRef, entries);
              const record = yield* Ref.get(stateRef);
              // Fence a late snapshot from a superseded generation.
              if (record.generation !== generation) return;
              const facts = yield* Ref.get(factsRef);
              yield* publish(record, facts, entries);
            }),
          Completed: ({ candidate }) =>
            Effect.gen(function* () {
              const outcome = yield* applyAt(
                generation,
                TurnTransition.Completed({ candidateRef: candidate.candidateRef }),
              );
              if (outcome.ok) {
                yield* Ref.set(lastCandidate, candidate);
                // Persist canonical assistant turn state through the thread authority.
                // The bounded provenance facts ride along so the host adapter can
                // keep the answer attributed (#9127) — e.g. a delegated subagent
                // answer stays labeled after reload.
                if (candidate.kind === "answer") {
                  yield* threads
                    .appendAssistant(input.threadRef, {
                      role: "assistant",
                      text: candidate.text,
                      provenance: {
                        candidate: candidate.provenance.candidate,
                        model: candidate.provenance.model,
                        dataDestination: candidate.provenance.dataDestination,
                        usageTruth: candidate.provenance.usageTruth,
                      },
                    })
                    .pipe(Effect.catch(() => Effect.void));
                }
                // Advisory delivery only. The broker converts the delivery into a
                // typed action REQUEST for the owning IDE service and records the
                // backlink; it never performs a host action here. It is fail-soft:
                // an action-routing failure must never break the advisory turn.
                yield* broker
                  .deliver({
                    candidate,
                    intent: input.intent,
                    threadRef: input.threadRef,
                    requestRef: input.requestRef,
                  })
                  .pipe(Effect.catch(() => Effect.void));
              }
            }),
          Refused: ({ reason }) => applyAt(generation, TurnTransition.Refused({ reason })),
          // Carry the provider's bounded, public-safe failure reason onto the
          // record so the terminal projection (and the delegation card that
          // consumes it) can show WHAT failed, not a bare ERRORED badge.
          Failed: ({ detail }) =>
            applyAt(generation, TurnTransition.Failed({ reason: safeFailureReasonText(detail) })),
        });

      const run = Effect.gen(function* () {
        yield* applyNow(TurnTransition.RouteStarted());

        // Persist the canonical user turn state through the thread authority.
        // A turn started WITH an advisory recommendation is a delegated
        // continuation of a router turn on the same thread (#9127): the router
        // turn already appended this user message, so re-appending it here would
        // duplicate the user note in the canonical thread store.
        if (input.intent._tag === "Ask" && (input.recommendation ?? null) === null) {
          yield* threads
            .appendUser(input.threadRef, { role: "user", text: input.intent.text })
            .pipe(Effect.catch(() => Effect.void));
        }

        const contextResult = yield* context.manifest({ threadRef: input.threadRef, intent: input.intent }).pipe(
          Effect.map((envelope) => ({ ok: true as const, envelope })),
          Effect.catch(() => Effect.succeed({ ok: false as const })),
        );
        if (!contextResult.ok) {
          yield* applyNow(TurnTransition.Refused({ reason: "not_ready" }));
          return yield* terminalResult(decodeRouteDecisionRef(`route.${input.requestRef}.context`));
        }
        const envelope = contextResult.envelope;

        const decision = yield* policy.decide({
          requestRef: input.requestRef,
          intent: input.intent,
          context: envelope,
          candidateSet: input.candidateSet,
          recommendation: input.recommendation ?? null,
        });

        if (decision.outcome === "closed") {
          yield* applyNow(TurnTransition.RouteClosed());
          return yield* terminalResult(decision.routeDecisionRef);
        }

        yield* applyNow(
          TurnTransition.RouteAdmitted({ selected: decision.selected, effective: decision.effective }),
        );

        const descriptors = yield* registry.describe;
        const effective = descriptors.find((d) => d.providerRef === decision.effective);
        const facts: EffectiveFacts = effective
          ? {
              candidate: effective.candidate,
              dataDestination: effective.dataDestination,
              usageTruth: effective.usageTruth,
              localOnly: decision.disclosure.localOnly,
            }
          : {
              candidate: undefined,
              dataDestination: decision.disclosure.dataDestination,
              usageTruth: "unknown",
              localOnly: decision.disclosure.localOnly,
            };
        yield* Ref.set(factsRef, facts);

        const startResult = yield* registry
          .start({
            providerRef: decision.effective,
            requestRef: input.requestRef,
            threadRef: input.threadRef,
            intent: input.intent,
            context: envelope,
          })
          .pipe(
            Effect.map((run) => ({ ok: true as const, run })),
            Effect.catch((error) => Effect.succeed({ ok: false as const, reason: error.reason })),
          );
        if (!startResult.ok) {
          yield* applyNow(TurnTransition.Refused({ reason: providerStartReason(startResult.reason) }));
          return yield* terminalResult(decision.routeDecisionRef);
        }
        const providerRun = startResult.run;

        yield* applyNow(TurnTransition.ProviderStarted({ providerTurnRef: providerRun.providerTurnRef }));
        const runGeneration = (yield* Ref.get(stateRef)).generation;

        const gateway = yield* makeTurnEventGateway<ProviderStreamEvent>(
          TURN_EVENT_GATEWAY_CAPACITY,
          runGeneration,
        );

        // Producer: pipe provider events into the bounded, fenced gateway. The
        // scoped fiber is interrupted when the run scope closes.
        yield* providerRun.events.pipe(
          Stream.runForEach((event) => gateway.offer(runGeneration, event)),
          Effect.forkScoped,
        );

        // Consumer: fold gateway events until the first terminal provider event.
        // `Progress` and `Chain` events continue; `Completed`, `Refused`, and
        // `Failed` stop the drain after they are folded.
        const consumer = gateway.stream.pipe(
          Stream.takeUntil(
            (event) =>
              event._tag === "Completed" || event._tag === "Refused" || event._tag === "Failed",
          ),
          Stream.runForEach((event) => foldProviderEvent(runGeneration, event)),
        );

        const onCancel = Deferred.await(cancelDeferred).pipe(
          Effect.flatMap(() =>
            Effect.gen(function* () {
              // Supersede the generation so late provider events fence out, then
              // cancel and close the transport. The scoped provider is released
              // when the run scope closes.
              yield* Ref.update(stateRef, bumpTurnGeneration);
              const superseded = (yield* Ref.get(stateRef)).generation;
              yield* gateway.setGeneration(superseded);
              yield* applyNow(TurnTransition.Cancelled());
            }),
          ),
        );

        yield* Effect.raceFirst(consumer, onCancel);
        return yield* terminalResult(decision.routeDecisionRef);
      });

      return yield* Effect.scoped(run).pipe(
        Effect.ensuring(Ref.update(cancelSignals, (map) => {
          const next = new Map(map);
          next.delete(requestKey);
          return next;
        })),
      );
    });

    const cancel = Effect.fn("TurnService.cancel")(function* (requestRef: TurnRequestRef) {
      const map = yield* Ref.get(cancelSignals);
      const deferred = map.get(requestRef);
      if (deferred !== undefined) yield* Deferred.succeed(deferred, undefined);
    });

    const status = Effect.fn("TurnService.status")(function* (requestRef: TurnRequestRef) {
      const live = (yield* Ref.get(latest)).get(requestRef);
      if (live !== undefined) return live;
      // Reload path: reconstruct the terminal card from persisted state and the
      // provider registry, without replaying any action.
      const record = yield* journal.load(requestRef).pipe(Effect.catch(() => Effect.succeed(null)));
      if (record === null) return null;
      const descriptors = yield* registry.describe;
      const effective = record.effective;
      const descriptor = effective === null ? undefined : descriptors.find((d) => d.providerRef === effective);
      const facts: EffectiveFacts = descriptor
        ? {
            candidate: descriptor.candidate,
            dataDestination: descriptor.dataDestination,
            usageTruth: descriptor.usageTruth,
            localOnly: descriptor.dataDestination === "on_device_local",
          }
        : DEFAULT_FACTS;
      const updatedAt = yield* timestamp;
      return deriveSafeProjection({
        record,
        dataDestination: facts.dataDestination,
        usageTruth: facts.usageTruth,
        localOnly: facts.localOnly,
        ...(facts.candidate === undefined ? {} : { candidate: facts.candidate }),
        updatedAt,
      });
    });

    return TurnService.of({
      start,
      cancel,
      status,
      progress: Stream.fromPubSub(progress),
    });
  }),
);
