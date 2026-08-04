import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vite-plus/test";

import {
  evaluateLoupeVerificationReleaseGate,
  runLoupeVerification,
  type LoupeVerificationRun,
} from "../src/verifier.ts";
import {
  APPROVED_PROVIDER_REF,
  FORBIDDEN_PROVIDER_REF,
  overrideRuntimeReceipt,
  simulatedControlPlane,
  simulationSpec,
  type SimulationOptions,
} from "./simulated-control-plane.ts";

/**
 * The verifier drives its own executor.
 *
 * Nothing below hands the verifier evidence, a worker receipt, an initial
 * verdict, or a lifecycle authority — the exported surface has nowhere to put
 * them. A test supplies a spec, a control-plane transport and a ledger
 * directory, exactly as a live run does, and every falsifier is expressed the
 * only way an attacker could express it: by making the control plane or the
 * guest answer differently.
 *
 * The transport here is a simulation, so `evidenceProvenance` comes out
 * `conformance_vector` and no run in this file can reach a confirmation however
 * agreeable the simulated sandbox is. Acceptance evidence lives in
 * `verifier-live.test.ts`, against a transcript recorded from admitted
 * `live_gce` sandboxes.
 */

const ledgerDirectory = () => mkdtempSync(join(tmpdir(), "loupe-ledger-"));

const run = async (
  options: SimulationOptions = {},
  ledger: string = ledgerDirectory(),
): Promise<LoupeVerificationRun> => {
  const spec = options.spec ?? simulationSpec();
  return await runLoupeVerification({
    spec,
    controlPlane: simulatedControlPlane({ ...options, spec }),
    ledgerDirectory: ledger,
  });
};

