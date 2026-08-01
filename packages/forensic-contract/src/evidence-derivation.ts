import { Schema as S } from "effect";

import { forensicSha256Digest, strictDecode } from "./canonical.ts";
import {
  FORENSIC_EVIDENCE_GRAPH_VERSION,
  ForensicEvidenceGraphSchema,
  REQUIRED_EVIDENCE_BY_CLAIM_KIND,
  ClaimEvidenceSchema,
  type EvidenceGraphEdge,
  type EvidenceGraphNode,
  type ForensicEvidenceGraph,
} from "./claims.ts";
import {
  ClaimKind,
  EvidenceKind,
  ForensicRef,
  ForensicTimestamp,
  NonNegativeInteger,
  PositiveInteger,
  Sha256Digest,
  ShortText,
} from "./primitives.ts";

export const EVIDENCE_DERIVATION_INPUT_VERSION = "openagents.evidence_derivation_input.v1" as const;
export const EVIDENCE_DERIVATION_REPORT_VERSION =
  "openagents.evidence_derivation_report.v1" as const;
export const EVIDENCE_RECONCILIATION_VERSION = "openagents.evidence_reconciliation.v1" as const;
export const CLAIM_HISTORY_EVENT_VERSION = "openagents.claim_history_event.v1" as const;

export const VictimReportSeedSchema = S.Struct({
  addressRef: ForensicRef,
  evidenceDigest: Sha256Digest,
  reportRef: ForensicRef,
  victimRef: S.optionalKey(ForensicRef),
  transactionRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(256)),
});
export type VictimReportSeed = typeof VictimReportSeedSchema.Type;

export const PublishedAddressSeedSchema = S.Struct({
  addressRef: ForensicRef,
  publishedClaimRef: ForensicRef,
  sourceConfidence: S.Literals(["published_unconfirmed", "victim_confirmed"]),
  sourceDigest: Sha256Digest,
});
export type PublishedAddressSeed = typeof PublishedAddressSeedSchema.Type;

export const ScanCandidateSeedSchema = S.Struct({
  candidateRef: ForensicRef,
  fingerprintRevisionDigest: Sha256Digest,
  sourceDigest: Sha256Digest,
  transactionRef: ForensicRef,
});
export type ScanCandidateSeed = typeof ScanCandidateSeedSchema.Type;

export const EvidenceChainOutputSchema = S.Struct({
  addressRef: ForensicRef,
  classification: S.Literals(["destination", "change", "unknown"]),
  outputRef: ForensicRef,
  valueSats: S.String.check(S.isPattern(/^(?:0|[1-9][0-9]*)$/)),
});
export type EvidenceChainOutput = typeof EvidenceChainOutputSchema.Type;

export const EvidenceChainTransactionSchema = S.Struct({
  blockHeight: NonNegativeInteger,
  inputAddressRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(10_000)),
  observedAt: ForensicTimestamp,
  outputs: S.Array(EvidenceChainOutputSchema).check(S.isMinLength(1), S.isMaxLength(10_000)),
  rawTransactionDigest: Sha256Digest,
  transactionRef: ForensicRef,
});
export type EvidenceChainTransaction = typeof EvidenceChainTransactionSchema.Type;

export const EvidenceDerivationConfigurationSchema = S.Struct({
  changeRule: S.Literals(["exclude_explicit_change", "include_classified_change"]),
  componentRule: S.Literal("shared_input_and_transaction_flow"),
  configurationDigest: Sha256Digest,
  dustThresholdSats: S.String.check(S.isPattern(/^(?:0|[1-9][0-9]*)$/)),
  episodeGapSeconds: PositiveInteger,
  maxTraversalDepth: PositiveInteger,
  minimumSharedInputs: PositiveInteger,
  revisionRef: ForensicRef,
});
export type EvidenceDerivationConfiguration = typeof EvidenceDerivationConfigurationSchema.Type;

