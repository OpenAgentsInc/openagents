import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vite-plus/test";

import {
  evaluateLoupeVerificationReleaseGate,
  executeLoupeVerification,
  type LoupeVerificationBackend,
  type LoupeVerificationPlan,
} from "../src/verifier.ts";

/**
 * Live acceptance evidence for openagents #9294 (OFR-007).
 *
 * `verifier.test.ts` is a conformance vector: repeated-character digests, a
 * lifecycle authority written by the same file that writes the evidence it
 * authorises, and a control pair that is asserted rather than run. It exercises
 * ordering and locking, and it is explicitly capped at `discovery_only`.
 *
 * This file is the other half. Everything it feeds the verifier was measured by
 * `scripts/cloud/coldcard-loupe-verification-live.ts`:
 *
 *  - the mechanical tier is what an admitted OpenAgents Cloud `live_gce`
 *    sandbox observed while building the pinned vulnerable Coldcard MK4 tree;
 *  - the initial verdict was then written to the durable first-verdict ledger
 *    checked in beside this fixture, before any control existed;
 *  - the two control tests are the provider-provenance detector executing
 *    inside two further admitted sandboxes, and their exit statuses are what
 *    the managed-sandbox I/O route observed.
 *
 * The falsifiers at the bottom are what stop this from being a rubber stamp. If
 * the verifier ever stopped discriminating, they would pass with the wrong
 * verdict rather than failing.
 */

interface LiveRun {
  readonly plan: Record<string, unknown>;
  readonly initialVerdict: {
    readonly verdictRef: string;
    readonly outcome: "confirmed" | "dismissed" | "inconclusive";
    readonly rationaleDigest: string;
    readonly lockedAt: string;
  };
  readonly mechanicalEvidence: ReadonlyArray<Record<string, unknown>>;
  readonly controlEvidence: ReadonlyArray<Record<string, unknown>>;
  readonly admittedWorkerReceipts: ReadonlyArray<Record<string, unknown>>;
  readonly detector: { readonly detectorDigest: string };
  readonly runs: ReadonlyArray<{
    readonly role: string;
    readonly variant: string;
    readonly sandboxRef: string;
    readonly buildExitStatus: number;
    readonly detectorExitStatus?: number;
    readonly detectorReport?: {
      readonly observedProviderRefs: ReadonlyArray<string>;
      readonly observedForbiddenSymbolCount: number;
      readonly enumeratedSymbolCount: number;
      readonly satisfied: boolean;
    };
  }>;
  readonly completedAt: string;
}

const fixture = (name: string) =>
  fileURLToPath(new URL(`../../../fixtures/forensics/coldcard/${name}`, import.meta.url));

const run = JSON.parse(readFileSync(fixture("loupe-verification-live-run.v1.json"), "utf8")) as LiveRun;
const ledger = JSON.parse(
  readFileSync(fixture("loupe-first-verdict-ledger.v1.json"), "utf8"),
) as { readonly verdicts: Record<string, { readonly lockedAt: string; readonly verdictRef: string }> };

const verificationRef = String(run.plan.verificationRef);
const roleRun = (role: string) => {
  const found = run.runs.find((entry) => entry.role === role);
  if (found === undefined) throw new Error(`live run missing ${role}`);
  return found;
};

/**
 * The backend replays exactly what the live run recorded. It invents nothing:
 * the lifecycle authority answers only from the receipts the orchestrating
 * process observed, and the verdict ledger answers from the checked-in file.
 */
const liveBackend = (overrides: Partial<LoupeVerificationBackend> = {}): LoupeVerificationBackend => ({
  collectMechanicalEvidence: async () => run.mechanicalEvidence,
  submitInitialVerdict: async () => ({
    verdictRef: run.initialVerdict.verdictRef,
    outcome: run.initialVerdict.outcome,
    rationaleDigest: run.initialVerdict.rationaleDigest,
    lockedAt: run.initialVerdict.lockedAt,
  }),
  applyPocAndRunControls: async () => run.controlEvidence,
  resolveAdmittedWorkerReceipt: async (_plan, workerReceiptRef) =>
    run.admittedWorkerReceipts.find((receipt) => receipt.receiptRef === workerReceiptRef),
  commitInitialVerdict: async (_plan, candidate) => {
    const stored = ledger.verdicts[verificationRef];
    if (stored === undefined) throw new Error("the durable ledger holds no verdict for this run");
    return { ...candidate, ...stored };
  },
  ...overrides,
});

