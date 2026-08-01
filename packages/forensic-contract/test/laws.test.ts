import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
import { describe, expect, it } from "vite-plus/test";

import {
  ForensicClaimRecordSchema,
  ForensicClaimRevisionSchema,
  evaluateClaimGate,
  evaluateClaimRevisionOrigin,
} from "../src/claims.ts";
import {
  canonicalizeForensicContract,
  forensicCanonicalJson,
  forensicSha256Digest,
  strictDecode,
} from "../src/canonical.ts";
import { evaluateForensicEventSequence, evaluateForensicRunTransition } from "../src/lifecycle.ts";
import { ForensicPromptPromotionSchema, ForensicScorecardSchema } from "../src/metrics.ts";
import { projectForensicRunPublicSafe } from "../src/projection.ts";
import {
  ColdcardReproductionManifestSchema,
  GeneratorTraceSchema,
  HistoricalChainSnapshotSchema,
  NodeScanReceiptSchema,
} from "../src/reproduction.ts";
import {
  ForensicCoverageManifestSchema,
  ForensicPromptArtifactSchema,
  ForensicRunEventSchema,
  ForensicRunSchema,
} from "../src/run.ts";

const FixtureFileSchema = S.Struct({
  schema: S.Literal("openagents.forensic_conformance_fixtures.v1"),
  contracts: S.Record(S.String, S.Unknown),
});

const fixture = strictDecode(
  FixtureFileSchema,
  JSON.parse(
    readFileSync(new URL("../../../fixtures/forensics/positive.v1.json", import.meta.url), "utf8"),
  ),
);

const positive = (schemaId: string): unknown => fixture.contracts[schemaId];

describe("forensic canonicalization", () => {
  it("sorts keys recursively and produces a portable SHA-256 known answer", () => {
    const left = { b: 2, a: 1 };
    const right = { a: 1, b: 2 };
    expect(forensicCanonicalJson(left)).toBe('{"a":1,"b":2}');
    expect(forensicCanonicalJson(left)).toBe(forensicCanonicalJson(right));
    expect(forensicSha256Digest(left)).toBe(
      "sha256:43258cff783fe7036d8a43033f830adfc60ec037382473548ac742b888292777",
    );
    expect(forensicSha256Digest(left)).toBe(forensicSha256Digest(right));
  });

  it("strictly decodes before producing canonical contract bytes", () => {
    const result = canonicalizeForensicContract(
      ForensicRunSchema,
      positive("openagents.forensic_run.v1"),
    );
    expect(result.value.state).toBe("completed");
    expect(result.digest).toMatch(/^sha256:[a-f0-9]{64}$/);
    expect(result.canonicalJson).not.toContain('providerUsageRef":undefined');
  });
});

describe("forensic lifecycle laws", () => {
  it("refuses incomplete inputs from becoming a complete result", () => {
    expect(
      evaluateForensicRunTransition("cleaned", "completed", {
        coverageStatus: "incomplete",
        cleanupState: "observed_zero_residue",
        cleanupReceiptRef: "receipt.cleanup.1",
      }),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.forensic.run.complete_evidence_missing",
    });
    expect(
      evaluateForensicRunTransition("cleaned", "completed_incomplete", {
        coverageStatus: "incomplete",
        cleanupState: "observed_zero_residue",
        cleanupReceiptRef: "receipt.cleanup.1",
      }),
    ).toMatchObject({ _tag: "Allowed" });
  });

  it("requires cleanup evidence and rejects graph-skipping transitions", () => {
    expect(
      evaluateForensicRunTransition("cleanup_requested", "cleaned", {
        coverageStatus: "complete",
        cleanupState: "requested",
      }),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.forensic.run.cleanup_not_observed",
    });
    expect(
      evaluateForensicRunTransition("running", "completed", {
        coverageStatus: "complete",
        cleanupState: "observed_zero_residue",
        cleanupReceiptRef: "receipt.cleanup.1",
      }),
    ).toMatchObject({
      _tag: "Refused",
      blockerRef: "blocker.forensic.run.invalid_transition",
    });
  });

  it("requires one dense append-only event sequence per run", () => {
    const first = strictDecode(
      ForensicRunEventSchema,
      positive("openagents.forensic_run_event.v1"),
    );
    const second = strictDecode(ForensicRunEventSchema, {
      ...first,
      eventRef: "event.run.1.worker.ready",
      sequence: 2,
      kind: "worker_ready",
    });
    expect(evaluateForensicEventSequence([first, second])).toMatchObject({
      _tag: "Valid",
      lastSequence: 2,
    });
    expect(evaluateForensicEventSequence([first, { ...second, sequence: 3 }])).toMatchObject({
      _tag: "Invalid",
      blockerRef: "blocker.forensic.events.non_dense_sequence",
    });
  });

  it("rejects a post-cleanup run state without observed cleanup", () => {
    const run = strictDecode(ForensicRunSchema, positive("openagents.forensic_run.v1"));
    expect(() =>
      strictDecode(ForensicRunSchema, { ...run, state: "review", cleanupState: "requested" }),
    ).toThrow(/post-cleanup run states/);
  });
});

