import { test, expect } from "bun:test";
import { Effect } from "effect";

import type { PredictReceiptV1 } from "../src/runtime/receipt.js";
import { candidateExampleFromRlmTrace } from "../src/traceMining/exportExamples.js";

test("TraceMining exports a candidate example from an RLM trace (Input + Final)", async () => {
  const receipt: PredictReceiptV1 = {
    format: "openagents.dse.predict_receipt",
    formatVersion: 1,
    receiptId: "r1",
    runId: "r1",
    createdAt: new Date().toISOString(),
    signatureId: "@openagents/test/Sig.v1",
    compiled_id: "c1",
    strategyId: "rlm_lite.v1",
    hashes: {
      inputSchemaHash: "sha256:in",
      outputSchemaHash: "sha256:out",
      promptIrHash: "sha256:prompt",
      paramsHash: "sha256:params"
    },
    model: {},
    timing: { startedAtMs: 1, endedAtMs: 2, durationMs: 1 },
    result: { _tag: "Ok" },
    rlmTrace: { blob: { id: "sha256:abcd", hash: "sha256:abcd", size: 1 }, eventCount: 3 }
  };

  const traceText = JSON.stringify({
    format: "openagents.dse.rlm_trace",
    formatVersion: 1,
    signatureId: receipt.signatureId,
    receiptId: receipt.receiptId,
    strategyId: receipt.strategyId,
    events: [
      { _tag: "Input", input: { question: "q?", blobs: [{ id: "sha256:b1", hash: "sha256:b1", size: 10 }] } },
      { iteration: 1, promptHash: "sha256:p", action: { _tag: "Search", query: "q" } },
      { iteration: 2, promptHash: "sha256:p2", action: { _tag: "Final", output: { answer: "a", evidence: { blobId: "sha256:b1", quote: "line" } } } }
    ]
  });

  const out = await Effect.runPromise(candidateExampleFromRlmTrace({ receipt, traceText }));

  expect(out.signatureId).toBe(receipt.signatureId);
  expect(out.exampleId).toBe("trace:r1");
  expect(out.inputJson).toEqual({
    question: "q?",
    blobs: [{ id: "sha256:b1", hash: "sha256:b1", size: 10 }]
  });
  expect(out.expectedJson).toEqual({ answer: "a", evidence: { blobId: "sha256:b1", quote: "line" } });
  expect(out.tags?.includes("trace_export")).toBe(true);
});

test("TraceMining exports from legacy v1 traces with top-level Final events", async () => {
  const receipt: PredictReceiptV1 = {
    format: "openagents.dse.predict_receipt",
    formatVersion: 1,
    receiptId: "r2",
    runId: "r2",
    createdAt: new Date().toISOString(),
    signatureId: "@openagents/test/Sig.v1",
    compiled_id: "c1",
    strategyId: "rlm_lite.v1",
    hashes: {
      inputSchemaHash: "sha256:in",
      outputSchemaHash: "sha256:out",
      promptIrHash: "sha256:prompt",
      paramsHash: "sha256:params"
    },
    model: {},
    timing: { startedAtMs: 1, endedAtMs: 2, durationMs: 1 },
    result: { _tag: "Ok" },
    rlmTrace: { blob: { id: "sha256:legacy", hash: "sha256:legacy", size: 1 }, eventCount: 3 }
  };

  const traceText = JSON.stringify({
    format: "openagents.dse.rlm_trace",
    formatVersion: 1,
    signatureId: receipt.signatureId,
    receiptId: receipt.receiptId,
    strategyId: receipt.strategyId,
    events: [
      { _tag: "Input", input: { question: "legacy?", blobs: [] } },
      { _tag: "UnknownLegacyEvent", note: "ignored by compatibility decoder" },
      { _tag: "Final", output: { answer: "legacy-output" } }
    ]
  });

  const out = await Effect.runPromise(candidateExampleFromRlmTrace({ receipt, traceText }));

  expect(out.exampleId).toBe("trace:r2");
  expect(out.inputJson).toEqual({ question: "legacy?", blobs: [] });
  expect(out.expectedJson).toEqual({ answer: "legacy-output" });
});
