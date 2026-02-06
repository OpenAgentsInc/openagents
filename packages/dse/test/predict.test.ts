import { test, expect } from "bun:test";
import { Effect, Layer, Schema } from "effect";

import * as PromptIR from "../src/promptIr.js";
import * as Signature from "../src/signature.js";
import * as Params from "../src/params.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerInMemory } from "../src/runtime/policyRegistry.js";
import { make as makePredict } from "../src/runtime/predict.js";

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

  const policyLayer = layerInMemory({
    activeBySignatureId: {
      [sig.id]: {
        compiledId: "sha256:test",
        params: {
          paramsVersion: 1,
          instruction: { text: "Extract the handle. Output JSON only." },
          fewShot: { exampleIds: ["ex2"] },
          decode: { mode: "strict_json", maxRepairs: 0 }
        } satisfies Params.DseParamsV1
      }
    }
  });

  const predict = makePredict(sig);
  const program = predict({ message: "Call me Chris." }).pipe(
    Effect.provide(Layer.mergeAll(lmLayer, policyLayer))
  );

  const out = await Effect.runPromise(program);
  expect(out).toEqual({ handle: "Chris" });

  expect(seen.length).toBe(1);
  const userMsg = seen[0]!.messages.find((m) => m.role === "user");
  expect(userMsg?.content).toContain("Extract the handle. Output JSON only.");
  expect(userMsg?.content).toContain('"id":"ex2"');
  expect(userMsg?.content).not.toContain('"id":"ex1"');
});

