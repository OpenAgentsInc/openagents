import { readFile } from "node:fs/promises";
import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  OPENAGENTS_STUDYBENCH_DATASET_PACKAGE_SCHEMA_REF,
  decodeOpenAgentsStudybenchDatasetPackage,
} from "../src";

const PUBLIC_RETAINED_ROWS_URL = new URL(
  "../../../../../docs/research/machine-studying/openagents-studybench/public-retained/openagents-launch-v0.jsonl",
  import.meta.url,
);

describe("OpenAgents public-retained StudyBench fixtures", () => {
  test("validate as an OpenAgents StudyBench dataset package", async () => {
    const text = await readFile(PUBLIC_RETAINED_ROWS_URL, "utf8");
    const tasks = text
      .trim()
      .split("\n")
      .filter((line) => line.trim().length > 0)
      .map((line) => JSON.parse(line));
    const decoded = await Effect.runPromise(
      decodeOpenAgentsStudybenchDatasetPackage({
        schemaRef: OPENAGENTS_STUDYBENCH_DATASET_PACKAGE_SCHEMA_REF,
        datasetRef: "dataset.openagents_studybench.public_retained.launch.v0",
        packageRef: "dataset_package.openagents_studybench.public_retained.launch.v0",
        packageVisibility: "openagents_public_retained",
        sourceBoundary: "public_refs_only",
        tasks,
      }),
    );

    expect(decoded.tasks).toHaveLength(10);
    expect(new Set(decoded.tasks.map((task) => task.topic))).toEqual(
      new Set([
        "launch_claims_and_promises",
        "tassadar_projection_truth",
        "settlement_and_wallet_truth",
        "customer_one_evidence",
        "forge_coder_repo_memory",
        "blueprint_probe_gepa_contracts",
        "pylon_assignment_wallet_readiness",
        "studybench_schema_adaptation",
        "studybench_answer_and_patch_modes",
        "product_promise_and_marketplace_gates",
      ]),
    );
    expect(decoded.tasks.every((task) => task.visibility === "openagents_public_retained")).toBe(true);
    expect(decoded.tasks.every((task) => task.rubric.some((claim) => claim.claim_type === "core"))).toBe(true);
  });
});