describe("Loupe verification driven by the verifier", () => {
  it("measures, locks, then runs the controls, and caps a simulated origin at inconclusive", async () => {
    const phases: Array<string> = [];
    const spec = simulationSpec();
    const ledger = ledgerDirectory();
    const outcome = await runLoupeVerification({
      spec,
      controlPlane: simulatedControlPlane({ spec }),
      ledgerDirectory: ledger,
      onPhase: (phase) => phases.push(phase),
    });

    expect(phases).toEqual(["mechanical", "lock", "control_vulnerable", "control_fixed"]);
    expect(outcome.runs.map((entry) => entry.role)).toEqual([
      "mechanical",
      "control_vulnerable",
      "control_fixed",
    ]);
    // Three isolated sandboxes, because one honest control pair cannot run in one.
    expect(new Set(outcome.runs.map((entry) => entry.sandboxRef)).size).toBe(3);

    // The ordering is the point and it is derived, not asserted by a producer.
    const lockedAt = Date.parse(String(outcome.initialVerdict.lockedAt));
    for (const receipt of outcome.mechanicalEvidence) {
      expect(Date.parse(String(receipt.observedAt))).toBeLessThanOrEqual(lockedAt);
    }
    for (const receipt of outcome.controlEvidence) {
      expect(Date.parse(String(receipt.observedAt))).toBeGreaterThan(lockedAt);
    }

    expect(outcome.result.derivedVulnerableTestOutcome).toBe("failure");
    expect(outcome.result.derivedFixedTestOutcome).toBe("success");
    expect(outcome.result.vulnerableControlPassed).toBe(true);
    expect(outcome.result.fixedControlPassed).toBe(true);
    expect(outcome.result.completionAuthority).toBe("adapter_atomic_result");
    expect(outcome.result.initialVerdictAuthority).toBe("durable_first_verdict_ledger");
    expect(outcome.result.productMode).toBe("discovery_only");

    // THE FILE-LEVEL POINT. Everything above went perfectly and still cannot be
    // an acceptance claim, because the provenance was derived from the
    // transport rather than declared by whoever wrote this file.
    expect(outcome.result.evidenceProvenance).toBe("conformance_vector");
    expect(outcome.result.evidenceOriginRef).toBe(
      simulatedControlPlane({ spec, startedAt: "2026-08-01T16:00:00.000Z" }).originRef,
    );
    expect(outcome.result.outcome).toBe("inconclusive");
    expect(outcome.result.evidenceTier).toBe("executed");

    const gate = evaluateLoupeVerificationReleaseGate({
      gateRef: "gate.verification.simulated.v1",
      results: [outcome.result],
      evaluatedAt: outcome.completedAt,
    });
    expect(gate.productMode).toBe("discovery_only");
    expect(gate.blockerRefs).toContain("blocker.verification.admittedProvenancePassed");
  });

  it("durably locks the verdict before either control sandbox is created", async () => {
    const created: Array<string> = [];
    const spec = simulationSpec();
    const ledger = ledgerDirectory();
    const phases: Array<string> = [];
    await runLoupeVerification({
      spec,
      controlPlane: simulatedControlPlane({
        spec,
        perturb: (context, response) => {
          if (context.route === "runtime_operation" && context.action === "create") {
            created.push(`${phases.at(-1) ?? "none"}:${context.sandboxRef}`);
          }
          return response;
        },
      }),
      ledgerDirectory: ledger,
      onPhase: (phase) => phases.push(phase),
    });
    // Only the mechanical sandbox exists before the lock phase is entered.
    expect(created.filter((entry) => entry.startsWith("mechanical:"))).toHaveLength(1);
    expect(created.filter((entry) => entry.startsWith("lock:"))).toHaveLength(0);
    expect(created).toHaveLength(3);
  });

  describe("what the control plane cannot get away with", () => {
    it("refuses a control plane that admits a different guest image than requested", async () => {
      await expect(
        run({
          perturb: overrideRuntimeReceipt(
            (context) => context.action === "create",
            { imageDigest: `sha256:${"a".repeat(64)}` },
          ),
        }),
      ).rejects.toThrow("different guest image");
    });

    it("refuses a control plane that admits a different worker profile than requested", async () => {
      await expect(
        run({
          perturb: overrideRuntimeReceipt(
            (context) => context.action === "create",
            { profileDigest: `sha256:${"b".repeat(64)}` },
          ),
        }),
      ).rejects.toThrow("different worker profile");
    });

    it("refuses a create that never reported an observed ready state", async () => {
      await expect(
        run({
          perturb: overrideRuntimeReceipt(
            (context) => context.action === "create",
            { readinessObserved: false },
          ),
        }),
      ).rejects.toThrow("observed ready state");
    });

    it("refuses a delete that never reported observed cleanup", async () => {
      await expect(
        run({
          perturb: overrideRuntimeReceipt(
            (context) => context.action === "delete",
            { cleanupObserved: false },
          ),
        }),
      ).rejects.toThrow("observed cleanup");
    });

    it("refuses a control plane that answers about a sandbox this run never created", async () => {
      await expect(
        run({
          perturb: overrideRuntimeReceipt(
            (context) => context.action === "create",
            { sandboxRef: "sandbox.somebody-elses.v1" },
          ),
        }),
      ).rejects.toThrow("a sandbox this verification did not create");
    });

    it("refuses a guest image digest that changed part way through a run", async () => {
      await expect(
        run({
          perturb: overrideRuntimeReceipt(
            (context) => context.action === "delete",
            { imageDigest: `sha256:${"c".repeat(64)}` },
          ),
        }),
      ).rejects.toThrow("not stable across this run");
    });

    it("refuses a run whose measured cost exceeded its sandbox budget", async () => {
      await expect(
        run({
          perturb: overrideRuntimeReceipt(
            (context) => context.action === "delete",
            { measuredCostMicrousd: 10_000_000 },
          ),
        }),
      ).rejects.toThrow("exceeded the sandbox budget");
    });

    it("refuses a guest capture that did not build the pinned commit", async () => {
      const spec = simulationSpec();
      await expect(
        run({
          spec,
          perturb: (context, response) => {
            if (context.route !== "guest_io" || context.action !== "read_artifact") return response;
            const body = response.body as { readonly contentBase64: string };
            const capture = JSON.parse(
              Buffer.from(body.contentBase64, "base64").toString("utf8"),
            ) as Record<string, unknown>;
            if (capture.schema === undefined) return response;
            return {
              ...response,
              body: {
                contentBase64: Buffer.from(
                  JSON.stringify({ ...capture, targetCommit: "0".repeat(40) }),
                  "utf8",
                ).toString("base64"),
              },
            };
          },
        }),
      ).rejects.toThrow("did not build the pinned commit");
    });
  });

  describe("what the measurement decides", () => {
    it("dismisses instead of confirming when the vulnerable build links the approved object", async () => {
      const outcome = await run({ providerRefFor: () => APPROVED_PROVIDER_REF });
      expect(outcome.result.outcome).toBe("dismissed");
      expect(outcome.result.evidenceTier).toBe("source_observed");
      // A dismissal costs one sandbox. No control pair is run for a finding the
      // mechanical tier did not reproduce.
      expect(outcome.runs).toHaveLength(1);
      expect(outcome.controlEvidence).toHaveLength(0);
    });

    it("refuses a confirmation when the detector also fails on the fixed build", async () => {
      const outcome = await run({ providerRefFor: () => FORBIDDEN_PROVIDER_REF });
      expect(outcome.result.derivedFixedTestOutcome).toBe("failure");
      expect(outcome.result.fixedControlPassed).toBe(false);
      expect(outcome.result.outcome).toBe("inconclusive");
    });

    it("refuses a confirmation when the detector also passes on the vulnerable build", async () => {
      const outcome = await run({
        providerRefFor: () => APPROVED_PROVIDER_REF,
        forbiddenSymbolsFor: (variant) => (variant === "fixed" ? 0 : 3),
      });
      // The mechanical tier now dismisses, which is the honest answer, and the
      // control pair never runs. A vulnerable build the detector accepts cannot
      // reach a confirmation by any route.
      expect(outcome.result.outcome).toBe("dismissed");
    });

    it("refuses a fixed control that exited cleanly but kept no report", async () => {
      const outcome = await run({
        perturb: (context, response) => {
          // The fixed guest exits zero and keeps nothing. Its report artifact
          // read comes back empty, so there is no result to digest.
          if (!context.sandboxRef.includes("control-fixed")) return response;
          if (context.action !== "read_artifact") return response;
          if (context.path?.endsWith("report.json") !== true) return response;
          return { status: 200, body: {} };
        },
      });
      expect(outcome.result.derivedFixedTestOutcome).toBe("not_observed");
      expect(outcome.result.fixedControlPassed).toBe(false);
      expect(outcome.result.outcome).toBe("inconclusive");
    });

    it("refuses a control pair that resolved to the same immutable target", async () => {
      const spec = simulationSpec({
        targets: {
          repositoryRef: "repository.github.Coldcard.firmware",
          vulnerable: {
            commitSha: "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
            gitTreeSha: "7abc9a4c680b5623fc8a64f70555dd2d3802e488",
          },
          fixed: {
            commitSha: "bcc2c382a324690a2fcf972c0bac3b79bf923f7b",
            gitTreeSha: "7abc9a4c680b5623fc8a64f70555dd2d3802e488",
          },
        },
      });
      await expect(
        run({
          spec,
          perturb: (context, response) => {
            if (context.route !== "guest_io" || context.action !== "read_artifact") return response;
            const body = response.body as { readonly contentBase64: string };
            const capture = JSON.parse(
              Buffer.from(body.contentBase64, "base64").toString("utf8"),
            ) as Record<string, unknown>;
            if (capture.schema === undefined) return response;
            // Both revisions now report the same bundle digest and the same commit.
            return {
              ...response,
              body: {
                contentBase64: Buffer.from(
                  JSON.stringify({ ...capture, variant: capture.variant, sourceBundleDigest: `sha256:${"d".repeat(64)}` }),
                  "utf8",
                ).toString("base64"),
              },
            };
          },
        }),
      ).rejects.toThrow("same immutable target");
    });
  });

  describe("the durable first verdict", () => {
    it("refuses to relock when the ledger already holds a different verdict", async () => {
      const ledger = ledgerDirectory();
      const first = await run({}, ledger);
      expect(first.result.outcome).toBe("inconclusive");

      // Same verification ref, a run whose measurement disagrees. The stored
      // verdict wins and the second run refuses rather than overwriting it.
      await expect(
        run({ providerRefFor: () => APPROVED_PROVIDER_REF }, ledger),
      ).rejects.toThrow("already durably locked");
    });

    it("re-derives the identical verdict when the same run is repeated", async () => {
      const ledger = ledgerDirectory();
      const first = await run({}, ledger);
      const second = await run({}, ledger);
      expect(second.result.initialVerdictDigest).toBe(first.result.initialVerdictDigest);
      expect(second.result.initialVerdictRef).toBe(first.result.initialVerdictRef);
    });
  });
});