export const EvidenceDerivationInputSchema = S.Struct({
  schema: S.Literal(EVIDENCE_DERIVATION_INPUT_VERSION),
  chainTransactions: S.Array(EvidenceChainTransactionSchema).check(S.isMaxLength(1_000_000)),
  configuration: EvidenceDerivationConfigurationSchema,
  generatedAt: ForensicTimestamp,
  inputRef: ForensicRef,
  publishedAddresses: S.Array(PublishedAddressSeedSchema).check(S.isMaxLength(100_000)),
  scanCandidates: S.Array(ScanCandidateSeedSchema).check(S.isMaxLength(1_000_000)),
  victimReports: S.Array(VictimReportSeedSchema).check(S.isMaxLength(100_000)),
})
  .pipe(
    S.check(
      S.makeFilter(
        (input) =>
          input.configuration.configurationDigest ===
          forensicSha256Digest({
            changeRule: input.configuration.changeRule,
            componentRule: input.configuration.componentRule,
            dustThresholdSats: input.configuration.dustThresholdSats,
            episodeGapSeconds: input.configuration.episodeGapSeconds,
            maxTraversalDepth: input.configuration.maxTraversalDepth,
            minimumSharedInputs: input.configuration.minimumSharedInputs,
            revisionRef: input.configuration.revisionRef,
          }),
        { message: "evidence derivation configuration digest must bind every rule and threshold" },
      ),
      S.makeFilter(
        (input) => {
          const seedRefs = [
            ...input.victimReports.map((seed) => seed.reportRef),
            ...input.publishedAddresses.map((seed) => seed.publishedClaimRef),
            ...input.scanCandidates.map((seed) => seed.candidateRef),
          ];
          return new Set(seedRefs).size === seedRefs.length;
        },
        { message: "hand-maintained evidence seed refs must be unique" },
      ),
    ),
  )
  .annotate({ identifier: "EvidenceDerivationInput" });
export type EvidenceDerivationInput = typeof EvidenceDerivationInputSchema.Type;

export const EdgeDerivationRecordSchema = S.Struct({
  edgeDigest: Sha256Digest,
  edgeRef: ForensicRef,
  explanationDigest: Sha256Digest,
  inputDigest: Sha256Digest,
  ruleRef: ForensicRef,
});
export type EdgeDerivationRecord = typeof EdgeDerivationRecordSchema.Type;

export const QuantityDerivationRecordSchema = S.Struct({
  metricRef: ForensicRef,
  ruleRef: ForensicRef,
  sourceRefs: S.Array(ForensicRef).check(S.isMinLength(1), S.isMaxLength(100_000)),
  valueDigest: Sha256Digest,
});
export type QuantityDerivationRecord = typeof QuantityDerivationRecordSchema.Type;

const UnmeasurableQuantitySchema = S.Struct({
  exactness: S.Literal("unavailable"),
  reasonRef: ForensicRef,
});

export const EvidenceDerivationReportSchema = S.Struct({
  schema: S.Literal(EVIDENCE_DERIVATION_REPORT_VERSION),
  addressCount: NonNegativeInteger,
  derivationRecords: S.Array(EdgeDerivationRecordSchema).check(S.isMaxLength(1_000_000)),
  graph: ForensicEvidenceGraphSchema,
  graphDigest: Sha256Digest,
  inputDigest: Sha256Digest,
  quantityRecords: S.Array(QuantityDerivationRecordSchema).check(
    S.isMinLength(7),
    S.isMaxLength(7),
  ),
  victimCount: UnmeasurableQuantitySchema,
  victimReportCount: NonNegativeInteger,
  transactionCount: NonNegativeInteger,
  unknownUnpooledOperators: UnmeasurableQuantitySchema,
  unknownUnreachableCollectors: UnmeasurableQuantitySchema,
  utxoCount: NonNegativeInteger,
})
  .pipe(
    S.check(
      S.makeFilter((report) => report.graphDigest === forensicSha256Digest(report.graph), {
        message: "evidence graph digest must bind the exact derived graph",
      }),
      S.makeFilter(
        (report) =>
          report.derivationRecords.length === report.graph.edges.length &&
          report.derivationRecords.every((record, index) => {
            const edge = report.graph.edges[index];
            return (
              edge !== undefined &&
              record.edgeRef === edge.edgeRef &&
              record.edgeDigest === forensicSha256Digest(edge) &&
              record.explanationDigest === forensicSha256Digest(edge.generatedExplanation) &&
              record.ruleRef === edge.derivationRuleRef
            );
          }),
        { message: "every graph edge and explanation must bind one derivation record" },
      ),
      S.makeFilter(
        (report) => {
          const values: Readonly<Record<string, unknown>> = {
            "metric.evidence.address-count": report.addressCount,
            "metric.evidence.transaction-count": report.transactionCount,
            "metric.evidence.utxo-count": report.utxoCount,
            "metric.evidence.victim-report-count": report.victimReportCount,
            "metric.evidence.victim-count": report.victimCount,
            "metric.evidence.unknown-unpooled-operators": report.unknownUnpooledOperators,
            "metric.evidence.unknown-unreachable-collectors": report.unknownUnreachableCollectors,
          };
          return (
            new Set(report.quantityRecords.map((record) => record.metricRef)).size === 7 &&
            report.quantityRecords.every(
              (record) =>
                Object.hasOwn(values, record.metricRef) &&
                record.valueDigest === forensicSha256Digest(values[record.metricRef]),
            )
          );
        },
        { message: "every displayed quantity must bind its value, sources, and derivation rule" },
      ),
      S.makeFilter(
        (report) => {
          const nodeRefs = new Set(report.graph.nodes.map((node) => node.nodeRef));
          return (
            nodeRefs.size === report.graph.nodes.length &&
            new Set(report.graph.edges.map((edge) => edge.edgeRef)).size ===
              report.graph.edges.length &&
            report.graph.edges.every(
              (edge) => nodeRefs.has(edge.fromNodeRef) && nodeRefs.has(edge.toNodeRef),
            )
          );
        },
        { message: "graph node and edge refs must be unique and every edge endpoint must exist" },
      ),
    ),
  )
  .annotate({ identifier: "EvidenceDerivationReport" });
