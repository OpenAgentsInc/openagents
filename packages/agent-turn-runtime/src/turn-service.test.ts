import { Effect, Schema as S } from "effect";
import { describe, expect, test } from "vite-plus/test";

import { TurnIntent } from "@openagentsinc/agent-runtime-schema";

import { fixtureCandidateSet, testTurnServiceLayer, type ProviderFixtureOutcome } from "./testing.js";
import { TurnService, type TurnStartInput } from "./turn-service.js";
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

  test("a fixture provider failure produces a failed terminal", async () => {
    const result = await runStart({ outcome: "fails" });
    expect(result.projection.cardState).toBe("failed");
    expect(result.receipt.decision).toBe("failed");
    expect(result.candidate).toBeNull();
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
});
