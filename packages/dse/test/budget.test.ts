import { test, expect } from "bun:test";
import { Effect, Layer, Schema } from "effect";

import * as PromptIR from "../src/promptIr.js";
import * as Signature from "../src/signature.js";
import * as Params from "../src/params.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerInMemory as policyLayerInMemory } from "../src/runtime/policyRegistry.js";
import { make as makePredict } from "../src/runtime/predict.js";
import { layerInMemory as blobLayerInMemory } from "../src/runtime/blobStore.js";
import { layerInMemory as budgetLayerInMemory } from "../src/runtime/budget.js";
import { makeInMemory as makeReceiptRecorder } from "../src/runtime/receipt.js";

test("Predict enforces budgets.maxLmCalls (fail closed before LM call)", async () => {
  type In = { message: string };
  type Out = { ok: boolean };

  const sig = Signature.make<In, Out>({
    id: "@openagents/test/BudgetLmCalls.v1",
    input: Schema.Struct({ message: Schema.String }),
    output: Schema.Struct({ ok: Schema.Boolean }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a test predictor."),
        PromptIR.instruction("Return JSON only."),
        PromptIR.outputJsonOnly()
      ]
    },
    defaults: {
      params: {
        ...Params.emptyParamsV1,
        decode: { mode: "strict_json", maxRepairs: 0 },
        budgets: { maxLmCalls: 0 }
      } satisfies Params.DseParamsV1
    }
  });

  let calls = 0;
  const lmLayer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (_req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          calls++;
          return { text: JSON.stringify({ ok: true }) };
        })
    })
  );

  const receipts = makeReceiptRecorder();
  const predict = makePredict(sig);

  const program = predict({ message: "hi" }).pipe(
    Effect.provide(
      Layer.mergeAll(
        lmLayer,
        policyLayerInMemory(),
        blobLayerInMemory(),
        receipts.layer,
        budgetLayerInMemory()
      )
    )
  );

  const either = await Effect.runPromise(program.pipe(Effect.either));
  expect(calls).toBe(0);
  expect(either._tag).toBe("Left");
  expect(either._tag === "Left" ? (either.left as any)._tag : null).toBe("BudgetExceededError");

  const seen = receipts.getReceipts();
  expect(seen.length).toBe(1);
  expect(seen[0]?.result._tag).toBe("Error");
  expect((seen[0]?.result as any).errorName).toBe("BudgetExceededError");
});

test("Predict enforces budgets.maxOutputChars (fails on oversized output)", async () => {
  type In = { message: string };
  type Out = { ok: boolean };

  const sig = Signature.make<In, Out>({
    id: "@openagents/test/BudgetOutputChars.v1",
    input: Schema.Struct({ message: Schema.String }),
    output: Schema.Struct({ ok: Schema.Boolean }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a test predictor."),
        PromptIR.instruction("Return JSON only."),
        PromptIR.outputJsonOnly()
      ]
    },
    defaults: {
      params: {
        ...Params.emptyParamsV1,
        decode: { mode: "strict_json", maxRepairs: 0 },
        budgets: { maxOutputChars: 5 }
      } satisfies Params.DseParamsV1
    }
  });

  let calls = 0;
  const lmLayer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (_req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          calls++;
          return { text: JSON.stringify({ ok: true }) };
        })
    })
  );

  const receipts = makeReceiptRecorder();
  const predict = makePredict(sig);

  const program = predict({ message: "hi" }).pipe(
    Effect.provide(
      Layer.mergeAll(
        lmLayer,
        policyLayerInMemory(),
        blobLayerInMemory(),
        receipts.layer,
        budgetLayerInMemory()
      )
    )
  );

  const either = await Effect.runPromise(program.pipe(Effect.either));
  expect(calls).toBe(1);
  expect(either._tag).toBe("Left");
  expect(either._tag === "Left" ? (either.left as any)._tag : null).toBe("BudgetExceededError");

  const seen = receipts.getReceipts();
  expect(seen.length).toBe(1);
  expect(seen[0]?.budget?.usage.outputChars).toBeGreaterThan(5);
});