export type EvidenceDerivationReport = typeof EvidenceDerivationReportSchema.Type;

type EdgeConfidence = EvidenceGraphEdge["confidence"];

const node = (
  nodeRef: string,
  kind: EvidenceGraphNode["kind"],
  sourceRefs: ReadonlyArray<string>,
  attributes: unknown,
): EvidenceGraphNode => ({
  nodeRef,
  kind,
  sourceRefs,
  attributesDigest: forensicSha256Digest(attributes),
});

const explainEdge = (
  ruleRef: string,
  fromNodeRef: string,
  toNodeRef: string,
  detail: string,
): string => {
  switch (ruleRef) {
    case "rule.evidence.victim-report.v1":
      return `Victim report ${fromNodeRef} names ${toNodeRef}; this is a confirmed report edge, not an inferred victim count.`;
    case "rule.evidence.published-address.v1":
      return `Published source ${fromNodeRef} names ${toNodeRef} at its retained source confidence.`;
    case "rule.evidence.scan-candidate.v1":
      return `Fingerprint candidate ${fromNodeRef} supports transaction ${toNodeRef} as program similarity only.`;
    case "rule.evidence.transaction-input.v1":
      return `${fromNodeRef} funds ${toNodeRef} in the immutable transaction record.`;
    case "rule.evidence.transaction-output.v1":
      return `${fromNodeRef} pays ${toNodeRef}; ${detail}.`;
    case "rule.evidence.component.v1":
      return `${fromNodeRef} belongs to ownership component ${toNodeRef} under shared-input and transaction-flow rules.`;
    case "rule.evidence.episode.v1":
      return `${fromNodeRef} occurs in temporal episode ${toNodeRef} under the configured time-gap rule.`;
    default:
      throw new Error(`unsupported evidence derivation rule ${ruleRef}`);
  }
};

const derivedEdge = (input: {
  readonly confidence: EdgeConfidence;
  readonly detail: string;
  readonly evidenceRefs: ReadonlyArray<string>;
  readonly fromNodeRef: string;
  readonly kind: EvidenceGraphEdge["kind"];
  readonly ruleRef: string;
  readonly sourceClaimRef: string;
  readonly toNodeRef: string;
}): Readonly<{ edge: EvidenceGraphEdge; record: EdgeDerivationRecord }> => {
  const explanation = explainEdge(input.ruleRef, input.fromNodeRef, input.toNodeRef, input.detail);
  const inputDigest = forensicSha256Digest(input);
  const edge: EvidenceGraphEdge = {
    edgeRef: `edge.${forensicSha256Digest({ ...input, explanation }).slice(7, 31)}`,
    fromNodeRef: input.fromNodeRef,
    toNodeRef: input.toNodeRef,
    kind: input.kind,
    derivationRuleRef: input.ruleRef,
    sourceClaimRef: input.sourceClaimRef,
    generatedExplanation: ShortText.make(explanation),
    confidence: input.confidence,
    evidenceRefs: input.evidenceRefs,
  };
  return {
    edge,
    record: {
      edgeDigest: forensicSha256Digest(edge),
      edgeRef: edge.edgeRef,
      explanationDigest: forensicSha256Digest(edge.generatedExplanation),
      inputDigest,
      ruleRef: input.ruleRef,
    },
  };
};

class Components {
  readonly parents = new Map<string, string>();

  add(reference: string): void {
    if (!this.parents.has(reference)) this.parents.set(reference, reference);
  }

