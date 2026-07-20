import { describe, expect, test } from "vite-plus/test";
import { Deferred, Duration, Effect, Fiber, Layer, Schema as S } from "effect";
import { TestClock } from "effect/testing";

import {
  AMBIENT_TASK_BOUNDS,
  AMBIENT_TASK_KINDS,
  AmbientInference,
  AmbientResourceGate,
  AmbientTaskProvenance,
  AmbientTaskRunner,
  AmbientTaskRunnerLayer,
  ambientTaskCatalog,
  BootExplanationInput,
  bootExplanationSignature,
  runAmbientTaskDetached,
  type AmbientInferenceShape,
  type AmbientResourceGateShape,
  type AmbientTaskSignature,
  type BootExplanationInput as BootExplanationInputType,
  type AmbientExplanationOutput,
} from "./ambient-task.js";
import { AMBIENT_CORPUS_EVALUATIONS } from "./ambient-task-corpus.js";
import { fixedAmbientInference, makeFakeAmbientInference, makeFakeAmbientResourceGate } from "./testing.js";
import type { AppleFmCompletionTurn } from "./client.js";

const bootFacts = S.decodeUnknownSync(BootExplanationInput)({
  bootRef: "boot.test.1",
  bootSequenceText: "Runtime gateway ready. Providers: codex ready, claude ready, apple_fm ready.",
});

const readyGate = makeFakeAmbientResourceGate();

const runnerLayer = (
  inference: AmbientInferenceShape,
  gate: AmbientResourceGateShape = readyGate,
): Layer.Layer<AmbientTaskRunner> =>
  AmbientTaskRunnerLayer.pipe(
    Layer.provide(Layer.succeed(AmbientInference, AmbientInference.of(inference))),
    Layer.provide(Layer.succeed(AmbientResourceGate, AmbientResourceGate.of(gate))),
  );

const runBoot = (
  inference: AmbientInferenceShape,
  gate: AmbientResourceGateShape = readyGate,
) =>
  AmbientTaskRunner.pipe(
    Effect.flatMap((runner) => runner.run({ signature: bootExplanationSignature, facts: bootFacts })),
    Effect.provide(runnerLayer(inference, gate)),
  );

const goodBoot = "The runtime gateway started and all three providers, including on-device Apple FM, are ready.";

describe("AFS-07 ambient task quality corpus", () => {
  test("each ambient task passes its own quality corpus", async () => {
    const kinds = new Set<string>();
    for (const evaluation of AMBIENT_CORPUS_EVALUATIONS) {
      const results = await Effect.runPromise(evaluation);
      expect(results.length).toBeGreaterThan(0);
      for (const result of results) {
        kinds.add(result.kind);
        expect(result.passed, `${result.kind}/${result.name}: ${result.detail}`).toBe(true);
      }
    }
    // The corpus covers every ambient task kind.
    expect([...kinds].sort()).toEqual([...AMBIENT_TASK_KINDS].sort());
  });

  test("the catalog exposes all seven kinds with separate bounds", () => {
    expect(ambientTaskCatalog.length).toBe(7);
    expect(new Set(ambientTaskCatalog.map((entry) => entry.kind)).size).toBe(7);
    // Separate thermal bounds: at least one task is nominal-ceiling and one is fair.
    const ceilings = new Set(ambientTaskCatalog.map((entry) => entry.bounds.thermalCeiling));
    expect(ceilings.has("nominal")).toBe(true);
    expect(ceilings.has("fair")).toBe(true);
    // Every kind has a bounds row with positive input/time/output/concurrency.
    for (const kind of AMBIENT_TASK_KINDS) {
      const bounds = AMBIENT_TASK_BOUNDS[kind];
      expect(bounds.maxInputChars).toBeGreaterThan(0);
      expect(bounds.timeoutMs).toBeGreaterThan(0);
      expect(bounds.maxOutputChars).toBeGreaterThan(0);
      expect(bounds.maxConcurrency).toBeGreaterThanOrEqual(1);
    }
  });
});

