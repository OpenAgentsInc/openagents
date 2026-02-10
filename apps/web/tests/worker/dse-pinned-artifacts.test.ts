import { describe, expect, it } from "vitest";
import { Effect } from "effect";

import { Hashes } from "@openagentsinc/dse";

import { signatures } from "../../../autopilot-worker/src/dseCatalog";
import { THREAD_SUMMARY_JUDGE_ARTIFACT_V1 } from "../../src/effuse-host/dsePinnedArtifacts";

describe("Pinned DSE artifacts", () => {
  it("THREAD_SUMMARY_JUDGE_ARTIFACT_V1 hashes match the judge signature contract + params", async () => {
    const sig = signatures.judge_thread_summary_quality;
    const params = sig.defaults.params;

    const [inputSchemaHash, outputSchemaHash, promptIrHash, paramsHash] = await Effect.runPromise(
      Effect.all([
        Hashes.schemaJsonHash(sig.input),
        Hashes.schemaJsonHash(sig.output),
        Hashes.promptIrHash(sig.prompt),
        Hashes.paramsHash(params),
      ]),
    );

    expect(THREAD_SUMMARY_JUDGE_ARTIFACT_V1.signatureId).toBe(sig.id);
    expect(THREAD_SUMMARY_JUDGE_ARTIFACT_V1.compiled_id).toBe(paramsHash);
    expect(THREAD_SUMMARY_JUDGE_ARTIFACT_V1.hashes.paramsHash).toBe(paramsHash);
    expect(THREAD_SUMMARY_JUDGE_ARTIFACT_V1.hashes.inputSchemaHash).toBe(inputSchemaHash);
    expect(THREAD_SUMMARY_JUDGE_ARTIFACT_V1.hashes.outputSchemaHash).toBe(outputSchemaHash);
    expect(THREAD_SUMMARY_JUDGE_ARTIFACT_V1.hashes.promptIrHash).toBe(promptIrHash);
  });
});