  find(reference: string): string {
    const parent = this.parents.get(reference);
    if (parent === undefined) throw new Error(`component member ${reference} is missing`);
    if (parent === reference) return reference;
    const root = this.find(parent);
    this.parents.set(reference, root);
    return root;
  }

  connect(left: string, right: string): void {
    this.add(left);
    this.add(right);
    const leftRoot = this.find(left);
    const rightRoot = this.find(right);
    if (leftRoot !== rightRoot) {
      const roots = [leftRoot, rightRoot].toSorted();
      const first = roots[0];
      const second = roots[1];
      if (first !== undefined && second !== undefined) this.parents.set(second, first);
    }
  }
}

const reachableTransactions = (
  input: EvidenceDerivationInput,
): Readonly<{ limitHit: boolean; transactions: ReadonlyArray<EvidenceChainTransaction> }> => {
  const byRef = new Map(
    input.chainTransactions.map((transaction) => [transaction.transactionRef, transaction]),
  );
  const queue: Array<{ depth: number; transactionRef: string }> = [
    ...input.victimReports.flatMap((seed) => seed.transactionRefs),
    ...input.scanCandidates.map((seed) => seed.transactionRef),
  ].map((transactionRef) => ({ depth: 0, transactionRef }));
  const seededAddresses = new Set(input.publishedAddresses.map((seed) => seed.addressRef));
  for (const transaction of input.chainTransactions) {
    if (
      transaction.inputAddressRefs.some((reference) => seededAddresses.has(reference)) ||
      transaction.outputs.some((output) => seededAddresses.has(output.addressRef))
    )
      queue.push({ depth: 0, transactionRef: transaction.transactionRef });
  }
  const visited = new Set<string>();
  let limitHit = false;
  while (queue.length > 0) {
    const current = queue.shift();
    if (current === undefined) break;
    if (visited.has(current.transactionRef)) continue;
    const transaction = byRef.get(current.transactionRef);
    if (transaction === undefined) continue;
    visited.add(current.transactionRef);
    const outputAddresses = new Set(transaction.outputs.map((output) => output.addressRef));
    const next = input.chainTransactions.filter((candidate) =>
      candidate.inputAddressRefs.some((reference) => outputAddresses.has(reference)),
    );
    if (next.length > 0 && current.depth >= input.configuration.maxTraversalDepth) {
      limitHit = true;
      continue;
    }
    for (const candidate of next) {
      queue.push({ depth: current.depth + 1, transactionRef: candidate.transactionRef });
    }
  }
  return {
    limitHit,
    transactions: input.chainTransactions.filter((transaction) =>
      visited.has(transaction.transactionRef),
    ),
  };
};

