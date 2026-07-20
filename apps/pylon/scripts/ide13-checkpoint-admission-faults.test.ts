import { access, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "vite-plus/test";
import { Schema } from "effect";

import {
  Ide13CheckpointAdmissionFaultReceiptSchema,
  runIde13CheckpointAdmissionFaults,
} from "./ide13-checkpoint-admission-faults.js";

const isReceipt = Schema.is(Ide13CheckpointAdmissionFaultReceiptSchema);

test("records production-component checkpoint and admission faults without acceptance claims", async () => {
  const outputRoot = await mkdtemp(join(tmpdir(), "oa-ide13-checkpoint-fault-receipt-"));
  const outputPath = join(outputRoot, "receipt.json");
  try {
    const receipt = await runIde13CheckpointAdmissionFaults({ outputPath });
    expect(isReceipt(receipt)).toBe(true);
    expect(receipt.cases).toHaveLength(8);
    expect(receipt.summary).toEqual({
      requiredCaseCount: 8,
      passedSimulatorCount: 6,
      notRunCount: 2,
      acceptanceContributionCount: 0,
      acceptanceReady: false,
    });
    expect(
      receipt.cases.filter((row) => row.evidenceClass === "simulator").map((row) => row.scenario),
    ).toEqual([
      "corrupt_checkpoint",
      "truncated_checkpoint",
      "wrong_schema_checkpoint",
      "missing_artifact",
      "auth_expiry_revocation",
      "destination_boot_failure",
    ]);
    expect(
      receipt.cases.filter((row) => row.evidenceClass === "not_run").map((row) => row.scenario),
    ).toEqual(["provider_capability_drift", "source_revocation_failure"]);
    expect(receipt.cases.every((row) => row.residueCount === 0)).toBe(true);
    await expect(access(outputPath)).resolves.toBeUndefined();
  } finally {
    await rm(outputRoot, { recursive: true, force: true });
  }
});
