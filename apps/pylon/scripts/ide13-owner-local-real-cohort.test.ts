import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { Schema } from "effect";
import { expect, test } from "vite-plus/test";

import {
  type Ide13OwnerLocalEventFaultProof,
  Ide13OwnerLocalRealCohortReceiptSchema,
  runIde13OwnerLocalRealCohort,
} from "./ide13-owner-local-real-cohort.js";
import { PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF } from "../src/portable-executable-profile-catalog.js";

const decodeReceipt = Schema.decodeUnknownSync(Ide13OwnerLocalRealCohortReceiptSchema);

test("runs a real owner-local move, failback, abort, replay, and teardown cohort", async () => {
  const root = await mkdtemp(join(tmpdir(), "ide13-owner-local-cohort-test-"));
  const outputPath = join(root, "receipt.json");
  try {
    const receipt = await runIde13OwnerLocalRealCohort({
      outputPath,
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    });
    const persisted = decodeReceipt(JSON.parse(await readFile(outputPath, "utf8")), {
      onExcessProperty: "error",
    });

    expect(persisted).toEqual(receipt);
    expect(receipt.cohort).toMatchObject({
      targetClass: "owner_local",
      evidenceClass: "real_local",
      journeyScope: "full_move",
      adapter: { kind: "production" },
      capabilityState: "degraded",
    });
    expect(receipt.cohort.metrics).toHaveLength(16);
    expect(receipt.cohort.metrics.every((metric) => metric.passed)).toBe(true);
    expect(
      receipt.helpers
        .filter((helper) => helper.readiness === "ready")
        .map((helper) => helper.kind)
        .toSorted(),
    ).toEqual(["lsp", "pty", "watcher"]);
    expect(
      receipt.helpers
        .filter((helper) => helper.readiness === "unsupported")
        .map((helper) => helper.kind)
        .toSorted(),
    ).toEqual(["dap", "native"]);
    expect(receipt.authority.admittedExecutableProfileRefs).toEqual([
      PYLON_TYPESCRIPT_LSP_EXECUTABLE_PROFILE_REF,
    ]);
    expect(receipt.execution).toEqual({
      acceptedWorkRefCount: 0,
      controlSessionProcessLifecycle: "settled",
      executorResumed: false,
      omissionRef: "omission.ide13.owner-local.codex-executor-resumption-not-implemented",
    });
    expect(JSON.stringify(receipt)).not.toMatch(
      /\/Users\/|\/private\/tmp\/|password|Bearer|processId|pid/u,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test.each(["duplicate_event", "reordered_event"] as const)(
  "runs the source-controlled %s fault through the full owner-local composition",
  async (scenario) => {
    let proof: Ide13OwnerLocalEventFaultProof | null = null;
    const receipt = await runIde13OwnerLocalRealCohort({
      injectedEventFaultScenario: scenario,
      onInjectedEventFaultProof: (value) => {
        proof = value;
      },
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    });
    expect(proof).toMatchObject({
      scenario,
      productionBoundaryRef: expect.stringContaining("boundary.pylon.owner-local.destination"),
      injectedFaultRef: expect.stringContaining(`injected-fault.ide13.owner-local`),
      recoveryPointRef: receipt.cohort.journeys.mainJourneyReceiptRef,
    });
    expect(receipt.cohort.metrics.find((metric) => metric.metric === "resource_cleanup")?.p99).toBe(
      0,
    );
    expect(receipt.cohort.metrics.find((metric) => metric.metric === "queue")?.p99).toBe(0);
  },
  120_000,
);

test("rejects a candidate that omits later implementation changes", async () => {
  await expect(
    runIde13OwnerLocalRealCohort({
      candidateCommitSha: "f6c4c669d032ad5c06518c7cbe6e7a6788ab540d",
      repositoryRoot: resolve(import.meta.dirname, "../../.."),
    }),
  ).rejects.toThrow("owner-local cohort candidate omits an implementation change");
});
