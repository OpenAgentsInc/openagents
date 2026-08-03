import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ArtifactWitnessAssertionSchema,
  ArtifactWitnessCaptureSchema,
  evaluateArtifactWitness,
  evaluateColdcardArtifactWitnessSuite,
} from "../src/artifact-witness.ts";
import { forensicSha256Digest, strictDecode } from "../src/canonical.ts";
import { Sha256Digest } from "../src/primitives.ts";

const FixtureSchema = S.Struct({
  schema: S.Literal("openagents.coldcard_artifact_witness_fixture.v1"),
  cases: S.Array(
    S.Struct({
      assertions: S.Array(ArtifactWitnessAssertionSchema),
      capture: ArtifactWitnessCaptureSchema,
    }),
  ),
  conformanceNoteRef: S.String,
  provenanceNote: S.String,
  toolchainDigest: Sha256Digest,
  witnessVersionRef: S.String,
  workerProfileDigest: Sha256Digest,
});

const fixture = strictDecode(
  FixtureSchema,
  JSON.parse(
    readFileSync(
      new URL(
        "../../../fixtures/forensics/coldcard/artifact-witness-fixtures.v1.json",
        import.meta.url,
      ),
      "utf8",
    ),
  ),
);

const evaluateCase = (index: number) => {
  const testCase = fixture.cases[index];
  if (testCase === undefined) throw new Error(`missing artifact witness fixture case ${index}`);
  return evaluateArtifactWitness({
    assertions: testCase.assertions,
    capture: testCase.capture,
    evaluatedAt: "2026-08-01T19:00:00.000Z",
    statisticalOutputTestRefs: ["diagnostic.nist-randomness-output"],
    witnessVersionRef: fixture.witnessVersionRef,
  });
};

