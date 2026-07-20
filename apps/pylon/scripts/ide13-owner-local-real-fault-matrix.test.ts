import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { Schema } from "effect";
import { expect, test } from "vite-plus/test";

import { IDE_PORTABLE_REQUIRED_FAULT_CASES } from "../../openagents-desktop/src/ide/portable-evidence-contract.js";
import {
  Ide13OwnerLocalRealFaultMatrixReceiptSchema,
  runIde13OwnerLocalRealFaultMatrix,
} from "./ide13-owner-local-real-fault-matrix.js";

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRealFaultMatrixReceiptSchema);

test("runs honest owner-local transition partition cases and records the missing seams", async () => {
  const root = await mkdtemp(join(tmpdir(), "ide13-owner-local-fault-matrix-test-"));
  const outputPath = join(root, "receipt.json");
  try {
    const receipt = await runIde13OwnerLocalRealFaultMatrix({
      outputPath,
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    });
    expect(
      decodeReceipt(JSON.parse(await readFile(outputPath, "utf8")), {
        onExcessProperty: "error",
      }),
    ).toEqual(receipt);
    expect(receipt.cases).toHaveLength(IDE_PORTABLE_REQUIRED_FAULT_CASES.length);
    expect(receipt.summary).toEqual({
      requiredCaseCount: IDE_PORTABLE_REQUIRED_FAULT_CASES.length,
      passedRealLocalCount: 8,
      notRunCount: 19,
      acceptanceReady: false,
    });
    expect(receipt.cases.filter((fault) => fault.outcome === "passed")).toHaveLength(8);
    expect(receipt.cases.filter((fault) => fault.outcome === "not_run")).toHaveLength(19);
    expect(
      receipt.cases
        .filter((fault) => fault.outcome === "passed")
        .every(
          (fault) =>
            fault.evidenceClass === "real_local" &&
            fault.recoveryPointRef !== null &&
            fault.receiptRef !== null &&
            fault.elapsedMilliseconds <= fault.deadlineMilliseconds,
        ),
    ).toBe(true);
    expect(receipt.safety).toMatchObject({
      secondWriterObserved: false,
      staleMutationAccepted: false,
      forbiddenMaterialProjected: false,
      orphanPtyCount: 0,
      orphanLspCount: 0,
      orphanWatcherCount: 0,
      custodyObjectResidueCount: 0,
      capabilityLeaseResidueCount: 0,
      queueRowResidueCount: 0,
      sqliteResidueCount: 0,
      sessionResidueCount: 0,
    });
    expect(JSON.stringify(receipt)).not.toMatch(
      /\/Users\/|\/private\/tmp\/|Bearer|password|processId|pid/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 180_000);

test("rejects a candidate that omits later implementation changes", async () => {
  await expect(
    runIde13OwnerLocalRealFaultMatrix({
      candidateCommitSha: "f6c4c669d032ad5c06518c7cbe6e7a6788ab540d",
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    }),
  ).rejects.toThrow("fault matrix candidate omits an implementation change");
});
