import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { Schema } from "effect";
import { expect, test } from "vite-plus/test";

import {
  Ide13OwnerLocalRecoveryFaultReceiptSchema,
  runIde13OwnerLocalRecoveryFaults,
} from "./ide13-owner-local-recovery-faults.js";

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRecoveryFaultReceiptSchema);

test("recovers truthful owner-local worker and checkpoint store faults", async () => {
  const root = await mkdtemp(join(tmpdir(), "ide13-owner-local-recovery-test-"));
  const outputPath = join(root, "receipt.json");
  try {
    const receipt = await runIde13OwnerLocalRecoveryFaults({
      outputPath,
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    });
    expect(
      decodeReceipt(JSON.parse(await readFile(outputPath, "utf8")), {
        onExcessProperty: "error",
      }),
    ).toEqual(receipt);
    expect(receipt.cases.map((row) => row.scenario)).toEqual([
      "coordinator_crash",
      "checkpoint_store_crash",
      "duplicate_event",
      "lease_expiry_clock_skew",
    ]);
    expect(receipt.cases.map((row) => row.evidenceClass)).toEqual([
      "simulator",
      "real_local",
      "simulator",
      "simulator",
    ]);
    expect(receipt.safety).toEqual({
      completionCount: 2,
      duplicateExecutionObserved: false,
      expiredClaimAccepted: false,
      checkpointCiphertextResidueCount: 0,
      journalEntryResidueCount: 0,
      forbiddenMaterialProjected: false,
    });
    expect(receipt.unsupported).toEqual([
      expect.objectContaining({ scenario: "reordered_event", evidenceClass: "not_run" }),
      expect.objectContaining({
        scenario: "cancellation_and_app_restart",
        evidenceClass: "not_run",
      }),
    ]);
    expect(JSON.stringify(receipt)).not.toMatch(
      /\/Users\/|\/private\/tmp\/|Bearer|password|processId|pid/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 180_000);
