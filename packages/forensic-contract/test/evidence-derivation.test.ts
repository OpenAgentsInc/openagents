import { readFileSync } from "node:fs";

import { describe, expect, it } from "vite-plus/test";

import { forensicSha256Digest, strictDecode } from "../src/canonical.ts";
import {
  ClaimHistoryEventSchema,
  EvidenceDerivationInputSchema,
  EvidenceDerivationReportSchema,
  EvidenceGraphSensitivityReceiptSchema,
  appendClaimHistoryEvent,
  claimEvidenceIsIsolatedToRung,
  deriveEvidenceGraph,
  reconcileEvidenceFigures,
  type ClaimHistoryEvent,
  type EvidenceDerivationInput,
} from "../src/evidence-derivation.ts";

const fixture = strictDecode(
  EvidenceDerivationInputSchema,
  JSON.parse(
    readFileSync(
      new URL(
        "../../../fixtures/forensics/coldcard/evidence-derivation-fixture.v1.json",
        import.meta.url,
      ),
      "utf8",
    ),
  ),
);

const withConfiguration = (
  input: EvidenceDerivationInput,
  overrides: Partial<EvidenceDerivationInput["configuration"]>,
): EvidenceDerivationInput => {
  const unsigned = { ...input.configuration, ...overrides };
  const { configurationDigest: _configurationDigest, ...digestInput } = unsigned;
  return strictDecode(EvidenceDerivationInputSchema, {
    ...input,
    configuration: {
      ...unsigned,
      configurationDigest: forensicSha256Digest(digestInput),
    },
  });
};