describe("AFS-07 advisory, on-device, zero-token provenance", () => {
  test("a completed task records advisory on-device provenance with honest usage truth", async () => {
    const outcome = await Effect.runPromise(runBoot(fixedAmbientInference(goodBoot)));
    expect(outcome._tag).toBe("Completed");
    if (outcome._tag !== "Completed") return;
    const { provenance } = outcome;
    expect(provenance.advisory).toBe(true);
    expect(provenance.dataDestination).toBe("on_device_local");
    expect(provenance.costClass).toBe("local_resource_only");
    expect(provenance.usageTruth).toBe("estimated");
    expect(provenance.factRefs).toContain("boot.test.1");
    // The provenance round-trips through its schema.
    expect(() => S.decodeUnknownSync(AmbientTaskProvenance)(provenance)).not.toThrow();
  });

  test("no local ambient result carries a provider token row", async () => {
    const outcome = await Effect.runPromise(runBoot(fixedAmbientInference(goodBoot)));
    expect(outcome._tag).toBe("Completed");
    if (outcome._tag !== "Completed") return;
    const provenanceKeys = Object.keys(outcome.provenance);
    // Zero-token invariant: no token-count / accounting fields ever leak into the
    // advisory provenance; usage stays a truth label only.
    for (const forbidden of ["promptTokens", "completionTokens", "totalTokens", "tokens", "cost", "usd", "amount"]) {
      expect(provenanceKeys).not.toContain(forbidden);
    }
    expect(["estimated", "unknown"]).toContain(outcome.provenance.usageTruth);
  });

  test("usage truth passes through unknown and only reports exact when the bridge supplies it", async () => {
    const unknownTurn: AppleFmCompletionTurn = { outcome: "completed", text: goodBoot, usageTruth: "unknown" };
    const unknown = await Effect.runPromise(runBoot(makeFakeAmbientInference(() => unknownTurn)));
    expect(unknown._tag === "Completed" && unknown.provenance.usageTruth).toBe("unknown");

    const exactTurn: AppleFmCompletionTurn = {
      outcome: "completed",
      text: goodBoot,
      usageTruth: "exact",
      totalTokens: 12,
    };
    const exact = await Effect.runPromise(runBoot(makeFakeAmbientInference(() => exactTurn)));
    // Only a bridge-supplied exact turn yields exact truth; the runner never invents it.
    expect(exact._tag === "Completed" && exact.provenance.usageTruth).toBe("exact");
    // Even then, the runner surfaces no raw token count as a billing row.
    if (exact._tag === "Completed") {
      expect(Object.keys(exact.provenance)).not.toContain("totalTokens");
    }
  });

  test("a refused output keeps advisory provenance and the exact reason", async () => {
    const outcome = await Effect.runPromise(
      runBoot(fixedAmbientInference("I ran the command and committed it for you.")),
    );
    expect(outcome._tag).toBe("Refused");
    if (outcome._tag !== "Refused") return;
    expect(outcome.reason).toBe("action_claim_rejected");
    expect(outcome.provenance.advisory).toBe(true);
  });
});

