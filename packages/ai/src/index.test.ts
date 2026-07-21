/**
 * AISDK-02 (#9148) umbrella smoke: the root entry and every curated layer
 * subpath must resolve, and the key symbol of each layer must be exported.
 * The umbrella holds no logic, so the oracle is resolution plus identity —
 * a subpath symbol must be the same binding the root re-exports.
 */
import { describe, expect, test } from "vite-plus/test";

import * as Root from "@openagentsinc/ai";
import * as Model from "@openagentsinc/ai/model";
import * as SchemaLayer from "@openagentsinc/ai/schema";
import * as EventLog from "@openagentsinc/ai/event-log";
import * as Sandbox from "@openagentsinc/ai/sandbox";
import * as Harness from "@openagentsinc/ai/harness";
import * as UiStream from "@openagentsinc/ai/ui-stream";
import * as Recall from "@openagentsinc/ai/recall";

describe("@openagentsinc/ai umbrella root", () => {
  test("re-exports the key symbol of every layer", () => {
    // L0 model call
    expect(typeof Root.makeKhalaModelFallbackPlan).toBe("function");
    expect(typeof Root.khalaEffectAiLanguageModelLayer).toBe("function");
    // L1 vocabulary
    expect(typeof Root.decodeKhalaRuntimeEvent).toBe("function");
    // L2 durable log
    expect(typeof Root.makeHarnessEventLog).toBe("function");
    expect(typeof Root.makeInMemoryEventLogStore).toBe("function");
    // L3 sandbox
    expect(typeof Root.makeLocalProcessSandboxProvider).toBe("function");
    // L4 harness
    expect(typeof Root.makeReferenceAdapter).toBe("function");
    expect(typeof Root.projectHarnessReadiness).toBe("function");
    // L5 UI stream
    expect(typeof Root.khalaEventToUiChunks).toBe("function");
    expect(typeof Root.applyUiChunk).toBe("function");
    // L6 recall
    expect(typeof Root.buildHistoryCorpus).toBe("function");
    expect(typeof Root.recallTierD).toBe("function");
  });

  test("the one audited shared name resolves to the single schema binding", () => {
    // Both agent-runtime-schema and agent-harness-contract export this name.
    // They are the same binding, so the star union stays unambiguous.
    expect(Root.KhalaRuntimeEventSchemaLiteral).toBe("openagents.khala_runtime_event.v1");
    expect(SchemaLayer.KhalaRuntimeEventSchemaLiteral).toBe(Root.KhalaRuntimeEventSchemaLiteral);
    expect(Harness.KhalaRuntimeEventSchemaLiteral).toBe(Root.KhalaRuntimeEventSchemaLiteral);
  });
});

describe("@openagentsinc/ai layer subpaths", () => {
  test("./model (L0) exports the model-call substrate", () => {
    expect(typeof Model.makeKhalaModelFallbackPlan).toBe("function");
    expect(typeof Model.khalaEffectAiLanguageModelLayer).toBe("function");
    expect(Model.makeKhalaModelFallbackPlan).toBe(Root.makeKhalaModelFallbackPlan);
  });

  test("./schema (L1) exports the runtime event vocabulary", () => {
    expect(typeof SchemaLayer.decodeKhalaRuntimeEvent).toBe("function");
    expect(SchemaLayer.decodeKhalaRuntimeEvent).toBe(Root.decodeKhalaRuntimeEvent);
  });

  test("./event-log (L2) exports the durable log and store", () => {
    expect(typeof EventLog.makeHarnessEventLog).toBe("function");
    expect(typeof EventLog.makeInMemoryEventLogStore).toBe("function");
    expect(EventLog.makeHarnessEventLog).toBe(Root.makeHarnessEventLog);
  });

  test("./sandbox (L3) exports the sandbox providers", () => {
    expect(typeof Sandbox.makeLocalSandboxProvider).toBe("function");
    expect(typeof Sandbox.makeLocalProcessSandboxProvider).toBe("function");
    expect(Sandbox.makeLocalProcessSandboxProvider).toBe(Root.makeLocalProcessSandboxProvider);
  });

  test("./harness (L4) exports the adapter and readiness surface", () => {
    expect(typeof Harness.makeReferenceAdapter).toBe("function");
    expect(typeof Harness.projectHarnessReadiness).toBe("function");
    expect(Harness.makeReferenceAdapter).toBe(Root.makeReferenceAdapter);
  });

  test("./ui-stream (L5) exports the chunk projection and reducer", () => {
    expect(typeof UiStream.khalaEventToUiChunks).toBe("function");
    expect(typeof UiStream.applyUiChunk).toBe("function");
    expect(typeof UiStream.initialUiMessage).toBe("function");
    expect(typeof UiStream.reduceUiMessageStream).toBe("function");
    expect(UiStream.khalaEventToUiChunks).toBe(Root.khalaEventToUiChunks);
  });

  test("./recall (L6) exports the corpus builder and Tier D recall", () => {
    expect(typeof Recall.buildHistoryCorpus).toBe("function");
    expect(typeof Recall.recallTierD).toBe("function");
    expect(Recall.buildHistoryCorpus).toBe(Root.buildHistoryCorpus);
  });
});