export const deriveEvidenceGraph = (
  rawInput: EvidenceDerivationInput,
): EvidenceDerivationReport => {
  const input = strictDecode(EvidenceDerivationInputSchema, rawInput);
  const inputDigest = forensicSha256Digest(input);
  const reachable = reachableTransactions(input);
  const nodes = new Map<string, EvidenceGraphNode>();
  const edges: Array<EvidenceGraphEdge> = [];
  const derivationRecords: Array<EdgeDerivationRecord> = [];
  const addNode = (candidate: EvidenceGraphNode) => {
    const existing = nodes.get(candidate.nodeRef);
    if (existing === undefined) {
      nodes.set(candidate.nodeRef, candidate);
      return;
    }
    const sourceRefs = [...new Set([...existing.sourceRefs, ...candidate.sourceRefs])].toSorted();
    nodes.set(candidate.nodeRef, {
      ...existing,
      sourceRefs,
      attributesDigest: forensicSha256Digest({
        attributeDigests: [existing.attributesDigest, candidate.attributesDigest].toSorted(),
        sourceRefs,
      }),
    });
  };
  const addEdge = (derived: ReturnType<typeof derivedEdge>) => {
    edges.push(derived.edge);
    derivationRecords.push(derived.record);
  };

  for (const seed of input.victimReports) {
    addNode(node(seed.reportRef, "source_claim", [seed.reportRef], seed));
    addNode(node(seed.addressRef, "address", [seed.reportRef], seed.addressRef));
    addEdge(
      derivedEdge({
        confidence: "victim_confirmed",
        detail: "victim confirmation retained",
        evidenceRefs: [seed.reportRef],
        fromNodeRef: seed.reportRef,
        kind: "reports",
        ruleRef: "rule.evidence.victim-report.v1",
        sourceClaimRef: seed.reportRef,
        toNodeRef: seed.addressRef,
      }),
    );
  }
  for (const seed of input.publishedAddresses) {
    addNode(node(seed.publishedClaimRef, "source_claim", [seed.publishedClaimRef], seed));
    addNode(node(seed.addressRef, "address", [seed.publishedClaimRef], seed.addressRef));
    addEdge(
      derivedEdge({
        confidence: seed.sourceConfidence,
        detail: "published confidence retained",
        evidenceRefs: [seed.publishedClaimRef],
        fromNodeRef: seed.publishedClaimRef,
        kind: "reports",
        ruleRef: "rule.evidence.published-address.v1",
        sourceClaimRef: seed.publishedClaimRef,
        toNodeRef: seed.addressRef,
      }),
    );
  }
  for (const seed of input.scanCandidates) {
    addNode(node(seed.candidateRef, "claim", [seed.candidateRef], seed));
    addNode(node(seed.transactionRef, "transaction", [seed.candidateRef], seed.transactionRef));
    addEdge(
      derivedEdge({
        confidence: "pattern_candidate",
        detail: "fingerprint evidence is not identity evidence",
        evidenceRefs: [seed.candidateRef],
        fromNodeRef: seed.candidateRef,
        kind: "supports",
        ruleRef: "rule.evidence.scan-candidate.v1",
        sourceClaimRef: seed.candidateRef,
        toNodeRef: seed.transactionRef,
      }),
    );
  }

  const components = new Components();
  const dustThreshold = BigInt(input.configuration.dustThresholdSats);
  for (const transaction of reachable.transactions) {
    addNode(
      node(transaction.transactionRef, "transaction", [transaction.transactionRef], transaction),
    );
    components.add(transaction.transactionRef);
    for (const addressRef of transaction.inputAddressRefs) {
      addNode(node(addressRef, "address", [transaction.transactionRef], addressRef));
      if (transaction.inputAddressRefs.length >= input.configuration.minimumSharedInputs) {
        components.connect(addressRef, transaction.transactionRef);
      } else {
        components.add(addressRef);
      }
      addEdge(
        derivedEdge({
          confidence: "observed",
          detail: "input recorded on chain",
          evidenceRefs: [transaction.transactionRef],
          fromNodeRef: addressRef,
          kind: "funds",
          ruleRef: "rule.evidence.transaction-input.v1",
          sourceClaimRef: `claim.chain.${transaction.transactionRef}`,
          toNodeRef: transaction.transactionRef,
        }),
      );
    }
    for (const output of transaction.outputs) {
      if (
        (output.classification === "change" &&
          input.configuration.changeRule === "exclude_explicit_change") ||
        BigInt(output.valueSats) <= dustThreshold
      )
        continue;
      addNode(node(output.addressRef, "address", [transaction.transactionRef], output));
      components.connect(transaction.transactionRef, output.addressRef);
      addEdge(
        derivedEdge({
          confidence: "observed",
          detail: `${output.classification} output above ${input.configuration.dustThresholdSats} satoshis`,
          evidenceRefs: [transaction.transactionRef, output.outputRef],
          fromNodeRef: transaction.transactionRef,
          kind: "pays_to",
          ruleRef: "rule.evidence.transaction-output.v1",
          sourceClaimRef: `claim.chain.${transaction.transactionRef}`,
          toNodeRef: output.addressRef,
        }),
      );
    }
  }

  const grouped = new Map<string, Array<string>>();
  for (const reference of components.parents.keys()) {
    const root = components.find(reference);
    const members = grouped.get(root) ?? [];
    members.push(reference);
    grouped.set(root, members);
  }
  const componentRefs: Array<string> = [];
  for (const members of [...grouped.values()]
    .map((values) => values.toSorted())
    .toSorted((left, right) => left[0]!.localeCompare(right[0]!))) {
    const componentRef = `component.${forensicSha256Digest(members).slice(7, 23)}`;
    componentRefs.push(componentRef);
    addNode(node(componentRef, "component", members, members));
    for (const member of members) {
      addEdge(
        derivedEdge({
          confidence: "observed",
          detail: input.configuration.componentRule,
          evidenceRefs: [member],
          fromNodeRef: member,
          kind: "member_of",
          ruleRef: "rule.evidence.component.v1",
          sourceClaimRef: `claim.component.${componentRef}`,
          toNodeRef: componentRef,
        }),
      );
    }
  }

  const episodeRefs: Array<string> = [];
  const orderedTransactions = [...reachable.transactions].sort((left, right) =>
    left.observedAt.localeCompare(right.observedAt),
  );
  let currentEpisode: Array<EvidenceChainTransaction> = [];
  const flushEpisode = () => {
    if (currentEpisode.length === 0) return;
    const transactionRefs = currentEpisode.map((transaction) => transaction.transactionRef);
    const episodeRef = `episode.${forensicSha256Digest(transactionRefs).slice(7, 23)}`;
    episodeRefs.push(episodeRef);
    addNode(node(episodeRef, "episode", transactionRefs, transactionRefs));
    for (const transaction of currentEpisode) {
      addEdge(
        derivedEdge({
          confidence: "observed",
          detail: `${input.configuration.episodeGapSeconds}s gap`,
          evidenceRefs: [transaction.transactionRef],
          fromNodeRef: transaction.transactionRef,
          kind: "occurs_in",
          ruleRef: "rule.evidence.episode.v1",
          sourceClaimRef: `claim.episode.${episodeRef}`,
          toNodeRef: episodeRef,
        }),
      );
    }
    currentEpisode = [];
  };
  for (const transaction of orderedTransactions) {
    const previous = currentEpisode.at(-1);
    if (
      previous !== undefined &&
      (Date.parse(transaction.observedAt) - Date.parse(previous.observedAt)) / 1_000 >
        input.configuration.episodeGapSeconds
    )
      flushEpisode();
    currentEpisode.push(transaction);
  }
  flushEpisode();

  const graph: ForensicEvidenceGraph = strictDecode(ForensicEvidenceGraphSchema, {
    schema: FORENSIC_EVIDENCE_GRAPH_VERSION,
    graphRef: `graph.${input.inputRef}`,
    runRef: input.inputRef,
    rawInputDigest: inputDigest,
    derivationRevisionDigest: input.configuration.configurationDigest,
    traversalMaxDepth: input.configuration.maxTraversalDepth,
    traversalLimitHit: reachable.limitHit,
    nodes: [...nodes.values()].sort((left, right) => left.nodeRef.localeCompare(right.nodeRef)),
    edges,
    componentRefs,
    episodeRefs,
    completeness: "known_floor",
    limitationRefs: [
      "limitation.unknown-unreachable-collectors-unmeasurable",
      "limitation.unpooled-operators-unmeasurable",
      "limitation.program-fingerprint-not-identity",
    ],
    generatedAt: input.generatedAt,
  });
  const addressCount = graph.nodes.filter((entry) => entry.kind === "address").length;
  const transactionCount = graph.nodes.filter((entry) => entry.kind === "transaction").length;
  const utxoCount = reachable.transactions.reduce(
    (count, transaction) => count + transaction.outputs.length,
    0,
  );
  const victimReportCount = new Set(input.victimReports.map((seed) => seed.reportRef)).size;
  const victimCount = {
    exactness: "unavailable" as const,
    reasonRef: "reason.victim-identity-deduplication-unavailable",
  };
  const unknownUnpooledOperators = {
    exactness: "unavailable" as const,
    reasonRef: "reason.unpooled-operators-unmeasurable",
  };
  const unknownUnreachableCollectors = {
    exactness: "unavailable" as const,
    reasonRef: "reason.unreachable-collectors-unmeasurable",
  };
  const quantityRecord = (
    metricRef: string,
    ruleRef: string,
    sourceRefs: ReadonlyArray<string>,
    value: unknown,
  ): QuantityDerivationRecord => ({
    metricRef,
    ruleRef,
    sourceRefs,
    valueDigest: forensicSha256Digest(value),
  });
  const chainSourceRefs =
    reachable.transactions.length === 0
      ? [input.inputRef]
      : reachable.transactions.map((transaction) => transaction.transactionRef);
  const reportSourceRefs =
    input.victimReports.length === 0
      ? [input.inputRef]
      : input.victimReports.map((report) => report.reportRef);
  return strictDecode(EvidenceDerivationReportSchema, {
    schema: EVIDENCE_DERIVATION_REPORT_VERSION,
    addressCount,
    derivationRecords,
    graph,
    graphDigest: forensicSha256Digest(graph),
    inputDigest,
    quantityRecords: [
      quantityRecord(
        "metric.evidence.address-count",
        "rule.evidence.distinct-address-count.v1",
        [input.inputRef],
        addressCount,
      ),
      quantityRecord(
        "metric.evidence.transaction-count",
        "rule.evidence.distinct-transaction-count.v1",
        chainSourceRefs,
        transactionCount,
      ),
      quantityRecord(
        "metric.evidence.utxo-count",
        "rule.evidence.chain-output-count.v1",
        chainSourceRefs,
        utxoCount,
      ),
      quantityRecord(
        "metric.evidence.victim-report-count",
        "rule.evidence.distinct-report-count.v1",
        reportSourceRefs,
        victimReportCount,
      ),
      quantityRecord(
        "metric.evidence.victim-count",
        "rule.evidence.no-report-to-person-inference.v1",
        [input.inputRef],
        victimCount,
      ),
      quantityRecord(
        "metric.evidence.unknown-unpooled-operators",
        "rule.evidence.unpooled-operators-unmeasurable.v1",
        [input.configuration.revisionRef],
        unknownUnpooledOperators,
      ),
      quantityRecord(
        "metric.evidence.unknown-unreachable-collectors",
        "rule.evidence.traversal-bounded-unknown.v1",
        [input.configuration.revisionRef],
        unknownUnreachableCollectors,
      ),
    ],
    transactionCount,
    unknownUnpooledOperators,
    unknownUnreachableCollectors,
    utxoCount,
    victimCount,
    victimReportCount,
  });
};