describe("Coldcard evidence derivation", () => {
  it("binds every edge and generated explanation to one immutable input and rule", () => {
    const report = deriveEvidenceGraph(fixture);
    expect(report.derivationRecords).toHaveLength(report.graph.edges.length);
    expect(report.graph.edges.every((edge) => edge.derivationRuleRef.startsWith("rule."))).toBe(
      true,
    );
    expect(deriveEvidenceGraph(fixture).graphDigest).toBe(report.graphDigest);

    const edge = report.graph.edges[0];
    if (edge === undefined) throw new Error("fixture must derive at least one edge");
    const tampered = {
      ...report,
      graph: {
        ...report.graph,
        edges: report.graph.edges.map((candidate, index) =>
          index === 0
            ? {
                ...candidate,
                generatedExplanation: "A hand-edited explanation detached from its rule.",
              }
            : candidate,
        ),
      },
    };
    expect(() => strictDecode(EvidenceDerivationReportSchema, tampered)).toThrow();
  });

  it("retains merged provenance and keeps pattern candidates weaker than victim reports", () => {
    const report = deriveEvidenceGraph(fixture);
    const address = report.graph.nodes.find((node) => node.nodeRef === "address.fixture.victim-a");
    expect(address?.sourceRefs).toEqual([
      "claim.published.fixture.victim-a",
      "report.fixture.victim-a",
      "transaction.fixture.001",
    ]);
    expect(
      report.graph.edges.find((edge) => edge.fromNodeRef === "candidate.fixture.program-match")
        ?.confidence,
    ).toBe("pattern_candidate");
    expect(
      report.graph.edges.find((edge) => edge.fromNodeRef === "report.fixture.victim-a")?.confidence,
    ).toBe("victim_confirmed");
  });

  it("never converts address, transaction, UTXO, or report counts into victim counts", () => {
    const report = deriveEvidenceGraph(fixture);
    expect(report).toMatchObject({
      victimCount: { exactness: "unavailable" },
      victimReportCount: 1,
      unknownUnpooledOperators: { exactness: "unavailable" },
      unknownUnreachableCollectors: { exactness: "unavailable" },
    });
    expect(report.addressCount).toBeGreaterThan(report.victimReportCount);
    expect(report.transactionCount).toBeGreaterThan(report.victimReportCount);
    expect(report.utxoCount).toBeGreaterThan(report.victimReportCount);
    expect(report.graph.completeness).toBe("known_floor");
    expect(report.quantityRecords).toHaveLength(7);
    expect(report.quantityRecords.every((record) => record.ruleRef.startsWith("rule."))).toBe(true);
  });

  it("keeps ownership components and temporal episodes as separate derivations", () => {
    const report = deriveEvidenceGraph(fixture);
    expect(report.graph.componentRefs.length).toBeGreaterThan(0);
    expect(report.graph.episodeRefs).toHaveLength(2);
    expect(report.graph.edges.some((edge) => edge.kind === "member_of")).toBe(true);
    expect(report.graph.edges.some((edge) => edge.kind === "occurs_in")).toBe(true);

    const inclusiveBoundary = deriveEvidenceGraph(
      withConfiguration(fixture, {
        episodeGapSeconds: 7201,
      }),
    );
    expect(inclusiveBoundary.graph.episodeRefs).toHaveLength(1);
  });

  it("records materially different graphs at depth, change, dust, gap, and pooling boundaries", () => {
    const baseline = deriveEvidenceGraph(fixture);
    const variants = [
      withConfiguration(fixture, { maxTraversalDepth: 1 }),
      withConfiguration(fixture, { changeRule: "include_classified_change" }),
      withConfiguration(fixture, { dustThresholdSats: "545" }),
      withConfiguration(fixture, { episodeGapSeconds: 7201 }),
      withConfiguration(fixture, { minimumSharedInputs: 3 }),
    ].map(deriveEvidenceGraph);
    expect(new Set(variants.map((variant) => variant.graphDigest)).size).toBe(5);
    expect(variants.some((variant) => variant.graphDigest === baseline.graphDigest)).toBe(false);
    expect(baseline.graph.nodes.some((node) => node.nodeRef === "address.fixture.change")).toBe(
      false,
    );
    expect(variants[1]?.graph.nodes.some((node) => node.nodeRef === "address.fixture.change")).toBe(
      true,
    );
    expect(variants[2]?.graph.nodes.some((node) => node.nodeRef === "address.fixture.dust")).toBe(
      true,
    );
    const receipt = strictDecode(EvidenceGraphSensitivityReceiptSchema, {
      baselineGraphDigest: baseline.graphDigest,
      boundaryRefs: [
        "boundary.traversal-depth",
        "boundary.change-rule",
        "boundary.dust-threshold",
        "boundary.episode-gap",
        "boundary.shared-input-threshold",
      ],
      receiptRef: "receipt.fixture.evidence-sensitivity",
      variants: variants.map((variant, index) => ({
        configurationDigest: variant.graph.derivationRevisionDigest,
        graphDigest: variant.graphDigest,
        variantRef: `variant.fixture.${index + 1}`,
      })),
    });
    expect(receipt.variants).toHaveLength(5);
  });

  it("isolates fingerprint, component, unauthorized-movement, and identity gates", () => {
    expect(
      claimEvidenceIsIsolatedToRung("program_fingerprint", [
        "historical_chain_snapshot",
        "positive_control",
        "negative_control",
        "base_rate",
      ]),
    ).toBe(true);
    expect(
      claimEvidenceIsIsolatedToRung("entity_cluster", ["evidence_graph", "pooling_edge"]),
    ).toBe(true);
    expect(
      claimEvidenceIsIsolatedToRung("temporal_episode", ["evidence_graph", "temporal_proximity"]),
    ).toBe(true);
    expect(
      claimEvidenceIsIsolatedToRung("unauthorized_movement", [
        "victim_testimony",
        "transaction_reference",
      ]),
    ).toBe(true);
    expect(
      claimEvidenceIsIsolatedToRung("identity_attribution", [
        "identity_evidence",
        "independent_review",
      ]),
    ).toBe(true);
    expect(
      claimEvidenceIsIsolatedToRung("identity_attribution", ["evidence_graph", "pooling_edge"]),
    ).toBe(false);
  });

  it("reconciles at published precision without overwriting either value", () => {
    const reconciliation = reconcileEvidenceFigures({
      derived: { "metric.amount": "12.347", "metric.drift": "13", "metric.missing": undefined },
      published: [
        {
          metricRef: "metric.amount",
          sourceRef: "source.paper.table-1",
          value: "12.35",
          lowerBound: "12.345",
          upperBound: "12.354999",
        },
        {
          metricRef: "metric.drift",
          sourceRef: "source.paper.table-2",
          value: "12",
          lowerBound: "11.5",
          upperBound: "12.499",
        },
        { metricRef: "metric.missing", sourceRef: "source.paper.table-3" },
      ],
      reconciliationRef: "reconciliation.fixture.coldcard",
    });
    expect(reconciliation.items.map((item) => item.status)).toEqual([
      "MATCH",
      "DRIFT",
      "UNAVAILABLE",
    ]);
    expect(reconciliation.items[0]).toMatchObject({
      derivedValue: "12.347",
      publishedValue: "12.35",
      publishedLowerBound: "12.345",
      publishedUpperBound: "12.354999",
    });
  });
});

