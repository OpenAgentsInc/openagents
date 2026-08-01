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
    expect(
      report.results.find((result) => result.assertionRef.endsWith("fallback-absent")),
    ).toMatchObject({ status: "satisfied" });

    const fixed = fixture.cases[1]!;
    const incomplete = evaluateArtifactWitness({
      assertions: fixed.assertions,
      capture: { ...fixed.capture, symbolInventoryComplete: false },
      evaluatedAt: "2026-08-01T19:00:00.000Z",
      witnessVersionRef: fixture.witnessVersionRef,
    });
    expect(
      incomplete.results.find((result) => result.assertionRef.endsWith("fallback-absent")),
    ).toMatchObject({ status: "not_proven" });
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

  it("requires the vulnerable, fixed, and failing fault build on one admitted profile", () => {
    const reports = fixture.cases.map((_, index) => evaluateCase(index));
    const captures = fixture.cases.map((testCase) => testCase.capture);
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
  });
});