const execute = (overrides: Partial<LoupeVerificationBackend> = {}, plan: unknown = run.plan) =>
  executeLoupeVerification(plan, liveBackend(overrides), run.completedAt);

describe("Loupe verification against admitted-worker controls", () => {
  it("measured a vulnerable build that links the deterministic fallback and a fixed build that does not", () => {
    const vulnerable = roleRun("control_vulnerable");
    const fixed = roleRun("control_fixed");
    expect(vulnerable.buildExitStatus).toBe(0);
    expect(fixed.buildExitStatus).toBe(0);
    // The two builds must not agree about which object provides rng_get. If
    // they ever do, the control pair proves nothing and this fails first.
    expect(vulnerable.detectorReport?.observedProviderRefs).not.toEqual(
      fixed.detectorReport?.observedProviderRefs,
    );
    expect(fixed.detectorReport?.observedProviderRefs).toEqual([
      "provider.object.boards/COLDCARD_MK4/rng.o",
    ]);
    expect(fixed.detectorReport?.observedForbiddenSymbolCount).toBe(0);
    expect(vulnerable.detectorReport?.observedForbiddenSymbolCount).toBeGreaterThan(0);
    expect(fixed.detectorReport?.enumeratedSymbolCount).toBeGreaterThan(1_000);
  });

  it("observed the detector failing on the vulnerable target and passing on the fixed one", () => {
    expect(roleRun("control_vulnerable").detectorExitStatus).toBe(1);
    expect(roleRun("control_fixed").detectorExitStatus).toBe(0);
  });

  it("locked the initial verdict before either control was observed", () => {
    const lockedAt = Date.parse(run.initialVerdict.lockedAt);
    expect(Number.isNaN(lockedAt)).toBe(false);
    expect(ledger.verdicts[verificationRef]?.verdictRef).toBe(run.initialVerdict.verdictRef);
    expect(ledger.verdicts[verificationRef]?.lockedAt).toBe(run.initialVerdict.lockedAt);
    for (const receipt of run.controlEvidence) {
      expect(Date.parse(String(receipt.observedAt))).toBeGreaterThan(lockedAt);
    }
    for (const receipt of run.mechanicalEvidence) {
      expect(Date.parse(String(receipt.observedAt))).toBeLessThanOrEqual(lockedAt);
    }
  });

  it("ran the mechanical tier and each control in its own admitted sandbox", () => {
    const sandboxes = new Set(run.runs.map((entry) => entry.sandboxRef));
    expect(sandboxes.size).toBe(3);
    const receiptRefs = new Set(run.admittedWorkerReceipts.map((receipt) => receipt.receiptRef));
    expect(receiptRefs.size).toBe(3);
    for (const receipt of run.admittedWorkerReceipts) {
      expect(receipt.lifecycleState).toBe("admitted");
      expect(receipt.exact).toBe(true);
    }
  });

  it("confirms the finding and reaches independent verification", async () => {
    const result = await execute();
    expect(result.evidenceProvenance).toBe("admitted_worker_run");
    expect(result.outcome).toBe("confirmed");
    expect(result.evidenceTier).toBe("independently_verified");
    expect(result.derivedVulnerableTestOutcome).toBe("failure");
    expect(result.derivedFixedTestOutcome).toBe("success");
    expect(result.vulnerableControlPassed).toBe(true);
    expect(result.fixedControlPassed).toBe(true);
    expect(result.admittedWorkerReceiptsResolved).toBe(true);
    expect(result.initialVerdictAuthority).toBe("durable_first_verdict_ledger");

    const gate = evaluateLoupeVerificationReleaseGate({
      gateRef: "gate.verification.coldcard.live.v1",
      results: [result],
      evaluatedAt: run.completedAt,
    });
    expect(gate.productMode).toBe("independent_verification");
    expect(gate.blockerRefs).toEqual([]);
  });

  // -------------------------------------------------------------------------
  // Falsifiers
  // -------------------------------------------------------------------------

  it("refuses to confirm when the vulnerable control did not fail", async () => {
    const controls = run.controlEvidence.map((receipt) =>
      receipt.controlRevision === "vulnerable"
        ? {
            ...receipt,
            observedTermination: {
              status: "observed",
              exitStatus: 0,
              resultArtifactDigest: run.detector.detectorDigest,
            },
          }
        : receipt,
    );
    const result = await execute({ applyPocAndRunControls: async () => controls });
    expect(result.derivedVulnerableTestOutcome).toBe("success");
    expect(result.vulnerableControlPassed).toBe(false);
    expect(result.outcome).toBe("inconclusive");
  });

  it("refuses to confirm when the fixed control did not pass", async () => {
    const controls = run.controlEvidence.map((receipt) =>
      receipt.controlRevision === "fixed"
        ? { ...receipt, observedTermination: { status: "observed", exitStatus: 1 } }
        : receipt,
    );
    const result = await execute({ applyPocAndRunControls: async () => controls });
    expect(result.derivedFixedTestOutcome).toBe("failure");
    expect(result.fixedControlPassed).toBe(false);
    expect(result.outcome).toBe("inconclusive");
  });

  it("refuses a fixed control that exited cleanly without keeping its report", async () => {
    const controls = run.controlEvidence.map((receipt) =>
      receipt.controlRevision === "fixed"
        ? { ...receipt, observedTermination: { status: "observed", exitStatus: 0 } }
        : receipt,
    );
    const result = await execute({ applyPocAndRunControls: async () => controls });
    expect(result.derivedFixedTestOutcome).toBe("not_observed");
    expect(result.outcome).toBe("inconclusive");
  });

  it("refuses the same measured evidence once its provenance is downgraded", async () => {
    const result = await execute({}, { ...run.plan, evidenceProvenance: "conformance_vector" });
    expect(result.outcome).toBe("inconclusive");
    expect(result.evidenceTier).toBe("executed");
    const gate = evaluateLoupeVerificationReleaseGate({
      gateRef: "gate.verification.coldcard.live.downgraded.v1",
      results: [result],
      evaluatedAt: run.completedAt,
    });
    expect(gate.blockerRefs).toContain("blocker.verification.admittedProvenancePassed");
  });

  it("refuses when the lifecycle authority does not know a cited receipt", async () => {
    await expect(
      execute({
        resolveAdmittedWorkerReceipt: async (_plan, workerReceiptRef) =>
          workerReceiptRef === run.admittedWorkerReceipts.at(-1)?.receiptRef
            ? undefined
            : run.admittedWorkerReceipts.find((receipt) => receipt.receiptRef === workerReceiptRef),
      }),
    ).rejects.toThrow("cannot resolve");
  });

  it("refuses a control receipt attributed to the wrong admitted sandbox", async () => {
    const plan = run.plan as unknown as LoupeVerificationPlan;
    const mechanicalWorkerRef = plan.admittedWorkers[0]?.workerRef;
    const controls = run.controlEvidence.map((receipt) =>
      receipt.controlRevision === "fixed" ? { ...receipt, workerRef: mechanicalWorkerRef } : receipt,
    );
    await expect(execute({ applyPocAndRunControls: async () => controls })).rejects.toThrow(
      "admitted sandbox and resource generation",
    );
  });

  it("refuses a second, different verdict once the ledger holds one", async () => {
    await expect(
      execute({
        submitInitialVerdict: async () => ({
          verdictRef: "verdict.ofr007.second-opinion.v1",
          outcome: "dismissed",
          rationaleDigest: run.initialVerdict.rationaleDigest,
          lockedAt: run.initialVerdict.lockedAt,
        }),
      }),
    ).rejects.toThrow("already durably locked");
  });
});