describe("AFS-07 degrade without a failure surface", () => {
  test("degrades to not_ready when Apple FM is unavailable", async () => {
    const outcome = await Effect.runPromise(
      runBoot(fixedAmbientInference(goodBoot), makeFakeAmbientResourceGate({ appleFmReady: false })),
    );
    expect(outcome._tag).toBe("Degraded");
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("not_ready");
  });

  test("degrades to resource_limited above the thermal ceiling", async () => {
    // The boot task ceiling is nominal; a fair thermal state degrades it.
    const outcome = await Effect.runPromise(
      runBoot(fixedAmbientInference(goodBoot), makeFakeAmbientResourceGate({ thermalState: "fair" })),
    );
    expect(outcome._tag).toBe("Degraded");
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("resource_limited");
  });

  test("degrades to resource_limited under memory pressure", async () => {
    const outcome = await Effect.runPromise(
      runBoot(fixedAmbientInference(goodBoot), makeFakeAmbientResourceGate({ underMemoryPressure: true })),
    );
    expect(outcome._tag).toBe("Degraded");
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("resource_limited");
  });

  test("degrades to not_ready when the on-device turn fails, without throwing", async () => {
    const failed: AppleFmCompletionTurn = { outcome: "failed", usageTruth: "unknown", failureClass: "bridge_unreachable" };
    const outcome = await Effect.runPromise(runBoot(makeFakeAmbientInference(() => failed)));
    expect(outcome._tag).toBe("Degraded");
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("not_ready");
  });

  test("degrades to input_too_large when the built prompt exceeds the input bound", async () => {
    const tightSignature: AmbientTaskSignature<BootExplanationInputType, AmbientExplanationOutput> = {
      ...bootExplanationSignature,
      bounds: { ...bootExplanationSignature.bounds, maxInputChars: 10 },
      buildPrompt: () => "x".repeat(64),
    };
    const outcome = await Effect.runPromise(
      AmbientTaskRunner.pipe(
        Effect.flatMap((runner) => runner.run({ signature: tightSignature, facts: bootFacts })),
        Effect.provide(runnerLayer(fixedAmbientInference(goodBoot))),
      ),
    );
    expect(outcome._tag).toBe("Degraded");
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("input_too_large");
  });

  test("degrades to timed_out when inference exceeds the task timeout", async () => {
    const program = Effect.gen(function* () {
      const fiber = yield* runBoot(makeFakeAmbientInference(() => Effect.never)).pipe(Effect.forkChild);
      yield* TestClock.adjust(Duration.millis(bootExplanationSignature.bounds.timeoutMs + 1));
      return yield* Fiber.join(fiber);
    });
    const outcome = await Effect.runPromise(program.pipe(Effect.provide(TestClock.layer())));
    expect(outcome._tag).toBe("Degraded");
    if (outcome._tag === "Degraded") expect(outcome.reason).toBe("timed_out");
  });

  test("degrades to resource_limited when the concurrency slot is busy (never blocks)", async () => {
    const program = Effect.gen(function* () {
      const started = yield* Deferred.make<void>();
      const release = yield* Deferred.make<void>();
      const inference = makeFakeAmbientInference(() =>
        Effect.gen(function* () {
          yield* Deferred.succeed(started, undefined);
          yield* Deferred.await(release);
          return { outcome: "completed", text: goodBoot, usageTruth: "estimated" } satisfies AppleFmCompletionTurn;
        }),
      );
      return yield* Effect.gen(function* () {
        const runner = yield* AmbientTaskRunner;
        const first = yield* runner.run({ signature: bootExplanationSignature, facts: bootFacts }).pipe(Effect.forkChild);
        // The first task holds the single boot permit.
        yield* Deferred.await(started);
        const second = yield* runner.run({ signature: bootExplanationSignature, facts: bootFacts });
        yield* Deferred.succeed(release, undefined);
        const firstOutcome = yield* Fiber.join(first);
        return { first: firstOutcome, second };
      }).pipe(Effect.provide(runnerLayer(inference)));
    });
    const { first, second } = await Effect.runPromise(program);
    expect(second._tag).toBe("Degraded");
    if (second._tag === "Degraded") expect(second.reason).toBe("resource_limited");
    expect(first._tag).toBe("Completed");
  });
});

describe("AFS-07 non-blocking, cancellable dispatch", () => {
  test("a detached task can be cancelled and resolves to Cancelled", async () => {
    const program = Effect.gen(function* () {
      const release = yield* Deferred.make<void>();
      const inference = makeFakeAmbientInference(() =>
        Deferred.await(release).pipe(
          Effect.as({ outcome: "completed", text: goodBoot, usageTruth: "estimated" } satisfies AppleFmCompletionTurn),
        ),
      );
      return yield* Effect.scoped(
        Effect.gen(function* () {
          const handle = yield* runAmbientTaskDetached({ signature: bootExplanationSignature, facts: bootFacts });
          // The caller is not blocked by the ambient task: cancel immediately.
          yield* handle.cancel;
          return yield* handle.outcome;
        }),
      ).pipe(Effect.provide(runnerLayer(inference)));
    });
    const outcome = await Effect.runPromise(program);
    expect(outcome._tag).toBe("Cancelled");
  });

  test("a detached task resolves to its normal outcome when not cancelled", async () => {
    const program = Effect.scoped(
      Effect.gen(function* () {
        const handle = yield* runAmbientTaskDetached({ signature: bootExplanationSignature, facts: bootFacts });
        return yield* handle.outcome;
      }),
    ).pipe(Effect.provide(runnerLayer(fixedAmbientInference(goodBoot))));
    const outcome = await Effect.runPromise(program);
    expect(outcome._tag).toBe("Completed");
  });
});