describe("forensic claim laws", () => {
  it("does not let source evidence satisfy unauthorized-movement evidence", () => {
    const sourceClaim = strictDecode(
      ForensicClaimRecordSchema,
      positive("openagents.forensic_claim_record.v1"),
    );
    const unverifiedUnauthorizedClaim = strictDecode(ForensicClaimRecordSchema, {
      ...sourceClaim,
      claimRef: "claim.coldcard.unauthorized.1",
      claimKind: "unauthorized_movement",
      adjudication: "unverified",
    });
    expect(evaluateClaimGate(unverifiedUnauthorizedClaim)).toMatchObject({
      _tag: "Refused",
      missingEvidenceKinds: ["victim_testimony", "transaction_reference"],
    });
    expect(() =>
      strictDecode(ForensicClaimRecordSchema, {
        ...unverifiedUnauthorizedClaim,
        adjudication: "qualified",
      }),
    ).toThrow(/own claim rung/);
  });

  it("binds append-only revisions to the exact prior claim digest", () => {
    const claim = strictDecode(
      ForensicClaimRecordSchema,
      positive("openagents.forensic_claim_record.v1"),
    );
    const revision = strictDecode(ForensicClaimRevisionSchema, {
      ...strictDecode(
        ForensicClaimRevisionSchema,
        positive("openagents.forensic_claim_revision.v1"),
      ),
      priorClaimDigest: forensicSha256Digest(claim),
    });
    expect(evaluateClaimRevisionOrigin(claim, revision)).toMatchObject({ _tag: "Valid" });
    expect(
      evaluateClaimRevisionOrigin(claim, {
        ...revision,
        priorClaimDigest: "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
      }),
    ).toMatchObject({
      _tag: "Invalid",
      blockerRef: "blocker.forensic.claim_revision.prior_digest_mismatch",
    });
  });
});

