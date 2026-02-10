import { test, expect } from "bun:test";
import { Effect, Layer, Schema } from "effect";

import * as PromptIR from "../src/promptIr.js";
import * as Signature from "../src/signature.js";
import * as Params from "../src/params.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerInMemory as policyLayerInMemory } from "../src/runtime/policyRegistry.js";
import { make as makePredict } from "../src/runtime/predict.js";
import { layerInMemory as blobLayerInMemory, BlobStoreService } from "../src/runtime/blobStore.js";
import { layerInMemory as budgetLayerInMemory } from "../src/runtime/budget.js";
import { makeInMemory as makeReceiptRecorder } from "../src/runtime/receipt.js";

test("Predict supports rlm_lite.v1 strategy with bounded iterations and emits an rlmTrace", async () => {
  type In = { question: string };
  type Out = { answer: string };

  const blobLayer = blobLayerInMemory();

  const responses = [
    JSON.stringify({
      _tag: "Preview",
      target: { _tag: "Var", name: "doc" },
      offset: 0,
      length: 5
    }),
    JSON.stringify({
      _tag: "Final",
      output: { answer: "ok" }
    })
  ];

  let calls = 0;
  const lmLayer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (_req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          const text = responses[calls] ?? responses[responses.length - 1]!;
          calls++;
          return { text };
        })
    })
  );

  const receipts = makeReceiptRecorder();
  const program = Effect.gen(function* () {
    const blobs = yield* BlobStoreService;
    const blob = yield* blobs.putText({ text: "hello world", mime: "text/plain" });

    const sig = Signature.make<In, Out>({
      id: "@openagents/test/RlmLite.v1",
      input: Schema.Struct({ question: Schema.String }),
      output: Schema.Struct({ answer: Schema.String }),
      prompt: {
        version: 1,
        blocks: [
          PromptIR.system("You are a test agent."),
          PromptIR.instruction("Answer the question using the provided context."),
          PromptIR.context([PromptIR.contextBlob("doc", blob)]),
          PromptIR.outputJsonOnly()
        ]
      },
      defaults: {
        params: {
          ...Params.emptyParamsV1,
          strategy: { id: "rlm_lite.v1" },
          decode: { mode: "strict_json", maxRepairs: 0 },
          budgets: { maxRlmIterations: 2, maxSubLmCalls: 0 }
        } satisfies Params.DseParamsV1
      }
    });

    const predict = makePredict(sig);
    return yield* predict({ question: "?" });
  }).pipe(
    Effect.provide(
      Layer.mergeAll(
        lmLayer,
        policyLayerInMemory(),
        blobLayer,
        receipts.layer,
        budgetLayerInMemory()
      )
    )
  );

  const out = await Effect.runPromise(program);
  expect(out).toEqual({ answer: "ok" });
  expect(calls).toBe(2);

  const seen = receipts.getReceipts();
  expect(seen.length).toBe(1);
  expect(seen[0]?.strategyId).toBe("rlm_lite.v1");
  expect(seen[0]?.rlmTrace?.eventCount).toBeGreaterThan(0);
  expect(seen[0]?.rlmTrace?.blob.id).toMatch(/[a-f0-9]{64}/);
});
