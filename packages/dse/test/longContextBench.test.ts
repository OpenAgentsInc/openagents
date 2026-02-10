import { test, expect } from "bun:test";
import { Effect, Layer, Schema } from "effect";

import * as Hashes from "../src/hashes.js";
import * as Params from "../src/params.js";
import type { DseCompiledArtifactV1 } from "../src/compiledArtifact.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerInMemory as blobLayerInMemory } from "../src/runtime/blobStore.js";
import { layerNoop as budgetLayerNoop } from "../src/runtime/budget.js";
import { layerInMemory as varSpaceLayerInMemory } from "../src/runtime/varSpace.js";
import { layerNoop as receiptLayerNoop } from "../src/runtime/receipt.js";

import * as EvalCache from "../src/eval/cache.js";
import * as Eval from "../src/eval/evaluate.js";

import {
  LongContextQaSignature,
  makeDummyLongContextLogQaDataset,
  rewardForLongContextQa,
} from "../src/eval/longContextBench.js";

function makeArtifact(signatureId: string, params: Params.DseParamsV1, compiled_id: string): DseCompiledArtifactV1 {
  return {
    format: "openagents.dse.compiled_artifact",
    formatVersion: 1,
    signatureId,
    compiled_id,
    createdAt: new Date().toISOString(),
    hashes: {
      inputSchemaHash: "sha256:in",
      outputSchemaHash: "sha256:out",
      promptIrHash: "sha256:prompt",
      paramsHash: compiled_id,
    },
    params,
    eval: { evalVersion: 1, kind: "unscored" },
    optimizer: { id: "test" },
    provenance: {},
  };
}

function firstBlobIdFromMessages(req: LmRequest): string | null {
  const all = req.messages.map((m) => m.content).join("\n");
  const m = all.match(/sha256:[a-f0-9]{64}/);
  return m?.[0] ?? null;
}

test("Phase E: Eval compares direct.v1 vs rlm_lite.v1 on a long-context needle beyond preview", async () => {
  const NEEDLE_LARGE = "NEEDLE_LOG_LARGE_91af";

  let rlmControllerCalls = 0;

  const lmLayer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          const sys = req.messages.find((m) => m.role === "system")?.content ?? "";
          const isRlmController = sys.includes("RLM Action schema") || sys.includes("RLM-lite controller");

          if (isRlmController) {
            const blobId = firstBlobIdFromMessages(req) ?? "sha256:missing";

            // First controller call: run a bounded search.
            if (rlmControllerCalls === 0) {
              rlmControllerCalls++;
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

            // Second controller call: finalize with the correct evidence.
            rlmControllerCalls++;
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
          }

          // Direct path: succeed only if the rendered prompt includes the needle token.
          const user = req.messages.find((m) => m.role === "user")?.content ?? "";
          const blobId = firstBlobIdFromMessages(req) ?? "";
          const ok = user.includes(NEEDLE_LARGE);
          if (!ok) {
            return {
              text: JSON.stringify({
                answer: "unknown",
                evidence: { blobId, quote: "" },
              }),
            };
          }

          return {
            text: JSON.stringify({
              answer: NEEDLE_LARGE,
              evidence: {
                blobId,
                quote: `ERROR oa_req=${NEEDLE_LARGE} unexpected EOF while reading headers`,
              },
            }),
          };
        }),
    }),
  );

  const env = Layer.mergeAll(
    lmLayer,
    blobLayerInMemory(),
    varSpaceLayerInMemory(),
    receiptLayerNoop(),
    budgetLayerNoop(),
    EvalCache.layerNoop(),
  );

  const reward = rewardForLongContextQa();

  const program = Effect.gen(function* () {
    const dataset = yield* makeDummyLongContextLogQaDataset();

    const paramsDirect: Params.DseParamsV1 = {
      ...Params.emptyParamsV1,
      strategy: { id: "direct.v1" },
      decode: { mode: "strict_json", maxRepairs: 1 },
      budgets: { maxTimeMs: 15_000, maxLmCalls: 4, maxOutputChars: 120_000 },
    };

    const paramsRlm: Params.DseParamsV1 = {
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

    const compiledDirect = yield* Hashes.paramsHash(paramsDirect);
    const compiledRlm = yield* Hashes.paramsHash(paramsRlm);

    const artifactDirect = makeArtifact(LongContextQaSignature.id, paramsDirect, compiledDirect);
    const artifactRlm = makeArtifact(LongContextQaSignature.id, paramsRlm, compiledRlm);

    const direct = yield* Eval.evaluate({
      signature: LongContextQaSignature,
      artifact: artifactDirect,
      dataset,
      reward,
      includeExampleDetails: true,
    });

    const rlm = yield* Eval.evaluate({
      signature: LongContextQaSignature,
      artifact: artifactRlm,
      dataset,
      reward,
      includeExampleDetails: true,
    });

    return { direct, rlm };
  }).pipe(Effect.provide(env));

  const { direct, rlm } = await Effect.runPromise(program);

  // Large example should fail under direct (needle beyond 20k preview), succeed under RLM.
  const directLarge = direct.examples?.find((e) => e.exampleId === "log_large.oa_req");
  const rlmLarge = rlm.examples?.find((e) => e.exampleId === "log_large.oa_req");
  expect(directLarge?.reward).toBeLessThan(1);
  expect(typeof rlmLarge?.reward).toBe("number");
  if (directLarge && rlmLarge) {
    expect(rlmLarge.reward).toBeGreaterThan(directLarge.reward);
  }

  const rlmLargeExact = rlmLarge?.signals.find((s) => s.signalId === "exact_answer.signal.v1");
  const rlmLargeEvidence = rlmLarge?.signals.find((s) => s.signalId === "evidence_quote_in_blob.v1");
  expect(rlmLargeExact?.score).toBe(1);
  expect(rlmLargeEvidence?.score).toBe(1);

  // Cost signal should be present and penalize RLM relative to direct on the small example.
  const directSmall = direct.examples?.find((e) => e.exampleId === "log_small.oa_req");
  const rlmSmall = rlm.examples?.find((e) => e.exampleId === "log_small.oa_req");
  const directCost = directSmall?.signals.find((s) => s.signalId === "predict_cost.v1");
  const rlmCost = rlmSmall?.signals.find((s) => s.signalId === "predict_cost.v1");
  expect(typeof directCost?.score).toBe("number");
  expect(typeof rlmCost?.score).toBe("number");
  if (directCost && rlmCost) {
    expect(rlmCost.score).toBeLessThanOrEqual(directCost.score);
  }
});