export const EvidenceReconciliationItemSchema = S.Struct({
  derivedValue: S.optionalKey(S.String),
  metricRef: ForensicRef,
  publishedLowerBound: S.optionalKey(S.String),
  publishedUpperBound: S.optionalKey(S.String),
  publishedSourceRef: ForensicRef,
  publishedValue: S.optionalKey(S.String),
  status: S.Literals(["MATCH", "DRIFT", "UNAVAILABLE"]),
});
export type EvidenceReconciliationItem = typeof EvidenceReconciliationItemSchema.Type;

export const EvidenceReconciliationSchema = S.Struct({
  schema: S.Literal(EVIDENCE_RECONCILIATION_VERSION),
  items: S.Array(EvidenceReconciliationItemSchema).check(S.isMinLength(1), S.isMaxLength(10_000)),
  reconciliationRef: ForensicRef,
});
export type EvidenceReconciliation = typeof EvidenceReconciliationSchema.Type;

const decimalPattern = /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/;

const decimalParts = (value: string): Readonly<{ digits: bigint; scale: number }> | undefined => {
  if (!decimalPattern.test(value)) return undefined;
  const negative = value.startsWith("-");
  const unsigned = negative ? value.slice(1) : value;
  const [integer, fraction = ""] = unsigned.split(".");
  if (integer === undefined) return undefined;
  const digits = BigInt(`${integer}${fraction}`);
  return { digits: negative ? -digits : digits, scale: fraction.length };
};