describe("forensic fail-closed schemas", () => {
  it("rejects prompt content or lineage that drifts from the canonical digest", () => {
    const artifact = strictDecode(
      ForensicPromptArtifactSchema,
      positive("openagents.forensic_prompt_artifact.v1"),
    );
    expect(() =>
      strictDecode(ForensicPromptArtifactSchema, {
        ...artifact,
        promptIr: { ...artifact.promptIr, role: "Tampered after digest creation." },
      }),
    ).toThrow(/canonical digest/);
  });

  it("rejects complete coverage with a missing required dependency", () => {
    const coverage = strictDecode(
      ForensicCoverageManifestSchema,
      positive("openagents.forensic_coverage_manifest.v1"),
    );
    expect(() =>
      strictDecode(ForensicCoverageManifestSchema, {
        ...coverage,
        entries: coverage.entries.map((entry, index) =>
          index === 1 ? Object.assign({}, entry, { presence: "absent" }) : entry,
        ),
      }),
    ).toThrow(/cannot omit required inputs/);
  });

  it("rejects non-dense chain snapshots and false-green successful scans", () => {
    const snapshot = strictDecode(
      HistoricalChainSnapshotSchema,
      positive("openagents.historical_chain_snapshot.v1"),
    );
    expect(() =>
      strictDecode(HistoricalChainSnapshotSchema, {
        ...snapshot,
        blocks: snapshot.blocks.slice(0, 1),
      }),
    ).toThrow(/densely cover/);

    const receipt = strictDecode(
      NodeScanReceiptSchema,
      positive("openagents.node_scan_receipt.v1"),
    );
    expect(() =>
      strictDecode(NodeScanReceiptSchema, {
        ...receipt,
        negativeControls: "not_run",
      }),
    ).toThrow(/positive and negative controls/);
    expect(() =>
      strictDecode(NodeScanReceiptSchema, {
        ...receipt,
        candidateFunnel: { ...receipt.candidateFunnel, exactMatches: 11 },
      }),
    ).toThrow(/monotonically non-increasing/);
    expect(() =>
      strictDecode(NodeScanReceiptSchema, {
        ...receipt,
        completedRanges: [{ ...receipt.completedRanges[0], startHeight: 2, endHeight: 1 }],
      }),
    ).toThrow(/cannot precede/);
  });

  it("requires complete Coldcard revision roles and dense generator calls", () => {
    const manifest = strictDecode(
      ColdcardReproductionManifestSchema,
      positive("openagents.coldcard_reproduction_manifest.v1"),
    );
    const firstRevision = manifest.pinnedRevisions[0];
    if (firstRevision === undefined) throw new Error("Coldcard fixture needs pinned revisions");
    expect(() =>
      strictDecode(ColdcardReproductionManifestSchema, {
        ...manifest,
        pinnedRevisions: manifest.pinnedRevisions.map((revision, index) =>
          index === manifest.pinnedRevisions.length - 1
            ? Object.assign({}, revision, { role: firstRevision.role })
            : revision,
        ),
      }),
    ).toThrow(/required revision role/);

    const trace = strictDecode(GeneratorTraceSchema, positive("openagents.generator_trace.v1"));
    const firstCall = trace.calls[0];
    if (firstCall === undefined) throw new Error("generator fixture needs at least one call");
    expect(() =>
      strictDecode(GeneratorTraceSchema, {
        ...trace,
        calls: [Object.assign({}, firstCall, { sequence: 2 })],
      }),
    ).toThrow(/dense sequence/);
  });

  it("rejects optimizer self-promotion and scorecard count drift", () => {
    const promotion = strictDecode(
      ForensicPromptPromotionSchema,
      positive("openagents.forensic_prompt_promotion.v1"),
    );
    expect(() =>
      strictDecode(ForensicPromptPromotionSchema, {
        ...promotion,
        evaluatorRef: promotion.candidateProducerRef,
      }),
    ).toThrow(/cannot evaluate or promote themselves/);

    const scorecard = strictDecode(
      ForensicScorecardSchema,
      positive("openagents.forensic_scorecard.v1"),
    );
    expect(() => strictDecode(ForensicScorecardSchema, { ...scorecard, missCount: 1 })).toThrow(
      /must match retained runs/,
    );
  });
});

describe("forensic public projection", () => {
  it("projects only digests, counts, lifecycle truth, and usage totals", () => {
    const run = strictDecode(ForensicRunSchema, positive("openagents.forensic_run.v1"));
    const projection = projectForensicRunPublicSafe({
      ...run,
      errorRefs: ["secret.mnemonic.must.not.project"],
    });
    const encoded = forensicCanonicalJson(projection);
    expect(encoded).not.toContain("secret.mnemonic");
    expect(encoded).not.toContain(run.runRef);
    expect(encoded).not.toContain(run.placementRef);
    expect(encoded).not.toContain("providerUsageRef");
    expect(projection.findingCount).toBe(1);
    expect(projection.cleanupState).toBe("observed_zero_residue");
  });
});
