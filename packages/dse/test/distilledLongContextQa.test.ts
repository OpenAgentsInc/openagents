import { test, expect } from "bun:test";
import { Effect, Layer } from "effect";

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

test("Phase F: distilled.search_line_extract.v1 becomes the default fast path; RLM remains fallback", async () => {
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

          const user = req.messages.find((m) => m.role === "user")?.content ?? "";
          const blobId = firstBlobIdFromMessages(req) ?? "";
          const ok = user.includes(NEEDLE_LARGE);
          if (!ok) {
            return { text: JSON.stringify({ answer: "unknown", evidence: { blobId, quote: "" } }) };
          }
          return {
            text: JSON.stringify({
              answer: NEEDLE_LARGE,
              evidence: { blobId, quote: `ERROR oa_req=${NEEDLE_LARGE} unexpected EOF while reading headers` },
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

    const paramsDistilled: Params.DseParamsV1 = {
      ...Params.emptyParamsV1,
      strategy: { id: "distilled.search_line_extract.v1" },
      decode: { mode: "strict_json", maxRepairs: 0 },
      // Include RLM limits so fallback is possible in production (novelty/high uncertainty).
      budgets: {
        maxTimeMs: 15_000,
        maxLmCalls: 40,
        maxOutputChars: 120_000,
        maxRlmIterations: 6,
        maxSubLmCalls: 10,
      },
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

    const [compiledDirect, compiledDistilled, compiledRlm] = yield* Effect.all([
      Hashes.paramsHash(paramsDirect),
      Hashes.paramsHash(paramsDistilled),
      Hashes.paramsHash(paramsRlm),
    ]);

    const direct = yield* Eval.evaluate({
      signature: LongContextQaSignature,
      artifact: makeArtifact(LongContextQaSignature.id, paramsDirect, compiledDirect),
      dataset,
      reward,
      includeExampleDetails: true,
    });

    const distilled = yield* Eval.evaluate({
      signature: LongContextQaSignature,
      artifact: makeArtifact(LongContextQaSignature.id, paramsDistilled, compiledDistilled),
      dataset,
      reward,
      includeExampleDetails: true,
    });

    const rlm = yield* Eval.evaluate({
      signature: LongContextQaSignature,
      artifact: makeArtifact(LongContextQaSignature.id, paramsRlm, compiledRlm),
      dataset,
      reward,
      includeExampleDetails: true,
    });

    return { direct, distilled, rlm };
  }).pipe(Effect.provide(env));

  const { direct, distilled, rlm } = await Effect.runPromise(program);

  const directHoldout = direct.examples?.find((e) => e.exampleId === "log_large.oa_req");
  const distilledHoldout = distilled.examples?.find((e) => e.exampleId === "log_large.oa_req");
  const rlmHoldout = rlm.examples?.find((e) => e.exampleId === "log_large.oa_req");

  expect(directHoldout?.reward).toBeLessThan(1);
  expect(distilledHoldout?.reward).toBe(1);
  expect(typeof rlmHoldout?.reward).toBe("number");
  expect(rlmHoldout!.reward).toBeGreaterThan(0.9);
  if (distilledHoldout && rlmHoldout) {
    expect(distilledHoldout.reward).toBeGreaterThanOrEqual(rlmHoldout.reward);
  }

  const dUsage = (distilledHoldout?.predictMeta as any)?.budgetUsage;
  const rUsage = (rlmHoldout?.predictMeta as any)?.budgetUsage;

  expect(Number(dUsage?.lmCalls ?? 0)).toBe(0);
  expect(Number(rUsage?.lmCalls ?? 0)).toBeGreaterThan(0);
});
