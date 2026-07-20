import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { Schema } from "effect";
import { expect, test } from "vite-plus/test";

import { IDE_PORTABLE_REQUIRED_FAULT_CASES } from "../../openagents-desktop/src/ide/portable-evidence-contract.js";
import {
  Ide13OwnerLocalRealFaultMatrixReceiptSchema,
  runIde13OwnerLocalRealFaultMatrix,
} from "./ide13-owner-local-real-fault-matrix.js";
import { Ide13OwnerLocalRecoveryFaultReceiptSchema } from "./ide13-owner-local-recovery-faults.js";

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRealFaultMatrixReceiptSchema);
const decodeRecoveryReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRecoveryFaultReceiptSchema);

test("runs honest owner-local injected faults and records the missing seams", async () => {
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
      passedRealLocalCount: 14,
      notRunCount: 13,
      acceptanceReady: false,
    });
    expect(receipt.cases.filter((fault) => fault.outcome === "passed")).toHaveLength(14);
    expect(receipt.cases.filter((fault) => fault.outcome === "not_run")).toHaveLength(13);
    const recoveryReceipt = decodeRecoveryReceipt(
      JSON.parse(
        await readFile(
          resolve(
            import.meta.dirname,
            "../../openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-recovery-faults.json",
          ),
          "utf8",
        ),
      ),
      { onExcessProperty: "error" },
    );
    const checkpointStoreCrashProof = recoveryReceipt.cases.find(
      (row) => row.scenario === "checkpoint_store_crash",
    );
    if (checkpointStoreCrashProof === undefined) {
      throw new Error("checkpoint store crash recovery proof is absent");
    }
    expect(receipt.cases.find((fault) => fault.scenario === "checkpoint_store_crash")).toEqual({
      ...checkpointStoreCrashProof,
      faultRef: "fault.ide13.owner-local.checkpoint_store_crash",
      phase: null,
    });
    expect(receipt.safety.proofReceiptRefs).toEqual(
      expect.arrayContaining([
        checkpointStoreCrashProof.receiptRef,
        checkpointStoreCrashProof.recoveryPointRef,
      ]),
    );
    expect(
      receipt.cases
        .filter((fault) =>
          ["old_generation_command", "dual_attachment_claim", "source_revocation_failure"].includes(
            fault.scenario,
          ),
        )
        .map((fault) => [fault.scenario, fault.evidenceClass, fault.outcome]),
    ).toEqual([
      ["old_generation_command", "real_local", "passed"],
      ["source_revocation_failure", "real_local", "passed"],
      ["dual_attachment_claim", "real_local", "passed"],
    ]);
    expect(
      receipt.cases
        .filter((fault) => ["duplicate_event", "reordered_event"].includes(fault.scenario))
        .map((fault) => [fault.scenario, fault.evidenceClass, fault.outcome]),
    ).toEqual([
      ["duplicate_event", "real_local", "passed"],
      ["reordered_event", "real_local", "passed"],
    ]);
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

test.each([
  {
    name: "candidate ancestry",
    mutate: (receipt: Record<string, unknown>) => ({
      ...receipt,
      candidateCommitSha: "0000000000000000000000000000000000000000",
    }),
    message: "checkpoint store crash proof candidate is not an ancestor",
  },
  {
    name: "base",
    mutate: (receipt: Record<string, unknown>) => ({
      ...receipt,
      baseCommitSha: "f6c4c669d032ad5c06518c7cbe6e7a6788ab540d",
    }),
    message: "checkpoint store crash proof base does not match the fault matrix",
  },
  {
    name: "identity",
    mutate: (receipt: Record<string, unknown>) => ({
      ...receipt,
      cases: (receipt.cases as Array<Record<string, unknown>>).map((row) =>
        row.scenario === "checkpoint_store_crash"
          ? { ...row, injectedFaultRef: "injected-fault.ide13.wrong-boundary" }
          : row,
      ),
    }),
    message: "checkpoint store crash proof identity or result is invalid",
  },
  {
    name: "residue",
    mutate: (receipt: Record<string, unknown>) => ({
      ...receipt,
      safety: {
        ...(receipt.safety as Record<string, unknown>),
        checkpointCiphertextResidueCount: 1,
      },
    }),
    message: "checkpointCiphertextResidueCount",
  },
])("rejects a checkpoint store crash proof with mismatched $name", async ({ mutate, message }) => {
  const root = await mkdtemp(join(tmpdir(), "ide13-owner-local-fault-proof-test-"));
  const repositoryRoot = resolve(import.meta.dirname, "../../..");
  const sourcePath = join(
    repositoryRoot,
    "apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-owner-local-recovery-faults.json",
  );
  const recoveryReceiptPath = join(root, "recovery.json");
  try {
    const source = JSON.parse(await readFile(sourcePath, "utf8")) as Record<string, unknown>;
    await writeFile(recoveryReceiptPath, `${JSON.stringify(mutate(source))}\n`, "utf8");
    await expect(
      runIde13OwnerLocalRealFaultMatrix({ recoveryReceiptPath, repositoryRoot }),
    ).rejects.toThrow(message);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects a candidate that omits later implementation changes", async () => {
  await expect(
    runIde13OwnerLocalRealFaultMatrix({
      candidateCommitSha: "f6c4c669d032ad5c06518c7cbe6e7a6788ab540d",
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    }),
  ).rejects.toThrow("fault matrix candidate omits an implementation change");
});
