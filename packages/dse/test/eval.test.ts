import { test, expect } from "bun:test";
import { Effect, Layer, Schema } from "effect";

import * as CompiledArtifact from "../src/compiledArtifact.js";
import * as EvalCache from "../src/eval/cache.js";
import * as Dataset from "../src/eval/dataset.js";
import * as Metric from "../src/eval/metric.js";
import * as Reward from "../src/eval/reward.js";
import * as Eval from "../src/eval/evaluate.js";
import * as Params from "../src/params.js";
import * as PromptIR from "../src/promptIr.js";
import * as Signature from "../src/signature.js";

import { LmClientService, type LmRequest, type LmResponse } from "../src/runtime/lm.js";
import { layerNoop as budgetLayerNoop } from "../src/runtime/budget.js";
import { layerInMemory as blobLayerInMemory } from "../src/runtime/blobStore.js";
import { layerNoop as receiptLayerNoop } from "../src/runtime/receipt.js";
import { layerInMemory as varSpaceLayerInMemory } from "../src/runtime/varSpace.js";

function makeFakeLmClient() {
  let calls = 0;
  const seen: Array<LmRequest> = [];

  const layer = Layer.succeed(
    LmClientService,
    LmClientService.of({
      complete: (req: LmRequest): Effect.Effect<LmResponse, never> =>
        Effect.sync(() => {
          calls++;
          seen.push(req);
          const user = req.messages.find((m) => m.role === "user")?.content ?? "";
          const isJudge = user.includes("JUDGE:");
          const idx = user.lastIndexOf("Input:\n");
          const raw = idx === -1 ? "{}" : user.slice(idx + "Input:\n".length).trim();
          const parsed = JSON.parse(raw) as any;

          if (isJudge) {
            const pred = parsed?.pred?.label;
            const expected = parsed?.expected?.label;
            const score = pred === expected ? 1 : 0;
            return { text: JSON.stringify({ score }) };
          }

          // Main signature: echo the input text as label.
          const label = String(parsed?.text ?? "");
          return { text: JSON.stringify({ label }) };
        })
    })
  );

  return { layer, getCalls: () => calls, getSeen: () => seen };
}

function makeArtifact(signatureId: string, compiled_id: string, params: Params.DseParamsV1) {
  const artifact: CompiledArtifact.DseCompiledArtifactV1 = {
    format: "openagents.dse.compiled_artifact",
    formatVersion: 1,
    signatureId,
    compiled_id,
    createdAt: new Date().toISOString(),
    hashes: {
      inputSchemaHash: "sha256:in",
      outputSchemaHash: "sha256:out",
      promptIrHash: "sha256:prompt",
      paramsHash: compiled_id
    },
    params,
    eval: { evalVersion: 1, kind: "unscored" },
    optimizer: { id: "test" },
    provenance: {}
  };

  // Ensure it round-trips schema validation (catch drift early).
  Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(artifact);
  return artifact;
}

test("Dataset.make enforces unique ids and sorts deterministically", async () => {
  const ds = await Effect.runPromise(
    Dataset.make({
      datasetId: "ds.v1",
      examples: [
        { exampleId: "b", input: { x: 2 }, expected: { y: 2 }, tags: ["z", "a", "a"] },
        { exampleId: "a", input: { x: 1 }, expected: { y: 1 }, tags: ["b", "a"] }
      ]
    })
  );

  expect(ds.examples.map((e) => e.exampleId)).toEqual(["a", "b"]);
  expect(ds.examples[0]?.tags).toEqual(["a", "b"]);
});