describe("Coldcard artifact witness", () => {
  it("observes the vulnerable macro, linked fallback provider, secret sink, and width", () => {
    const report = evaluateCase(0);
    expect(report.overallResult).toBe("satisfied");
    expect(report.results).toHaveLength(4);
    expect(report.results.every((result) => result.status === "satisfied")).toBe(true);
    expect(
      report.results.find((result) => result.assertionRef.endsWith("fallback-provider")),
    ).toMatchObject({ reasonRef: "reason.artifact_witness.symbol_provider_observed" });
    expect(report.statisticalOutputTestsAdmissibleForProvenance).toBe(false);
  });

  it("observes the fixed provider and proves fallback absence only from a complete inventory", () => {
    const report = evaluateCase(1);
    expect(report.overallResult).toBe("satisfied");
    expect(report.derivedSymbolInventoryComplete).toBe(true);
    expect(
      report.results.find((result) => result.assertionRef.endsWith("fallback-absent")),
    ).toMatchObject({ status: "satisfied" });

    const fixed = fixture.cases[1]!;
    // Inventory completeness is derived from coverage of the collected linked
    // artifacts. Dropping one source artifact must un-prove absence, and there
    // is no caller-settable "complete" flag to override it.
    const incomplete = evaluateArtifactWitness({
      assertions: fixed.assertions,
      capture: {
        ...fixed.capture,
        symbolInventorySourceArtifactRefs: fixed.capture.symbolInventorySourceArtifactRefs.slice(1),
      },
      evaluatedAt: "2026-08-01T19:00:00.000Z",
      witnessVersionRef: fixture.witnessVersionRef,
    });
    expect(incomplete.derivedSymbolInventoryComplete).toBe(false);
    expect(
      incomplete.results.find((result) => result.assertionRef.endsWith("fallback-absent")),
    ).toMatchObject({ status: "not_proven" });

    // An empty enumeration is never a proof of absence either.
    const empty = evaluateArtifactWitness({
      assertions: fixed.assertions,
      capture: { ...fixed.capture, symbolInventory: [] },
      evaluatedAt: "2026-08-01T19:00:00.000Z",
      witnessVersionRef: fixture.witnessVersionRef,
    });
    expect(
      empty.results.find((result) => result.assertionRef.endsWith("fallback-absent")),
    ).toMatchObject({ status: "not_proven" });
  });

  it("derives the build outcome instead of accepting a caller verdict", () => {
    const vulnerable = fixture.cases[0]!;
    expect(evaluateCase(0).derivedBuildOutcome).toBe("succeeded");

    // Exit zero with no firmware image did not build the firmware.
    const noFirmware = evaluateArtifactWitness({
      assertions: vulnerable.assertions,
      capture: {
        ...vulnerable.capture,
        artifacts: vulnerable.capture.artifacts.filter((artifact) => artifact.kind !== "firmware"),
      },
      evaluatedAt: "2026-08-01T19:00:00.000Z",
      witnessVersionRef: fixture.witnessVersionRef,
    });
    expect(noFirmware.derivedBuildOutcome).toBe("not_observed");

    // A fault build whose termination was never observed cannot be reported as
    // failing closed.
    const fault = fixture.cases[2]!;
    const unobserved = evaluateArtifactWitness({
      assertions: fault.assertions,
      capture: {
        ...fault.capture,
        buildTermination: {
          status: "unobserved" as const,
          unavailableReasonRef: "reason.fixture.build-log-truncated",
        },
      },
      evaluatedAt: "2026-08-01T19:00:00.000Z",
      witnessVersionRef: fixture.witnessVersionRef,
    });
    expect(unobserved.derivedBuildOutcome).toBe("not_observed");
    expect(unobserved.results[0]).toMatchObject({
      reasonRef: "reason.artifact_witness.build_termination_not_observed",
      status: "not_proven",
    });
  });

  it("derives call-graph completeness from sources and unresolved indirect calls", () => {
    const vulnerable = fixture.cases[0]!;
    expect(evaluateCase(0).derivedCallGraphComplete).toBe(true);

    const unresolved = evaluateArtifactWitness({
      assertions: vulnerable.assertions,
      capture: { ...vulnerable.capture, callEdges: [], unresolvedIndirectCallSites: 1 },
      evaluatedAt: "2026-08-01T19:00:00.000Z",
      witnessVersionRef: fixture.witnessVersionRef,
    });
    expect(unresolved.derivedCallGraphComplete).toBe(false);
    expect(
      unresolved.results.find((result) => result.assertionRef.endsWith("secret-sink")),
    ).toMatchObject({
      reasonRef: "reason.artifact_witness.call_graph_incomplete",
      status: "not_proven",
    });
  });

  it("turns missing artifact inputs into not_proven and never safe", () => {
    const vulnerable = fixture.cases[0]!;
    const capture = {
      ...vulnerable.capture,
      artifacts: vulnerable.capture.artifacts.map((artifact) =>
        artifact.kind === "link_map"
          ? {
              artifactRef: artifact.artifactRef,
              kind: artifact.kind,
              status: "missing" as const,
              unavailableReasonRef: "reason.fixture.link-map-missing",
            }
          : artifact,
      ),
      callEdges: [],
      callGraphSourceArtifactRefs: [],
      symbolInventory: [],
      symbolInventorySourceArtifactRefs: [],
      symbolProviders: [],
    };
    const report = evaluateArtifactWitness({
      assertions: vulnerable.assertions,
      capture,
      evaluatedAt: "2026-08-01T19:00:00.000Z",
      witnessVersionRef: fixture.witnessVersionRef,
    });
    expect(report.overallResult).toBe("not_proven");
    expect(
      report.results.find((result) => result.assertionRef.endsWith("fallback-provider")),
    ).toMatchObject({
      missingArtifactKinds: ["link_map"],
      status: "not_proven",
    });
  });

  it("content-addresses each exact assertion result", () => {
    const report = evaluateCase(0);
    for (const result of report.results) {
      expect(result.receiptDigest).toBe(
        forensicSha256Digest({
          assertionDigest: result.assertionDigest,
          assertionRef: result.assertionRef,
          evidenceArtifactRefs: result.evidenceArtifactRefs,
          missingArtifactKinds: result.missingArtifactKinds,
          reasonRef: result.reasonRef,
          status: result.status,
        }),
      );
    }
  });

  it("refuses the checked-in conformance fixture as artifact-witness evidence", () => {
    // This is the honest state of OFR-014. The checked-in captures carry
    // placeholder digests and were never produced by a firmware build, so the
    // acceptance-level matrix gate must refuse them outright. Recording this as
    // "Verified" is exactly the laundering that reopened the issue.
    const reports = fixture.cases.map((_, index) => evaluateCase(index));
    const captures = fixture.cases.map((testCase) => testCase.capture);
    expect(reports.every((report) => report.provenanceKind === "conformance_vector")).toBe(true);
    expect(evaluateColdcardArtifactWitnessSuite(captures, reports)).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.artifact_witness.provenance_not_admitted",
    });
  });

  it("requires the vulnerable, fixed, and failing fault build on one admitted profile", () => {
    // Evaluator unit test only: the admitted provenance below is constructed
    // in-test to exercise the gate's success path. It is not a worker receipt
    // and proves nothing about any firmware build.
    const admit = (capture: (typeof fixture.cases)[number]["capture"], suffix: string) => ({
      ...capture,
      provenance: {
        guestImageDigest:
          "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        isolationClass: "gce_vm" as const,
        kind: "admitted_worker_run" as const,
        providerKind: "live_gce" as const,
        receiptRefs: [`receipt.test.artifact-witness.${suffix}`],
        resourceGeneration: 1,
        sandboxRef: `sandbox-ref://test/artifact-witness-${suffix}`,
      },
    });
    const captures = fixture.cases.map((testCase, index) =>
      admit(testCase.capture, `case-${index}`),
    );
    const reports = fixture.cases.map((testCase, index) =>
      evaluateArtifactWitness({
        assertions: testCase.assertions,
        capture: captures[index]!,
        evaluatedAt: "2026-08-01T19:00:00.000Z",
        witnessVersionRef: fixture.witnessVersionRef,
      }),
    );
    expect(evaluateColdcardArtifactWitnessSuite(captures, reports)).toMatchObject({
      _tag: "Verified",
    });
    expect(
      evaluateColdcardArtifactWitnessSuite(
        captures.map((capture, index) =>
          index === 2
            ? {
                ...capture,
                workerProfileDigest:
                  "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
              }
            : capture,
        ),
        reports,
      ),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.artifact_witness.worker_profile_drift",
    });
    expect(
      evaluateColdcardArtifactWitnessSuite(
        captures.map((capture, index) =>
          index === 2
            ? {
                ...capture,
                provenance: {
                  ...capture.provenance,
                  guestImageDigest:
                    "sha256:2222222222222222222222222222222222222222222222222222222222222222",
                },
              }
            : capture,
        ),
        reports,
      ),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.artifact_witness.worker_profile_drift",
    });
  });
});
