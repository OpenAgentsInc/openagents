import { Schema } from "effect";

import type { DseCompiledArtifactV1 } from "@openagentsinc/dse";
import { CompiledArtifact } from "@openagentsinc/dse";

import { signatures } from "../../../autopilot-worker/src/dseCatalog";

/**
 * Phase 7: pinned judge artifact used by judge-based reward bundles.
 *
 * This artifact must be stable and replayable:
 * - compiled_id MUST equal hashes.paramsHash (params canonical hash).
 * - hashes.inputSchemaHash / outputSchemaHash / promptIrHash MUST match the judge signature contract.
 *
 * If you change the judge signature contract or defaults, update these hashes and compiled_id.
 */
export const THREAD_SUMMARY_JUDGE_ARTIFACT_V1: DseCompiledArtifactV1 = {
  format: "openagents.dse.compiled_artifact",
  formatVersion: 1,
  signatureId: "@openagents/autopilot/judge/ThreadSummaryQuality.v1",
  compiled_id: "sha256:c287f9f20d8683469f4418ffeefada8dcce77a89e08b57f71d7c6a19cbb7bd6c",
  createdAt: "2026-02-10T00:00:00.000Z",
  hashes: {
    inputSchemaHash: "sha256:49a4c4e4aeae6683f31481385d3464a45eca8c3aed883c84bf3aa7e2427b7ba9",
    outputSchemaHash: "sha256:171c9338457fd69a8dcba58602cb26e6d84dd8c008e85dcd1456b51afb2ffa8e",
    promptIrHash: "sha256:1ee437d788031328d9488b4a95fc0515a3cb95e0949da6cde86003ce79b22d4d",
    paramsHash: "sha256:c287f9f20d8683469f4418ffeefada8dcce77a89e08b57f71d7c6a19cbb7bd6c",
  },
  params: signatures.judge_thread_summary_quality.defaults.params,
  eval: { evalVersion: 1, kind: "unscored" },
  optimizer: { id: "pinned_judge.v1" },
  provenance: { compilerVersion: "pinned" },
};

export const PINNED_DSE_ARTIFACTS: ReadonlyArray<DseCompiledArtifactV1> = [THREAD_SUMMARY_JUDGE_ARTIFACT_V1];

// Fail fast in development/tests if the pinned artifact is invalid.
Schema.decodeUnknownSync(CompiledArtifact.DseCompiledArtifactV1Schema)(THREAD_SUMMARY_JUDGE_ARTIFACT_V1);
