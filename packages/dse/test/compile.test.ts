import { test, expect } from "bun:test";
import { Effect, Layer, Schema } from "effect";

import * as Compile from "../src/compile/compile.js";
import * as CompiledArtifact from "../src/compiledArtifact.js";
import * as EvalCache from "../src/eval/cache.js";
import * as Dataset from "../src/eval/dataset.js";
import * as Metric from "../src/eval/metric.js";
import * as Reward from "../src/eval/reward.js";
import * as Hashes from "../src/hashes.js";
import * as Params from "../src/params.js";
import * as PromptIR from "../src/promptIr.js";
import * as Signature from "../src/signature.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerNoop as budgetLayerNoop } from "../src/runtime/budget.js";
import { layerInMemory as blobLayerInMemory } from "../src/runtime/blobStore.js";
import { layerNoop as receiptLayerNoop } from "../src/runtime/receipt.js";

function makeFakeLmClientForCompile() {
  let calls = 0;
  const layer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          calls++;
          const user = req.messages.find((m) => m.role === "user")?.content ?? "";

          // Instruction grid test: output depends on variant token.
          if (user.includes("VARIANT_B")) {
            return { text: JSON.stringify({ label: "b" }) };
          }
          if (user.includes("VARIANT_A")) {
            return { text: JSON.stringify({ label: "a" }) };
          }

          // Few-shot test: treat mixed examples as confusing/bad.
          const hasGood = user.includes('"id":"ex_good"');
          const hasBad = user.includes('"id":"ex_bad"');
          if (hasGood && !hasBad) {
            return { text: JSON.stringify({ label: "good" }) };
          }

          return { text: JSON.stringify({ label: "bad" }) };
        })
    })
  );

  return { layer, getCalls: () => calls };
}

function envForCompile(fakeLmLayer: Layer.Layer<LmClientService>) {
  return Layer.mergeAll(
    fakeLmLayer,
    blobLayerInMemory(),
    receiptLayerNoop(),
    budgetLayerNoop(),
    EvalCache.layerInMemory()
  );
}

test("Compile.instruction_grid selects the best instruction variant", async () => {
  type In = { message: string };
  type Out = { label: string };

  const sig = Signature.make<In, Out>({
    id: "@openagents/test/CompileInstruction.v1",
    input: Schema.Struct({ message: Schema.String }),
    output: Schema.Struct({ label: Schema.String }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a test predictor."),
        PromptIR.instruction("Default instruction (will be replaced)."),
        PromptIR.outputJsonOnly()
      ]
    },
    defaults: { params: { ...Params.emptyParamsV1, decode: { mode: "strict_json", maxRepairs: 0 } } }
  });

  const ds = await Effect.runPromise(
    Dataset.make<In, Out>({
      datasetId: "compile_ds.v1",
      examples: [
        { exampleId: "ex1", input: { message: "x" }, expected: { label: "b" } },
        { exampleId: "ex2", input: { message: "y" }, expected: { label: "b" } }
      ]
    })
  );

  const metric = Metric.deterministic<Out, Out>({
    metricId: "exact",
    metricVersion: 1,
    score: (pred, expected) => (pred.label === expected.label ? 1 : 0)
  });

  const reward = Reward.makeBundle<In, Out, Out>({
    rewardId: "reward_exact.v1",
    rewardVersion: 1,
    signals: [
      Reward.signalFormatValidity({ weight: 0.1 }),
      Reward.signalMetric(metric, { weight: 0.9 })
    ]
  });

  const fakeLm = makeFakeLmClientForCompile();

  const res = await Effect.runPromise(
    Compile.compile({
      signature: sig,
      dataset: ds,
      reward,
      searchSpace: {
        instructionVariants: [
          { id: "a", text: "VARIANT_A: always output a" },
          { id: "b", text: "VARIANT_B: always output b" }
        ]
      },
      optimizer: { id: "instruction_grid.v1" }
    }).pipe(Effect.provide(envForCompile(fakeLm.layer)))
  );

  expect(res.report.best.reward).toBe(1);
  expect(res.artifact.params.instruction?.text).toContain("VARIANT_B");

  const expectedCompiled = await Effect.runPromise(
    Hashes.paramsHash(res.artifact.params)
  );
  expect(res.artifact.compiled_id).toBe(expectedCompiled);
});

test("Compile.fewshot_greedy_forward selects the best few-shot example ids", async () => {
  type In = { message: string };
  type Out = { label: string };

  const sig = Signature.make<In, Out>({
    id: "@openagents/test/CompileFewShot.v1",
    input: Schema.Struct({ message: Schema.String }),
    output: Schema.Struct({ label: Schema.String }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a test predictor."),
        PromptIR.instruction("Return JSON only."),
        PromptIR.fewShot<In, Out>([
          { id: "ex_good", input: { message: "good" }, output: { label: "good" } },
          { id: "ex_bad", input: { message: "bad" }, output: { label: "bad" } }
        ]),
        PromptIR.outputJsonOnly()
      ]
    },
    defaults: { params: { ...Params.emptyParamsV1, decode: { mode: "strict_json", maxRepairs: 0 } } }
  });

  const ds = await Effect.runPromise(
    Dataset.make<In, Out>({
      datasetId: "compile_fs_ds.v1",
      examples: [
        { exampleId: "ex1", input: { message: "x" }, expected: { label: "good" } },
        { exampleId: "ex2", input: { message: "y" }, expected: { label: "good" } }
      ]
    })
  );

  const metric = Metric.deterministic<Out, Out>({
    metricId: "exact",
    metricVersion: 1,
    score: (pred, expected) => (pred.label === expected.label ? 1 : 0)
  });

  const reward = Reward.makeBundle<In, Out, Out>({
    rewardId: "reward_exact.v1",
    rewardVersion: 1,
    signals: [
      Reward.signalFormatValidity({ weight: 0.1 }),
      Reward.signalMetric(metric, { weight: 0.9 })
    ]
  });

  const fakeLm = makeFakeLmClientForCompile();

  const res = await Effect.runPromise(
    Compile.compile({
      signature: sig,
      dataset: ds,
      reward,
      searchSpace: {
        fewShot: { candidateExampleIds: ["ex_bad", "ex_good"], kMax: 1 }
      },
      optimizer: { id: "fewshot_greedy_forward.v1" }
    }).pipe(Effect.provide(envForCompile(fakeLm.layer)))
  );

  expect(res.report.best.reward).toBe(1);
  expect(res.artifact.params.fewShot?.exampleIds).toEqual(["ex_good"]);

  // Sanity: artifact schema validates.
  Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(res.artifact);
});
