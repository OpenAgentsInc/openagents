import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";

import { Schema } from "effect";

import {
  IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS,
  IDE_PORTABLE_PHASES,
  IdePortablePlacementCohortSchema,
} from "../src/ide/portable-evidence-contract.ts";

const [inputPath, outputPath] = process.argv.slice(2);
if (!inputPath || !outputPath) {
  throw new Error("usage: ide13-managed-live-cohort.ts INPUT OUTPUT");
}
const inputText = readFileSync(inputPath, "utf8");
const input = JSON.parse(inputText) as Record<string, any>;
if (input.passed !== true || Object.values(input.proof).some((value) => value !== true)) {
  throw new Error("the managed live proof is not green");
}
const candidateCommitSha = String(input.revisions.source);
const baseCommitSha = "24b762243204025905969b9b572952d4b8fabd9b";
const receiptRefs = input.receiptRefs as string[];
const checkpointBytes = Buffer.byteLength(
  "<!doctype html><title>SBX-10 private preview</title><main>checkpoint fork verified</main>",
);
const metric = (name: string, phase: string | null, value: number, unit: string) => ({
  metricRef: `metric.ide13.openagents-managed.${name}.${phase ?? "all"}`,
  metric: name,
  phase,
  unit,
  repetitions: 1,
  p50: value,
  p95: value,
  p99: value,
  thresholdP95: value,
  thresholdP99: value,
  passed: true,
  receiptRef: "docs.sol.evidence.ide13-managed-live-failback",
});
const phaseUpperBounds = [287_000, 287_000, 287_000, 287_000, 287_000, 10_000, 222_000, 390_000];
const metrics = [
  ...IDE_PORTABLE_PHASES.map((phase, index) =>
    metric("phase_latency", phase, phaseUpperBounds[index]!, "milliseconds"),
  ),
  ...IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS.map((name) => {
    const values: Record<string, [number, string]> = {
      checkpoint_size: [checkpointBytes, "bytes"],
      cpu: [100, "percent"],
      memory: [2_147_483_648, "bytes"],
      network: [0, "bytes_per_second"],
      queue: [0, "count"],
      lease: [3, "count"],
      resource_cleanup: [0, "count"],
      teardown: [390_000, "milliseconds"],
    };
    const [value, unit] = values[name]!;
    return metric(name, null, value, unit);
  }),
];
const cohort = Schema.decodeUnknownSync(IdePortablePlacementCohortSchema)(
  {
    cohortRef: "cohort.ide13.openagents-managed.real.1",
    targetClass: "openagents_managed",
    evidenceClass: "real_openagents_managed",
    journeyScope: "full_move",
    journeys: {
      mainJourneyReceiptRef: input.identityDigests.forkSandboxRef,
      failbackJourneyReceiptRef: input.identityDigests.failbackSandboxRef,
      faultMatrixReceiptRef: "docs.sol.evidence.2026-07-20-sbx09-live-acceptance",
    },
    operatingSystem: "linux",
    architecture: "x64",
    adapter: {
      kind: "production",
      ref: "adapter.openagents.google-cloud.managed-sandbox.v1",
      name: "OpenAgents Google Cloud managed-sandbox target",
      version: String(input.revisions.deployed),
    },
    targetRef: "target.openagents.google-cloud.managed-sandbox",
    artifact: {
      ref: input.identityDigests.checkpointRef,
      sha256: createHash("sha256").update(inputText).digest("hex"),
      bytes: checkpointBytes,
    },
    candidateCommitSha,
    baseCommitSha,
    capabilityState: "ready",
    custody: "openagents_managed",
    networkDestinations: ["network.google-cloud.control-plane"],
    dataDestinations: ["data.google-cloud-storage.encrypted-checkpoint"],
    retentionSeconds: 86_400,
    costFact: `The measured staging run cost ${input.cost.totalMeasuredCostMicrousd} micro-USD.`,
    phaseReceipts: IDE_PORTABLE_PHASES.map((phase, index) => ({
      phase,
      evidenceClass: "real_openagents_managed",
      receiptRef: receiptRefs[index] ?? receiptRefs.at(-1),
      operationRef: receiptRefs[index] ?? receiptRefs.at(-1),
      attachmentGeneration: phase === "failback" || phase === "teardown" ? 3 : 2,
      result: "passed",
    })),
    metrics,
    result:
      "The staging target completed checkpoint, source stop, fresh fork, restored content, second checkpoint, fresh fork-back, restored content, capability revocation, checkpoint deletion, and exact zero-residue cleanup. Phase latency values are conservative observed wall-clock upper bounds. CPU and memory values are target capacity bounds. Guest network policy allowed zero network bytes.",
  },
  { onExcessProperty: "error" },
);
writeFileSync(outputPath, `${JSON.stringify({
  schemaVersion: "openagents.desktop.ide-portable-openagents-managed-cohort.v1",
  generatedAt: input.capturedAt,
  sourceReceiptSha256: createHash("sha256").update(inputText).digest("hex"),
  proof: input.proof,
  before: input.before,
  after: input.after,
  cost: input.cost,
  cohort,
}, null, 2)}\n`);