test("Eval.evaluate produces stable summary and uses eval cache keys", async () => {
  type In = { text: string };
  type Out = { label: string };

  const InSchema = Schema.Struct({ text: Schema.String });
  const OutSchema = Schema.Struct({ label: Schema.String });

  const sig = Signature.make<In, Out>({
    id: "@openagents/test/EvalSig.v1",
    input: InSchema,
    output: OutSchema,
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a test predictor."),
        PromptIR.instruction("Return JSON only."),
        PromptIR.outputJsonOnly()
      ]
    }
  });

  const params: Params.DseParamsV1 = {
    ...Params.emptyParamsV1,
    decode: { mode: "strict_json", maxRepairs: 0 }
  };

  const artifact = makeArtifact(sig.id, "sha256:test-main", params);

  const ds = await Effect.runPromise(
    Dataset.make<In, Out>({
      datasetId: "eval_ds.v1",
      examples: [
        { exampleId: "ex2", input: { text: "b" }, expected: { label: "b" } },
        { exampleId: "ex1", input: { text: "a" }, expected: { label: "a" } }
      ]
    })
  );

  const metric = Metric.deterministic<Out, Out>({
    metricId: "exact_match",
    metricVersion: 1,
    score: (pred, expected) => (pred.label === expected.label ? 1 : 0)
  });

  const reward = Reward.makeBundle<In, Out, Out>({
    rewardId: "reward_exact.v1",
    rewardVersion: 1,
    signals: [
      Reward.signalFormatValidity({ weight: 0.2 }),
      Reward.signalMetric(metric, { weight: 0.8 })
    ]
  });

  const fakeLm = makeFakeLmClient();
  const blobs = blobLayerInMemory();
  const receipts = receiptLayerNoop();

  const cacheMap = new Map<string, unknown>();
  const cacheSvc = EvalCache.EvalCacheService.of({
    get: (keyId) => Effect.sync(() => cacheMap.get(keyId) ?? null),
    set: (keyId, value) => Effect.sync(() => void cacheMap.set(keyId, value))
  });

  const layer = Layer.mergeAll(fakeLm.layer, blobs, receipts, budgetLayerNoop(), varSpaceLayerInMemory());

  const runEval = () =>
    Effect.runPromise(
      Eval.evaluate({
        signature: sig,
        artifact,
        dataset: ds,
        reward,
        includeExampleDetails: true
      }).pipe(Effect.provide(layer), Effect.provideService(EvalCache.EvalCacheService, cacheSvc))
    );

  const res1 = await runEval();
  expect(res1.summary.kind).toBe("scored");
  expect(res1.summary.n).toBe(2);
  expect(res1.summary.metricId).toBe("reward_exact.v1");
  expect(res1.summary.metricVersion).toBe(1);
  expect(res1.summary.reward).toBe(1);
  expect(res1.examples?.map((e) => e.exampleId)).toEqual(["ex1", "ex2"]);

  const callsAfter1 = fakeLm.getCalls();
  expect(callsAfter1).toBe(2);

  // Second run should be fully cached (no additional LM calls).
  const res2 = await runEval();
  expect(res2.summary.reward).toBe(1);
  expect(fakeLm.getCalls()).toBe(callsAfter1);
});

test("Judge metrics are pinned to a compiled artifact and recorded in reports", async () => {
  type In = { text: string };
  type Out = { label: string };
  type JudgeIn = { input: In; pred: Out; expected: Out };
  type JudgeOut = { score: number };

  const InSchema = Schema.Struct({ text: Schema.String });
  const OutSchema = Schema.Struct({ label: Schema.String });

  const main = Signature.make<In, Out>({
    id: "@openagents/test/JudgeMain.v1",
    input: InSchema,
    output: OutSchema,
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a test predictor."),
        PromptIR.instruction("Return JSON only."),
        PromptIR.outputJsonOnly()
      ]
    }
  });

  const judgeSig = Signature.make<JudgeIn, JudgeOut>({
    id: "@openagents/test/JudgeSig.v1",
    input: Schema.Struct({ input: InSchema, pred: OutSchema, expected: OutSchema }),
    output: Schema.Struct({ score: Schema.Number }),
    prompt: {
      version: 1,
      blocks: [
        PromptIR.system("You are a judge."),
        PromptIR.instruction("JUDGE: output a score 1 if pred matches expected else 0."),
        PromptIR.outputJsonOnly()
      ]
    }
  });

  const mainArtifact = makeArtifact(
    main.id,
    "sha256:main",
    { ...Params.emptyParamsV1, decode: { mode: "strict_json", maxRepairs: 0 } }
  );
  const judgeArtifact = makeArtifact(
    judgeSig.id,
    "sha256:judge",
    { ...Params.emptyParamsV1, decode: { mode: "strict_json", maxRepairs: 0 } }
  );

  const ds = await Effect.runPromise(
    Dataset.make<In, Out>({
      datasetId: "judge_ds.v1",
      examples: [{ exampleId: "ex1", input: { text: "ok" }, expected: { label: "ok" } }]
    })
  );

  const judgeMetric = Metric.judge<In, Out, Out, JudgeIn, JudgeOut>({
    metricId: "judge_exact",
    metricVersion: 1,
    judgeSignature: judgeSig,
    judgeArtifact,
    buildJudgeInput: ({ input, pred, expected }) => ({ input, pred, expected }),
    scoreFromJudgeOutput: (o) => o.score
  });

  const reward = Reward.makeBundle<In, Out, Out>({
    rewardId: "reward_judge.v1",
    rewardVersion: 1,
    signals: [Reward.signalMetric(judgeMetric, { weight: 1 })]
  });

  const fakeLm = makeFakeLmClient();
  const blobs = blobLayerInMemory();
  const receipts = receiptLayerNoop();
  const cache = EvalCache.layerNoop();

  const res = await Effect.runPromise(
    Eval.evaluate({
      signature: main,
      artifact: mainArtifact,
      dataset: ds,
      reward,
      includeExampleDetails: true
    }).pipe(
      Effect.provide(
        Layer.mergeAll(fakeLm.layer, blobs, receipts, budgetLayerNoop(), cache, varSpaceLayerInMemory())
      )
    )
  );

  const signal = res.examples?.[0]?.signals?.[0];
  expect(signal?.metric?.kind).toBe("judge");
  expect(signal?.metric?.judge?.signatureId).toBe(judgeSig.id);
  expect(signal?.metric?.judge?.compiled_id).toBe(judgeArtifact.compiled_id);
});