const compareDecimals = (left: string, right: string): number | undefined => {
  const leftParts = decimalParts(left);
  const rightParts = decimalParts(right);
  if (leftParts === undefined || rightParts === undefined) return undefined;
  const scale = Math.max(leftParts.scale, rightParts.scale);
  const normalizedLeft = leftParts.digits * 10n ** BigInt(scale - leftParts.scale);
  const normalizedRight = rightParts.digits * 10n ** BigInt(scale - rightParts.scale);
  return normalizedLeft < normalizedRight ? -1 : normalizedLeft > normalizedRight ? 1 : 0;
};

export const reconcileEvidenceFigures = (input: {
  readonly derived: Readonly<Record<string, string | undefined>>;
  readonly published: ReadonlyArray<{
    readonly metricRef: string;
    readonly lowerBound?: string;
    readonly upperBound?: string;
    readonly sourceRef: string;
    readonly value?: string;
  }>;
  readonly reconciliationRef: string;
}): EvidenceReconciliation =>
  strictDecode(EvidenceReconciliationSchema, {
    schema: EVIDENCE_RECONCILIATION_VERSION,
    reconciliationRef: input.reconciliationRef,
    items: input.published.map((published) => {
      const derivedValue = input.derived[published.metricRef];
      const hasBounds = published.lowerBound !== undefined && published.upperBound !== undefined;
      const lowerComparison =
        derivedValue === undefined || published.lowerBound === undefined
          ? undefined
          : compareDecimals(derivedValue, published.lowerBound);
      const upperComparison =
        derivedValue === undefined || published.upperBound === undefined
          ? undefined
          : compareDecimals(derivedValue, published.upperBound);
      const status =
        derivedValue === undefined ||
        published.value === undefined ||
        !hasBounds ||
        lowerComparison === undefined ||
        upperComparison === undefined
          ? "UNAVAILABLE"
          : lowerComparison >= 0 && upperComparison <= 0
            ? "MATCH"
            : "DRIFT";
      return {
        metricRef: published.metricRef,
        publishedSourceRef: published.sourceRef,
        status,
        ...(derivedValue === undefined ? {} : { derivedValue }),
        ...(published.value === undefined ? {} : { publishedValue: published.value }),
        ...(published.lowerBound === undefined
          ? {}
          : { publishedLowerBound: published.lowerBound }),
        ...(published.upperBound === undefined
          ? {}
          : { publishedUpperBound: published.upperBound }),
      };
    }),
  });

