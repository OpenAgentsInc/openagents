import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { gunzipSync } from "node:zlib";
import { describe, expect, it } from "vite-plus/test";

import {
  type ArtifactWitnessAssertion,
  type ArtifactWitnessCapture,
  evaluateArtifactWitness,
  evaluateColdcardArtifactWitnessSuite,
} from "../src/artifact-witness.ts";

/**
 * Acceptance evidence for openagents #9296 (OFR-014).
 *
 * Unlike `artifact-witness-fixtures.v1.json`, which is a conformance vector the
 * suite must REFUSE, the captures loaded here are measurements of three real
 * Coldcard MK4 firmware builds that ran inside admitted OpenAgents Cloud
 * `live_gce` managed sandboxes. This file is the check that the evaluator
 * reaches `Verified` on evidence that exists, and — just as importantly — that
 * it still discriminates: the same assertions applied to the wrong build are
 * violated, not quietly satisfied.
 */

const fixtureUrl = (name: string) =>
  fileURLToPath(new URL(`../../../fixtures/forensics/coldcard/${name}`, import.meta.url));

interface LiveRunFixture {
  readonly assertions: Record<string, ReadonlyArray<ArtifactWitnessAssertion>>;
  readonly pinnedRevisions: Record<string, string>;
  readonly witnessVersionRef: string;
}

const run = JSON.parse(
  readFileSync(fixtureUrl("artifact-witness-live-run.v1.json"), "utf8"),
) as LiveRunFixture;

const captures = JSON.parse(
  gunzipSync(readFileSync(fixtureUrl("artifact-witness-live-captures.v1.json.gz"))).toString("utf8"),
) as ReadonlyArray<ArtifactWitnessCapture>;

const captureFor = (variant: string): ArtifactWitnessCapture => {
  const found = captures.find((capture) => capture.variant === variant);
  if (found === undefined) throw new Error(`live capture missing for ${variant}`);
  return found;
};

const evaluate = (variant: string, assertions: ReadonlyArray<ArtifactWitnessAssertion>) =>
  evaluateArtifactWitness({
    assertions,
    capture: captureFor(variant),
    evaluatedAt: "2026-08-03T18:00:00.000Z",
    witnessVersionRef: run.witnessVersionRef,
  });

const reports = (["vulnerable", "fixed", "fault_build"] as const).map((variant) =>
  evaluate(variant, run.assertions[variant] ?? []),
);

describe("coldcard live artifact witness", () => {
  it("carries admitted-worker provenance bound to one guest image", () => {
    for (const capture of captures) {
      expect(capture.provenance.kind).toBe("admitted_worker_run");
      if (capture.provenance.kind !== "admitted_worker_run") return;
      expect(capture.provenance.providerKind).toBe("live_gce");
      expect(capture.provenance.isolationClass).toBe("gce_vm");
      expect(capture.provenance.resourceGeneration).toBeGreaterThanOrEqual(1);
      expect(capture.provenance.receiptRefs.length).toBeGreaterThan(0);
    }
    const guestImages = new Set(
      captures.flatMap((capture) =>
        capture.provenance.kind === "admitted_worker_run" ? [capture.provenance.guestImageDigest] : [],
      ),
    );
    expect(guestImages.size).toBe(1);
    // Each run is its own sandbox, so the three captures must not share one.
    const sandboxes = new Set(
      captures.flatMap((capture) =>
        capture.provenance.kind === "admitted_worker_run" ? [capture.provenance.sandboxRef] : [],
      ),
    );
    expect(sandboxes.size).toBe(3);
  });

  it("builds the pinned revisions and observes the fault build failing closed", () => {
    expect(captureFor("vulnerable").targetCommit).toBe(run.pinnedRevisions.vulnerable);
    expect(captureFor("fixed").targetCommit).toBe(run.pinnedRevisions.fixed);
    expect(captureFor("fault_build").targetCommit).toBe(run.pinnedRevisions.fixed);
    expect(reports[0]?.derivedBuildOutcome).toBe("succeeded");
    expect(reports[1]?.derivedBuildOutcome).toBe("succeeded");
    expect(reports[2]?.derivedBuildOutcome).toBe("failed");
  });

  it("satisfies every assertion on the build it was written for", () => {
    for (const report of reports) {
      expect(report.overallResult).toBe("satisfied");
      expect(report.provenanceKind).toBe("admitted_worker_run");
    }
  });

  it("proves the fixed build's symbol inventory is complete enough to show absence", () => {
    expect(reports[1]?.derivedSymbolInventoryComplete).toBe(true);
  });

  it("verifies the three-build suite", () => {
    expect(evaluateColdcardArtifactWitnessSuite(captures, reports)).toEqual({
      _tag: "Verified",
      reportRefs: reports.map((report) => report.captureRef),
    });
  });

  // The falsifiers below are what stop this file from being a rubber stamp. If
  // the evaluator ever stopped discriminating between the two builds, these
  // would pass with the wrong verdict rather than failing.
  it("violates the fixed build's absence claim when applied to the vulnerable build", () => {
    const report = evaluate("vulnerable", run.assertions.fixed ?? []);
    expect(report.overallResult).toBe("violated");
    const absence = report.results.find(
      (result) => result.assertionRef === "assertion.coldcard.fixed.fallback-provider-absent",
    );
    expect(absence?.status).toBe("violated");
    expect(absence?.reasonRef).toBe("reason.artifact_witness.forbidden_symbol_present");
  });

  it("violates the vulnerable build's provider claim when applied to the fixed build", () => {
    const report = evaluate("fixed", run.assertions.vulnerable ?? []);
    const provider = report.results.find(
      (result) => result.assertionRef === "assertion.coldcard.vulnerable.rng-get-provider",
    );
    expect(provider?.status).toBe("violated");
    expect(provider?.reasonRef).toBe("reason.artifact_witness.symbol_provider_mismatch");
  });

  it("refuses a matrix whose fault build did not fail", () => {
    const faultMutationRef = captureFor("fault_build").faultMutationRef ?? "mutation.absent";
    const substituted = captures.map((capture) =>
      capture.variant === "fault_build"
        ? { ...captureFor("fixed"), faultMutationRef, variant: "fault_build" as const }
        : capture,
    );
    expect(evaluateColdcardArtifactWitnessSuite(substituted, reports)).toEqual({
      _tag: "Refused",
      blockerRef: "blocker.artifact_witness.build_outcome_invalid",
    });
  });

  it("refuses the same builds once their provenance is downgraded", () => {
    const downgraded = captures.map((capture) => ({
      ...capture,
      provenance: {
        conformanceNoteRef: "note.artifact_witness.downgraded_for_this_test",
        kind: "conformance_vector" as const,
      },
    }));
    expect(evaluateColdcardArtifactWitnessSuite(downgraded, reports)).toEqual({
      _tag: "Refused",
      blockerRef: "blocker.artifact_witness.provenance_not_admitted",
    });
  });
});
