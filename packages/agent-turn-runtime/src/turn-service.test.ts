import { Effect, Layer, Ref, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { ROUTE_RECOMMENDATION_SCHEMA_LITERAL, RouteRecommendation, TurnIntent } from "@openagentsinc/agent-runtime-schema";

import { ThreadRepository, TurnPolicy, type ThreadTurnMessage } from "./ports.js";
import {
  actionBrokerRecordingLayer,
  artifactResolverFixtureLayer,
  contextSourceFixtureLayer,
  fixtureCandidateSet,
  providerRegistryFixtureLayer,
  testTurnServiceLayer,
  threadRepositoryMemoryLayer,
  turnJournalMemoryLayer,
  turnPolicyFixtureLayer,
  type ProviderFixtureOutcome,
} from "./testing.js";
import { layer as turnServiceLayer, TurnService, type TurnStartInput } from "./turn-service.js";
import { turnRequestRef, turnThreadRef } from "./turn-state.js";

const askIntent = S.decodeUnknownSync(TurnIntent)({ _tag: "Ask", text: "hi" });

const startInput = (): TurnStartInput => ({
  requestRef: turnRequestRef("request.fixture.1"),
  threadRef: turnThreadRef("thread.1"),
  intent: askIntent,
  candidateSet: fixtureCandidateSet,
});

const runStart = (options: Parameters<typeof testTurnServiceLayer>[0]) =>
  Effect.runPromise(
    Effect.gen(function* () {
      const service = yield* TurnService;
      return yield* service.start(startInput());
    }).pipe(Effect.provide(testTurnServiceLayer(options))),
  );

describe("TurnService", () => {
  test("a fixture provider completes deterministically", async () => {
    const result = await runStart({ outcome: "completes" });
    expect(result.projection.cardState).toBe("done");
    expect(result.receipt.decision).toBe("accepted");
    expect(result.candidate).not.toBeNull();
    expect(result.refusal).toBeNull();
  });

  test("a provider chain snapshot reaches the safe projection message chain", async () => {
    const result = await runStart({ outcome: "completes_with_chain" });
    expect(result.projection.cardState).toBe("done");
    expect(result.projection.messageChain.length).toBe(2);
    expect(result.projection.messageChain[0]!.role).toBe("assistant");
    expect(result.projection.messageChain[1]!.toolLabel).toBe("shell");
    // The chain carries labels and counts only — never raw command or output.
    expect(result.projection.messageChain[1]!.commandOutputByteCount).toBe(12);
  });

  test("a fixture provider failure produces a failed terminal that carries the bounded reason", async () => {
    const result = await runStart({ outcome: "fails" });
    expect(result.projection.cardState).toBe("failed");
    expect(result.receipt.decision).toBe("failed");
    expect(result.candidate).toBeNull();
    // The provider's `Failed({ detail })` reason must survive to the safe
    // projection (was dropped before): the card can now show WHAT failed.
    expect(result.projection.failureReason).toBe("fixture failure");
  });

  test("a fixture provider refusal keeps input and shows the reason", async () => {
    const result = await runStart({ outcome: "refuses" });
    expect(result.projection.cardState).toBe("refused");
    expect(result.refusal).toBe("empty_output");
    expect(result.receipt.decision).toBe("rejected");
  });

  test("a closed policy fails closed with no candidate", async () => {
    const result = await runStart({ closedPolicy: true });
    expect(result.projection.cardState).toBe("refused");
    expect(result.refusal).toBe("route_closed_no_candidate");
  });

  test("an unavailable provider refuses without dispatching", async () => {
    const result = await runStart({ outcome: "start_unavailable" });
    expect(result.projection.cardState).toBe("refused");
    expect(result.refusal).toBe("provider_unavailable");
    expect(result.projection.providerTurnRef).toBeUndefined();
  });

  test("a context failure refuses as not ready", async () => {
    const result = await runStart({ failContext: true });
    expect(result.projection.cardState).toBe("refused");
    expect(result.refusal).toBe("not_ready");
  });

  test("cancellation closes the provider run and settles cancelled", async () => {
    const requestRef = turnRequestRef("request.fixture.1");
    const result = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* TurnService;
        const waitRunning = Effect.gen(function* () {
          for (;;) {
            const projection = yield* service.status(requestRef);
            if (projection !== null && projection.cardState === "running") return;
            yield* Effect.yieldNow;
          }
        });
        return yield* Effect.raceFirst(
          service.start(startInput()),
          Effect.gen(function* () {
            yield* waitRunning;
            yield* service.cancel(requestRef);
            return yield* Effect.never;
          }),
        );
      }).pipe(Effect.provide(testTurnServiceLayer({ outcome: "hangs" as ProviderFixtureOutcome }))),
    );
    expect(result.projection.cardState).toBe("cancelled");
    expect(result.receipt.decision).toBe("cancelled");
  });

  test("thread persistence (#9127): a plain turn appends user + attributed assistant; a delegated continuation (recommendation) does not re-append the user message", async () => {
    const fixtureRecommendation = S.decodeUnknownSync(RouteRecommendation)({
      schema: ROUTE_RECOMMENDATION_SCHEMA_LITERAL,
      candidate: "codex",
      taskClass: "delegate",
      reasonCode: "needs_delegation",
      confidence: 0.9,
    });
    const messages = await Effect.runPromise(
      Effect.gen(function* () {
        const recorded = yield* Ref.make<ReadonlyArray<ThreadTurnMessage>>([]);
        const recordingRepository = Layer.succeed(
          ThreadRepository,
          ThreadRepository.of({
            exists: () => Effect.succeed(true),
            appendUser: (_thread, message) => Ref.update(recorded, (all) => [...all, message]),
            appendAssistant: (_thread, message) => Ref.update(recorded, (all) => [...all, message]),
          }),
        );
        const layer = turnServiceLayer.pipe(
          Layer.provide(contextSourceFixtureLayer()),
          Layer.provide(turnPolicyFixtureLayer()),
          Layer.provide(providerRegistryFixtureLayer("completes")),
          Layer.provide(turnJournalMemoryLayer),
          Layer.provide(recordingRepository),
          Layer.provide(artifactResolverFixtureLayer),
          Layer.provide(actionBrokerRecordingLayer),
        );
        yield* Effect.gen(function* () {
          const service = yield* TurnService;
          // The router-style turn: no recommendation → user message persists.
          yield* service.start(startInput());
          // The delegated continuation: SAME user message, started with the
          // router's recommendation → the kernel must not re-append it.
          yield* service.start({
            ...startInput(),
            requestRef: turnRequestRef("request.fixture.2"),
            recommendation: fixtureRecommendation,
          });
        }).pipe(Effect.provide(layer));
        return yield* Ref.get(recorded);
      }),
    );
    // One user note, two assistant answers — never a duplicated user note.
    expect(messages.map((message) => message.role)).toEqual(["user", "assistant", "assistant"]);
    // The assistant answers carry bounded provenance for attribution.
    expect(messages[1]!.provenance).toEqual({
      candidate: "codex",
      model: "codex",
      dataDestination: "remote_provider",
      usageTruth: "exact",
    });
  });

  test("status reconstructs a terminal card from persisted state without replaying an action", async () => {
    const requestRef = turnRequestRef("request.fixture.1");
    const reloaded = await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* TurnService;
        yield* service.start(startInput());
        // A fresh status read models a renderer reload: it derives the card from
        // the persisted record and the provider registry only.
        return yield* service.status(requestRef);
      }).pipe(Effect.provide(testTurnServiceLayer({ outcome: "completes" }))),
    );
    expect(reloaded).not.toBeNull();
    expect(reloaded?.cardState).toBe("done");
  });

  test("the policy receives the registry's MAIN-OWNED descriptors (#9145 readiness-aware routing input)", async () => {
    const seen: { value: ReadonlyArray<{ readonly candidate: string }> | undefined } = { value: undefined };
    // Wrap the fixture policy so the decide input's additive `descriptors` field
    // is observable while the decision itself stays the fixture's.
    const capturingPolicy = Layer.effect(
      TurnPolicy,
      Effect.gen(function* () {
        const fixture = yield* TurnPolicy;
        return TurnPolicy.of({
          decide: (input) =>
            Effect.sync(() => {
              seen.value = input.descriptors;
            }).pipe(Effect.andThen(fixture.decide(input))),
        });
      }),
    ).pipe(Layer.provide(turnPolicyFixtureLayer()));
    const layer = turnServiceLayer.pipe(
      Layer.provide(contextSourceFixtureLayer()),
      Layer.provide(capturingPolicy),
      Layer.provide(providerRegistryFixtureLayer("completes")),
      Layer.provide(turnJournalMemoryLayer),
      Layer.provide(threadRepositoryMemoryLayer),
      Layer.provide(artifactResolverFixtureLayer),
      Layer.provide(actionBrokerRecordingLayer),
    );
    await Effect.runPromise(
      Effect.gen(function* () {
        const service = yield* TurnService;
        yield* service.start(startInput());
      }).pipe(Effect.provide(layer)),
    );
    expect(seen.value).toBeDefined();
    expect(seen.value).toHaveLength(1);
    expect(seen.value![0]!.candidate).toBe("codex");
  });
});