export const ClaimHistoryEventSchema = S.Struct({
  schema: S.Literal(CLAIM_HISTORY_EVENT_VERSION),
  action: S.Literals(["created", "promoted", "corrected", "superseded"]),
  affectedProjectionRefs: S.Array(ForensicRef).check(S.isMaxLength(10_000)),
  appendedEvidence: S.Array(ClaimEvidenceSchema).check(S.isMaxLength(64)),
  claimKind: ClaimKind,
  claimRef: ForensicRef,
  eventDigest: Sha256Digest,
  eventRef: ForensicRef,
  occurredAt: ForensicTimestamp,
  priorEventDigest: S.optionalKey(Sha256Digest),
  reasonRef: ForensicRef,
  sequence: PositiveInteger,
})
  .pipe(
    S.check(
      S.makeFilter(
        (event) => {
          const { eventDigest: _eventDigest, ...unsigned } = event;
          return event.eventDigest === forensicSha256Digest(unsigned);
        },
        { message: "claim history event digest must bind the entire append-only event" },
      ),
    ),
  )
  .annotate({ identifier: "ClaimHistoryEvent" });
export type ClaimHistoryEvent = typeof ClaimHistoryEventSchema.Type;

export const appendClaimHistoryEvent = (
  history: ReadonlyArray<ClaimHistoryEvent>,
  event: Omit<ClaimHistoryEvent, "eventDigest" | "schema">,
): ReadonlyArray<ClaimHistoryEvent> => {
  const previous = history.at(-1);
  if (event.sequence !== history.length + 1)
    throw new Error("claim history sequence must append densely");
  if (event.claimRef !== (previous?.claimRef ?? event.claimRef))
    throw new Error("claim history cannot switch claims");
  if (event.claimKind !== (previous?.claimKind ?? event.claimKind))
    throw new Error("claim history cannot switch claim rungs");
  if (
    previous === undefined
      ? event.priorEventDigest !== undefined
      : event.priorEventDigest !== previous.eventDigest
  ) {
    throw new Error("claim history event must bind the exact prior event digest");
  }
  if (previous === undefined && event.action !== "created") {
    throw new Error("the first claim history event must create the claim");
  }
  if (previous !== undefined && event.action === "created") {
    throw new Error("a claim can only be created once");
  }
  if (event.action === "promoted" && event.appendedEvidence.length === 0) {
    throw new Error("claim promotion must append provenance-bearing evidence");
  }
  if (event.action === "corrected" && event.affectedProjectionRefs.length === 0) {
    throw new Error("claim correction must identify affected projections");
  }
  const value = { ...event, schema: CLAIM_HISTORY_EVENT_VERSION };
  return [
    ...history,
    strictDecode(ClaimHistoryEventSchema, { ...value, eventDigest: forensicSha256Digest(value) }),
  ];
};

export const claimEvidenceIsIsolatedToRung = (
  claimKind: ClaimKind | "temporal_episode",
  evidenceKinds: ReadonlyArray<EvidenceKind>,
): boolean => {
  const requiredByRung: Readonly<
    Record<ClaimKind | "temporal_episode", ReadonlyArray<EvidenceKind>>
  > = {
    ...REQUIRED_EVIDENCE_BY_CLAIM_KIND,
    temporal_episode: ["evidence_graph", "temporal_proximity"],
  };
  const observed = new Set(evidenceKinds);
  const targetIsSatisfied = requiredByRung[claimKind].every((kind) => observed.has(kind));
  return (
    targetIsSatisfied &&
    Object.entries(requiredByRung).every(
      ([otherClaimKind, requiredKinds]) =>
        otherClaimKind === claimKind || !requiredKinds.every((kind) => observed.has(kind)),
    )
  );
};

export const EvidenceGraphSensitivityReceiptSchema = S.Struct({
  baselineGraphDigest: Sha256Digest,
  boundaryRefs: S.Array(ForensicRef).check(S.isMinLength(5), S.isMaxLength(64)),
  receiptRef: ForensicRef,
  variants: S.Array(
    S.Struct({
      configurationDigest: Sha256Digest,
      graphDigest: Sha256Digest,
      variantRef: ForensicRef,
    }),
  ).check(S.isMinLength(1), S.isMaxLength(128)),
});
export type EvidenceGraphSensitivityReceipt = typeof EvidenceGraphSensitivityReceiptSchema.Type;
