import { test, expect } from "bun:test";
import { Effect, Layer, Schema } from "effect";

import * as PromptIR from "../src/promptIr.js";
import * as Signature from "../src/signature.js";
import * as Params from "../src/params.js";
import * as Hashes from "../src/hashes.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerNoop as budgetLayerNoop } from "../src/runtime/budget.js";
import { layerInMemory } from "../src/runtime/policyRegistry.js";
import { make as makePredict } from "../src/runtime/predict.js";
import { layerInMemory as blobLayerInMemory } from "../src/runtime/blobStore.js";
import { makeInMemory as makeReceiptRecorder } from "../src/runtime/receipt.js";

test("Predict applies active policy params (instruction + few-shot selection)", async () => {
  type In = { message: string };
  type Out = { handle: string };

  const InSchema = Schema.Struct({ message: Schema.String });
  const OutSchema = Schema.Struct({ handle: Schema.String });

  const sig = Signature.make<In, Out>({
    id: "@openagents/test/UserHandle.v1",
    input: InSchema,
    output: OutSchema,
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a test agent."),
        PromptIR.instruction("Extract the user's preferred handle."),
        PromptIR.fewShot<In, Out>([
          { id: "ex1", input: { message: "Call me Ada." }, output: { handle: "Ada" } },
          { id: "ex2", input: { message: "Name is Chris." }, output: { handle: "Chris" } }
        ]),
        PromptIR.outputJsonOnly()
      ]
    }
  });

  const seen: Array<LmRequest> = [];

  const lmLayer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          seen.push(req);
          return { text: JSON.stringify({ handle: "Chris" }) };
        })
    })
  );

  const params: Params.DseParamsV1 = {
    paramsVersion: 1,
    instruction: { text: "Extract the handle. Output JSON only." },
    fewShot: { exampleIds: ["ex2"] },
    decode: { mode: "strict_json", maxRepairs: 0 }
  };

  const compiled_id = await Effect.runPromise(Hashes.paramsHash(params));
  const inputSchemaHash = await Effect.runPromise(Hashes.schemaJsonHash(InSchema));
  const outputSchemaHash = await Effect.runPromise(Hashes.schemaJsonHash(OutSchema));
  const promptIrHash = await Effect.runPromise(Hashes.promptIrHash(sig.prompt));

  const policyLayer = layerInMemory({
    activeBySignatureId: { [sig.id]: compiled_id },
    artifacts: [
      {
        format: "openagents.dse.compiled_artifact",
        formatVersion: 1,
        signatureId: sig.id,
        compiled_id,
        createdAt: new Date().toISOString(),
        hashes: {
          inputSchemaHash,
          outputSchemaHash,
          promptIrHash,
          paramsHash: compiled_id
        },
        params,
        eval: { evalVersion: 1, kind: "unscored" },
        optimizer: { id: "test" },
        provenance: {}
      }
    ]
  });

  const blobLayer = blobLayerInMemory();
  const receiptRecorder = makeReceiptRecorder();

  const predict = makePredict(sig);
  const program = predict({ message: "Call me Chris." }).pipe(
    Effect.provide(
      Layer.mergeAll(
        lmLayer,
        policyLayer,
        blobLayer,
        receiptRecorder.layer,
        budgetLayerNoop()
      )
    )
  );

  const out = await Effect.runPromise(program);
  expect(out).toEqual({ handle: "Chris" });

  expect(seen.length).toBe(1);
  const userMsg = seen[0]!.messages.find((m) => m.role === "user");
  expect(userMsg?.content).toContain("Extract the handle. Output JSON only.");
  expect(userMsg?.content).toContain('"id":"ex2"');
  expect(userMsg?.content).not.toContain('"id":"ex1"');

  const receipts = receiptRecorder.getReceipts();
  expect(receipts.length).toBe(1);
  expect(receipts[0]?.signatureId).toBe(sig.id);
  expect(receipts[0]?.compiled_id).toBe(compiled_id);
});