describe("append-only claim history", () => {
  const evidence = {
    kind: "historical_chain_snapshot" as const,
    evidenceRef: "evidence.snapshot.fixture",
    receiptDigest:
      "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee" as const,
  };

  const event = (
    overrides: Partial<Omit<ClaimHistoryEvent, "eventDigest" | "schema">> = {},
  ): Omit<ClaimHistoryEvent, "eventDigest" | "schema"> => ({
    action: "created",
    affectedProjectionRefs: [],
    appendedEvidence: [],
    claimKind: "program_fingerprint",
    claimRef: "claim.fixture.program-fingerprint",
    eventRef: "event.fixture.1",
    occurredAt: "2026-08-01T22:00:00.000Z",
    reasonRef: "reason.fixture.created",
    sequence: 1,
    ...overrides,
  });

  it("preserves provenance through promotion and correction without rewriting history", () => {
    const created = appendClaimHistoryEvent([], event());
    const promoted = appendClaimHistoryEvent(
      created,
      event({
        action: "promoted",
        appendedEvidence: [evidence],
        eventRef: "event.fixture.2",
        priorEventDigest: created[0]?.eventDigest,
        reasonRef: "reason.fixture.positive-control-passed",
        sequence: 2,
      }),
    );
    const corrected = appendClaimHistoryEvent(
      promoted,
      event({
        action: "corrected",
        affectedProjectionRefs: ["projection.public.fixture"],
        eventRef: "event.fixture.3",
        priorEventDigest: promoted[1]?.eventDigest,
        reasonRef: "reason.fixture.figure-reconciled",
        sequence: 3,
      }),
    );
    expect(created).toHaveLength(1);
    expect(corrected).toHaveLength(3);
    expect(corrected[1]?.appendedEvidence).toEqual([evidence]);
    expect(() =>
      strictDecode(ClaimHistoryEventSchema, {
        ...corrected[1],
        reasonRef: "reason.fixture.tampered",
      }),
    ).toThrow();
  });

  it("rejects promotion without evidence and correction without affected projections", () => {
    const created = appendClaimHistoryEvent([], event());
    expect(() =>
      appendClaimHistoryEvent(
        created,
        event({
          action: "promoted",
          eventRef: "event.fixture.2",
          priorEventDigest: created[0]?.eventDigest,
          sequence: 2,
        }),
      ),
    ).toThrow();
    expect(() =>
      appendClaimHistoryEvent(
        created,
        event({
          action: "corrected",
          eventRef: "event.fixture.2",
          priorEventDigest: created[0]?.eventDigest,
          sequence: 2,
        }),
      ),
    ).toThrow();
  });
});
