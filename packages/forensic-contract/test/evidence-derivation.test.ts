import { readFileSync } from "node:fs";

import { Schema as S } from "effect";
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
  EvidenceReconciliationSchema,
  type ClaimHistoryEvent,
  type EvidenceDerivationInput,
} from "../src/evidence-derivation.ts";
import { Sha256Digest } from "../src/primitives.ts";

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

const BOUNDARY_SOURCE_DIGEST =
  "sha256:0000000000000000000000000000000000000000000000000000000000000000" as const;

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
      baselineStructureDigest: baseline.graphStructureDigest,
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
        structureDigest: variant.graphStructureDigest,
        structureMoved: variant.graphStructureDigest !== baseline.graphStructureDigest,
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

  // Rounding arithmetic only. The reconciliation that carries a claim about the
  // Coldcard incident runs against independently published figures below.
  it("reconciles bounded precision arithmetic without overwriting either value", () => {
    const reconciliation = reconcileEvidenceFigures({
      derived: { "metric.amount": "12.347", "metric.drift": "13", "metric.missing": undefined },
      published: [
        {
          metricRef: "metric.amount",
          sourceDigest: BOUNDARY_SOURCE_DIGEST,
          sourceRef: "source.boundary.table-1",
          value: "12.35",
          lowerBound: "12.345",
          upperBound: "12.354999",
        },
        {
          metricRef: "metric.drift",
          sourceDigest: BOUNDARY_SOURCE_DIGEST,
          sourceRef: "source.boundary.table-2",
          value: "12",
          lowerBound: "11.5",
          upperBound: "12.499",
        },
        {
          metricRef: "metric.missing",
          sourceDigest: BOUNDARY_SOURCE_DIGEST,
          sourceRef: "source.boundary.table-3",
        },
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

/**
 * The Coldcard incident, derived rather than described.
 *
 * The seeds here are the two things a hand-maintained seed set is allowed to
 * be: addresses a third party published, and fingerprint candidates a frozen
 * OFR-016 scan produced from mainnet blocks. Everything else — which
 * transactions spent those addresses, where the money went, how much moved,
 * what groups with what, and when — is derived from chain records extracted
 * read-only from our own archival node.
 *
 * The figures it reconciles against are not written here. They come from the
 * postmortem author's own published dataset at a pinned commit and file digest,
 * so a disagreement between our derivation and theirs is meaningful in both
 * directions.
 */
const incidentInput = strictDecode(
  EvidenceDerivationInputSchema,
  JSON.parse(
    readFileSync(
      new URL(
        "../../../fixtures/forensics/coldcard/evidence-derivation-incident-wave6.v1.json",
        import.meta.url,
      ),
      "utf8",
    ),
  ),
);

const PublishedFigureSchema = S.Struct({
  lowerBound: S.optionalKey(S.String),
  metricRef: S.String,
  note: S.String,
  sourceDigest: Sha256Digest,
  sourceRef: S.String,
  upperBound: S.optionalKey(S.String),
  value: S.optionalKey(S.String),
});

const publishedFigures = strictDecode(
  S.Struct({
    schema: S.Literal("openagents.coldcard_published_figures.v1"),
    figures: S.Array(PublishedFigureSchema).check(S.isMinLength(6)),
    source: S.Struct({
      capturedFrom: S.String,
      commit: S.String.check(S.isPattern(/^[a-f0-9]{40}$/)),
      fileDigest: Sha256Digest,
      generatedAt: S.String,
      path: S.String,
      repository: S.String,
      vendored: S.Literal(false),
    }),
  }),
  JSON.parse(
    readFileSync(
      new URL(
        "../../../fixtures/forensics/coldcard/coldcard-published-figures.v1.json",
        import.meta.url,
      ),
      "utf8",
    ),
  ),
);

// The frozen OFR-016 outputs the fingerprint candidates were taken from.
const OFR_016_NARROWED_REVISION_DIGEST =
  "sha256:9447eef3f4e5de95d7eb4f2e98f36f4af4d4f93523a85d8458d2c126036744fe";
const OFR_016_BUNDLE_DIGESTS = new Set([
  "sha256:ea72db0f93927b3a93cf8ed6f791b499996ee0cb8a946cb00e643b761ff078ad",
  "sha256:83758b49a99005a44ad529ee0d5d87d1b8dadd643544b865c7a80e9df24400ee",
]);

describe("Coldcard incident wave 6, derived from frozen chain evidence", () => {
  const report = deriveEvidenceGraph(incidentInput);

  const destinationValueSats = incidentInput.chainTransactions
    .flatMap((transaction) => transaction.outputs)
    .reduce((total, output) => total + BigInt(output.valueSats), 0n);
  const inputAddresses = new Set(
    incidentInput.chainTransactions.flatMap((transaction) => transaction.inputAddressRefs),
  );
  const destinationAddresses = new Set(
    incidentInput.chainTransactions.flatMap((transaction) =>
      transaction.outputs.map((output) => output.addressRef),
    ),
  );

  it("takes its candidates from a frozen OFR-016 scan, not from a hand-written list", () => {
    expect(incidentInput.scanCandidates).toHaveLength(13);
    for (const candidate of incidentInput.scanCandidates) {
      expect(candidate.fingerprintRevisionDigest).toBe(OFR_016_NARROWED_REVISION_DIGEST);
      expect(OFR_016_BUNDLE_DIGESTS.has(candidate.sourceDigest)).toBe(true);
      expect(
        incidentInput.chainTransactions.some(
          (transaction) => transaction.transactionRef === candidate.transactionRef,
        ),
      ).toBe(true);
    }
    for (const address of incidentInput.publishedAddresses) {
      expect(address.sourceConfidence).toBe("published_unconfirmed");
      expect(address.sourceDigest).toBe(publishedFigures.source.fileDigest);
    }
  });

  it("derives a stable graph in which every edge and explanation binds one rule", () => {
    expect(deriveEvidenceGraph(incidentInput).graphDigest).toBe(report.graphDigest);
    expect(report.derivationRecords).toHaveLength(report.graph.edges.length);
    expect(report.graph.edges.length).toBeGreaterThan(50);
    for (const record of report.derivationRecords) {
      expect(record.ruleRef.startsWith("rule.evidence.")).toBe(true);
    }
    expect(report.transactionCount).toBe(13);
    expect(report.addressCount).toBe(inputAddresses.size + destinationAddresses.size);
    expect(report.graph.completeness).toBe("known_floor");
  });

  it("never turns thirteen addresses or thirteen transactions into thirteen victims", () => {
    expect(inputAddresses.size).toBe(13);
    expect(report.transactionCount).toBe(13);
    // No victim has reported to us. A third party publishing an address is not
    // a victim report, and the derivation refuses to promote it into one.
    expect(incidentInput.victimReports).toEqual([]);
    expect(report.victimReportCount).toBe(0);
    expect(report.victimCount).toEqual({
      exactness: "unavailable",
      reasonRef: "reason.victim-identity-deduplication-unavailable",
    });
    expect(report.unknownUnreachableCollectors.exactness).toBe("unavailable");
    expect(report.unknownUnpooledOperators.exactness).toBe("unavailable");
    expect(
      report.graph.edges.every((edge) => edge.confidence !== "victim_confirmed"),
    ).toBe(true);
    expect(
      report.graph.edges.some((edge) => edge.confidence === "pattern_candidate"),
    ).toBe(true);
  });

  it("refuses to pool single-input sweeps into one ownership component", () => {
    // Every wave-6 sweep spends exactly one input, so there is no shared-input
    // evidence at all. A component rule that grouped these anyway would be
    // inventing common ownership from co-occurrence in time.
    for (const transaction of incidentInput.chainTransactions) {
      expect(transaction.inputAddressRefs).toHaveLength(1);
    }
    expect(report.graph.componentRefs).toHaveLength(13 + 13);
    const componentNodes = report.graph.nodes.filter((node) => node.kind === "component");
    expect(componentNodes.filter((node) => node.sourceRefs.length > 1)).toHaveLength(13);
  });

  it("separates the two block times into distinct temporal episodes", () => {
    // 960,359 and 960,367 are about 104 minutes apart, past the configured
    // one-hour gap. A temporal episode is a timing claim, never an ownership one.
    expect(report.graph.episodeRefs).toHaveLength(2);
    expect(
      claimEvidenceIsIsolatedToRung("temporal_episode", ["evidence_graph", "temporal_proximity"]),
    ).toBe(true);
    expect(
      claimEvidenceIsIsolatedToRung("unauthorized_movement", [
        "evidence_graph",
        "temporal_proximity",
      ]),
    ).toBe(false);
  });

  it("reconciles against independently published figures, and drifts where it should", () => {
    const derived: Record<string, string | undefined> = {
      "metric.coldcard.wave-6.sweep-transaction-count": String(report.transactionCount),
      "metric.coldcard.wave-6.swept-value-sats": destinationValueSats.toString(),
      "metric.coldcard.wave-6.published-address-count": String(inputAddresses.size),
      // Our typed chain record names addresses, not outpoints, so a spent-UTXO
      // count is not something this graph can derive. Saying so is the point.
      "metric.coldcard.wave-6.spent-utxo-count": undefined,
      // Our frozen window is nine blocks of a 341-block incident, so this can
      // only ever be a floor.
      "metric.coldcard.incident.total-swept-sats": destinationValueSats.toString(),
      "metric.coldcard.incident.victim-count": undefined,
    };
    const reconciliation = reconcileEvidenceFigures({
      derived,
      published: publishedFigures.figures.map((figure) => ({
        metricRef: figure.metricRef,
        sourceDigest: figure.sourceDigest,
        sourceRef: figure.sourceRef,
        ...(figure.value === undefined ? {} : { value: figure.value }),
        ...(figure.lowerBound === undefined ? {} : { lowerBound: figure.lowerBound }),
        ...(figure.upperBound === undefined ? {} : { upperBound: figure.upperBound }),
      })),
      reconciliationRef: "reconciliation.coldcard.incident.wave-6",
    });
    const status = (metricRef: string) =>
      reconciliation.items.find((item) => item.metricRef === metricRef)?.status;

    expect(status("metric.coldcard.wave-6.sweep-transaction-count")).toBe("MATCH");
    expect(status("metric.coldcard.wave-6.swept-value-sats")).toBe("MATCH");
    expect(status("metric.coldcard.wave-6.published-address-count")).toBe("MATCH");
    expect(status("metric.coldcard.wave-6.spent-utxo-count")).toBe("UNAVAILABLE");
    expect(status("metric.coldcard.incident.total-swept-sats")).toBe("DRIFT");
    expect(status("metric.coldcard.incident.victim-count")).toBe("UNAVAILABLE");

    // The published figure is 32,629,041 satoshis for the wave and
    // 115,964,784,686 for the incident. Our derivation lands on the first from
    // chain bytes and is three orders of magnitude below the second, because it
    // only ever looked at nine blocks.
    expect(destinationValueSats).toBe(32_629_041n);
    const drifted = reconciliation.items.find(
      (item) => item.metricRef === "metric.coldcard.incident.total-swept-sats",
    );
    expect(drifted?.derivedValue).toBe("32629041");
    expect(drifted?.publishedValue).toBe("115964784686");
    for (const item of reconciliation.items) {
      expect(item.publishedSourceDigest).toBe(publishedFigures.source.fileDigest);
    }
  });

  it("refuses a reconciliation that claims a verdict without both values", () => {
    expect(() =>
      strictDecode(EvidenceReconciliationSchema, {
        schema: "openagents.evidence_reconciliation.v1",
        reconciliationRef: "reconciliation.coldcard.bad",
        items: [
          {
            metricRef: "metric.coldcard.wave-6.swept-value-sats",
            publishedSourceDigest: publishedFigures.source.fileDigest,
            publishedSourceRef: "source.coldcard-postmortem.chain-json.wave-6",
            status: "MATCH",
          },
        ],
      }),
    ).toThrow();
  });

  it("moves the graph when a boundary rule moves, and records it", () => {
    const variants = [
      { ref: "variant.depth-1", overrides: { maxTraversalDepth: 1 } },
      { ref: "variant.dust-5000000", overrides: { dustThresholdSats: "5000000" } },
      { ref: "variant.episode-gap-1d", overrides: { episodeGapSeconds: 86_400 } },
      { ref: "variant.shared-inputs-1", overrides: { minimumSharedInputs: 1 } },
      {
        ref: "variant.include-change",
        overrides: { changeRule: "include_classified_change" as const },
      },
    ].map((variant) => {
      const configured = withConfiguration(incidentInput, variant.overrides);
      const derived = deriveEvidenceGraph(configured);
      return {
        configurationDigest: configured.configuration.configurationDigest,
        graphDigest: derived.graphDigest,
        structureDigest: derived.graphStructureDigest,
        structureMoved: derived.graphStructureDigest !== report.graphStructureDigest,
        variantRef: variant.ref,
      };
    });
    const receipt = strictDecode(EvidenceGraphSensitivityReceiptSchema, {
      baselineGraphDigest: report.graphDigest,
      baselineStructureDigest: report.graphStructureDigest,
      boundaryRefs: [
        "boundary.traversal-depth",
        "boundary.dust-threshold",
        "boundary.episode-gap",
        "boundary.minimum-shared-inputs",
        "boundary.change-rule",
      ],
      receiptRef: "receipt.sensitivity.coldcard.incident.wave-6",
      variants,
    });
    expect(receipt.variants).toHaveLength(5);
    expect(new Set(variants.map((variant) => variant.configurationDigest)).size).toBe(5);

    const moved = (ref: string) =>
      variants.find((variant) => variant.variantRef === ref)!.structureMoved;
    // Every variant changes the configuration digest, so every variant changes
    // the graph digest. Only three of them change what the graph says.
    expect(variants.every((variant) => variant.graphDigest !== report.graphDigest)).toBe(true);
    // Raising the dust threshold above every swept amount removes every payment
    // edge, and pooling on a single shared input regroups the components.
    expect(moved("variant.dust-5000000")).toBe(true);
    expect(moved("variant.shared-inputs-1")).toBe(true);
    // A one-day episode window merges the two block times into one episode.
    expect(moved("variant.episode-gap-1d")).toBe(true);
    // These sweeps have no classified change output and are never traversed
    // past their first generation, so those two rules genuinely do not bind
    // here. A receipt that pretended they did would be the laundering.
    expect(moved("variant.include-change")).toBe(false);
    expect(moved("variant.depth-1")).toBe(false);
    expect(() =>
      strictDecode(EvidenceGraphSensitivityReceiptSchema, {
        ...receipt,
        variants: receipt.variants.map((variant) => ({ ...variant, structureMoved: true })),
      }),
    ).toThrow();
  });
});

describe("Coldcard claim history over the frozen OFR-016 output", () => {
  it("creates, promotes, and corrects one claim without rewriting its provenance", () => {
    const created = appendClaimHistoryEvent([], {
      action: "created",
      affectedProjectionRefs: [],
      appendedEvidence: [],
      claimKind: "program_fingerprint",
      claimRef: "claim.coldcard.wave-6.program-fingerprint",
      eventRef: "event.coldcard.wave-6.1",
      occurredAt: "2026-08-01T18:06:00.000Z",
      reasonRef: "reason.ofr-016.frozen-bundle-scan-produced-candidates",
      sequence: 1,
    });
    const promoted = appendClaimHistoryEvent(created, {
      action: "promoted",
      affectedProjectionRefs: [],
      appendedEvidence: [
        {
          kind: "historical_chain_snapshot",
          evidenceRef: "evidence.ofr-016.bundle.960365-960367",
          receiptDigest:
            "sha256:83758b49a99005a44ad529ee0d5d87d1b8dadd643544b865c7a80e9df24400ee",
        },
        {
          kind: "base_rate",
          evidenceRef: "evidence.ofr-016.wide-scan-ledger.2026-08-01",
          receiptDigest:
            "sha256:a3ee450edbe653d9059afaa6e466882a05efd13d341cc41913f3feef37849b0e",
        },
      ],
      claimKind: "program_fingerprint",
      claimRef: "claim.coldcard.wave-6.program-fingerprint",
      eventRef: "event.coldcard.wave-6.2",
      occurredAt: "2026-08-03T00:00:00.000Z",
      priorEventDigest: created[0]?.eventDigest,
      reasonRef: "reason.ofr-016.known-positives-reproduced-and-base-rate-measured",
      sequence: 2,
    });
    // The published postmortem states the size estimate exceeds the real signed
    // size every time. On the frozen bundles it does not: the estimate equals
    // the real vsize on all eight known positives. That is a correction to a
    // supporting property, recorded rather than quietly dropped.
    const corrected = appendClaimHistoryEvent(promoted, {
      action: "corrected",
      affectedProjectionRefs: [
        "projection.docs.coldcard.bitcoin-node-forensic-capability",
        "projection.docs.loupe.coldcard-historical-fingerprint-scan",
      ],
      appendedEvidence: [],
      claimKind: "program_fingerprint",
      claimRef: "claim.coldcard.wave-6.program-fingerprint",
      eventRef: "event.coldcard.wave-6.3",
      occurredAt: "2026-08-03T00:00:00.000Z",
      priorEventDigest: promoted[1]?.eventDigest,
      reasonRef: "reason.published-overshoot-property-did-not-reproduce",
      sequence: 3,
    });

    expect(corrected).toHaveLength(3);
    expect(corrected[0]?.appendedEvidence).toEqual([]);
    expect(corrected[1]?.appendedEvidence.map((item) => item.kind)).toEqual([
      "historical_chain_snapshot",
      "base_rate",
    ]);
    expect(corrected[2]?.priorEventDigest).toBe(promoted[1]?.eventDigest);
    // The claim never leaves its rung. Reproducing a program fingerprint is not
    // a finding that a movement was unauthorized, however many times it is
    // promoted.
    expect(corrected.every((event) => event.claimKind === "program_fingerprint")).toBe(true);
    expect(() =>
      appendClaimHistoryEvent(corrected, {
        action: "promoted",
        affectedProjectionRefs: [],
        appendedEvidence: [
          {
            kind: "victim_testimony",
            evidenceRef: "evidence.absent",
            receiptDigest:
              "sha256:0000000000000000000000000000000000000000000000000000000000000000",
          },
        ],
        claimKind: "unauthorized_movement",
        claimRef: "claim.coldcard.wave-6.program-fingerprint",
        eventRef: "event.coldcard.wave-6.4",
        occurredAt: "2026-08-03T00:00:00.000Z",
        priorEventDigest: corrected[2]?.eventDigest,
        reasonRef: "reason.invalid-rung-switch",
        sequence: 4,
      }),
    ).toThrow();
  });
});
