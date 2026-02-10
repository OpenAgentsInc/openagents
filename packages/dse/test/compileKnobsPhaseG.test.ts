import { test, expect } from "bun:test";
import { Effect, Layer } from "effect";

import * as Compile from "../src/compile/compile.js";
import * as Dataset from "../src/eval/dataset.js";
import * as EvalCache from "../src/eval/cache.js";
import * as Params from "../src/params.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerNoop as budgetLayerNoop } from "../src/runtime/budget.js";
import { layerInMemory as blobLayerInMemory } from "../src/runtime/blobStore.js";
import { layerNoop as receiptLayerNoop } from "../src/runtime/receipt.js";
import { layerInMemory as varSpaceLayerInMemory } from "../src/runtime/varSpace.js";

import {
  LongContextQaSignature,
  makeDummyLongContextLogQaDataset,
  rewardForLongContextQa,
} from "../src/eval/longContextBench.js";

function firstBlobIdFromMessages(req: LmRequest): string | null {
  const all = req.messages.map((m) => m.content).join("\n");
  const m = all.match(/sha256:[a-f0-9]{64}/);
  return m?.[0] ?? null;
}

function envForCompile(fakeLmLayer: Layer.Layer<LmClientService>) {
  return Layer.mergeAll(
    fakeLmLayer,
    blobLayerInMemory(),
    receiptLayerNoop(),
    budgetLayerNoop(),
    EvalCache.layerInMemory(),
    varSpaceLayerInMemory(),
  );
}

test("Phase G: Compile knobs can pick a better strategy (direct -> distilled) based on reward", async () => {
  const NEEDLE_SMALL = "NEEDLE_LOG_SMALL_7b2c";
  const NEEDLE_LARGE = "NEEDLE_LOG_LARGE_91af";

  const lmLayer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          const sys = req.messages.find((m) => m.role === "system")?.content ?? "";
          const isRlmController = sys.includes("RLM Action schema") || sys.includes("RLM-lite controller");

          // This test only compiles direct vs distilled. If we ever hit an RLM controller here,
          // fail closed so the test doesn't silently pass via the wrong path.
          if (isRlmController) return { text: "not_json" };

          const user = req.messages.find((m) => m.role === "user")?.content ?? "";
          const blobId = firstBlobIdFromMessages(req) ?? "";

          if (user.includes(NEEDLE_LARGE)) {
            return {
              text: JSON.stringify({
                answer: NEEDLE_LARGE,
                evidence: {
                  blobId,
                  quote: `ERROR oa_req=${NEEDLE_LARGE} unexpected EOF while reading headers`,
                },
              }),
            };
          }

          if (user.includes(NEEDLE_SMALL)) {
            return {
              text: JSON.stringify({
                answer: NEEDLE_SMALL,
                evidence: {
                  blobId,
                  quote: `ERROR oa_req=${NEEDLE_SMALL} unexpected EOF while reading headers`,
                },
              }),
            };
          }

          return {
            text: JSON.stringify({
              answer: "unknown",
              evidence: { blobId, quote: "" },
            }),
          };
        }),
    }),
  );

  const program = Effect.gen(function* () {
    const ds0 = yield* makeDummyLongContextLogQaDataset();
    const ds = yield* Dataset.make({
      datasetId: ds0.datasetId + ":train_all",
      examples: ds0.examples.map((e) => ({ ...e, split: "train" })),
    });

    const baseParams: Params.DseParamsV1 = {
      ...Params.emptyParamsV1,
      strategy: { id: "direct.v1" },
      decode: { mode: "strict_json", maxRepairs: 1 },
      budgets: { maxTimeMs: 15_000, maxLmCalls: 4, maxOutputChars: 120_000 },
    };

    const res = yield* Compile.compile({
      signature: LongContextQaSignature,
      baseParams,
      dataset: ds,
      reward: rewardForLongContextQa(),
      searchSpace: {
        strategyVariants: [
          { id: "direct", strategyId: "direct.v1" },
          { id: "distilled", strategyId: "distilled.search_line_extract.v1" },
        ],
      },
      optimizer: { id: "knobs_grid.v1" },
    });

    return res;
  }).pipe(Effect.provide(envForCompile(lmLayer)));

  const res = await Effect.runPromise(program);

  expect(res.artifact.params.strategy?.id).toBe("distilled.search_line_extract.v1");
  expect(res.report.best.reward).toBe(1);
});

test("Phase G: knobs_grid_refine can patch RLM controller instructions based on decode failures", async () => {
  const NEEDLE_LARGE = "NEEDLE_LOG_LARGE_91af";

  let controllerCalls = 0;

  const lmLayer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          const sys = req.messages.find((m) => m.role === "system")?.content ?? "";
          const isRlmController = sys.includes("RLM Action schema") || sys.includes("RLM-lite controller");

          if (!isRlmController) {
            // Should not hit direct for this test.
            return { text: JSON.stringify({ answer: "unknown", evidence: { blobId: "", quote: "" } }) };
          }

          // Fail closed unless the refined controller instruction is present.
          const ok = sys.includes("Critical: Output MUST be valid JSON matching the RLM Action schema");
          if (!ok) {
            return { text: "not json" };
          }

          const blobId = firstBlobIdFromMessages(req) ?? "sha256:missing";

          if (controllerCalls === 0) {
            controllerCalls++;
            return {
              text: JSON.stringify({
                _tag: "Search",
                target: { _tag: "Blob", blobId },
                query: NEEDLE_LARGE,
                maxMatches: 1,
                contextChars: 80,
              }),
            };
          }

          controllerCalls++;
          return {
            text: JSON.stringify({
              _tag: "Final",
              output: {
                answer: NEEDLE_LARGE,
                evidence: {
                  blobId,
                  quote: `ERROR oa_req=${NEEDLE_LARGE} unexpected EOF while reading headers`,
                },
              },
            }),
          };
        }),
    }),
  );

  const program = Effect.gen(function* () {
    const ds0 = yield* makeDummyLongContextLogQaDataset();
    const ex = ds0.examples.find((e) => e.exampleId === "log_large.oa_req");
    if (!ex) throw new Error("missing_fixture_example");

    const ds = yield* Dataset.make({
      datasetId: ds0.datasetId + ":large_train_only",
      examples: [{ ...ex, split: "train" }],
    });

    const baseParams: Params.DseParamsV1 = {
      ...Params.emptyParamsV1,
      strategy: { id: "rlm_lite.v1" },
      decode: { mode: "strict_json", maxRepairs: 0 },
      budgets: {
        maxTimeMs: 15_000,
        maxLmCalls: 40,
        maxOutputChars: 120_000,
        maxRlmIterations: 6,
        maxSubLmCalls: 10,
      },
    };

    const res = yield* Compile.compile({
      signature: LongContextQaSignature,
      baseParams,
      dataset: ds,
      reward: rewardForLongContextQa(),
      searchSpace: {},
      optimizer: { id: "knobs_grid_refine.v1" },
    });

    return res;
  }).pipe(Effect.provide(envForCompile(lmLayer)));

  const res = await Effect.runPromise(program);

  expect(res.report.best.reward).toBeGreaterThan(0.9);
  expect(res.artifact.params.strategy?.id).toBe("rlm_lite.v1");
  expect(res.artifact.params.rlmLite?.controllerInstructions).toContain(
    "Critical: Output MUST be valid JSON matching the RLM Action schema",
  );
  expect(res.report.evaluatedCandidates.length).toBeGreaterThanOrEqual(2);
});
