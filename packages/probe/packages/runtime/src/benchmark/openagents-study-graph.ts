import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  OpenAgentsRepoStudyPacket,
  OpenAgentsRepoStudyPacketRationaleKind,
  openAgentsRepoStudyPacketHash,
} from "./openagents-study-packet";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

export const OPENAGENTS_REPO_STUDIED_KNOWLEDGE_GRAPH_SCHEMA_REF =
  "openagents.repo_studied_knowledge_graph.v0" as const;

export const OPENAGENTS_REPO_STUDIED_KNOWLEDGE_TRAVERSAL_SCHEMA_REF =
  "openagents.repo_studied_knowledge_traversal.v0" as const;

export const OpenAgentsRepoStudiedKnowledgeNodeKind = S.Literals([
  "code",
  "commit",
  "doc",
  "evidence_span",
  "invariant",
  "issue",
  "rationale",
]);
export type OpenAgentsRepoStudiedKnowledgeNodeKind =
  typeof OpenAgentsRepoStudiedKnowledgeNodeKind.Type;

export const OpenAgentsRepoStudiedKnowledgeNodeSourceKind = S.Literals([
  "commit_history",
  "corpus_entry",
  "evidence_span",
  "github_issue",
  "rationale_source",
]);
export type OpenAgentsRepoStudiedKnowledgeNodeSourceKind =
  typeof OpenAgentsRepoStudiedKnowledgeNodeSourceKind.Type;

export const OpenAgentsRepoStudiedKnowledgeEdgeKind = S.Literals([
  "code_explained_by_audit",
  "code_explained_by_roadmap",
  "code_warned_by_rejected_lineage",
  "doc_explains_rationale",
  "evidence_span_supports_node",
  "edit_site_commit_context",
  "issue_depends_on_packet",
  "issue_tracks_edit_site",
  "edit_site_respects_invariant",
  "section_indexes_entry",
]);
export type OpenAgentsRepoStudiedKnowledgeEdgeKind =
  typeof OpenAgentsRepoStudiedKnowledgeEdgeKind.Type;

export const OpenAgentsRepoStudiedKnowledgeNodeSource = S.Struct({
  commit: S.optional(S.String),
  corpusManifestHash: S.optional(S.String),
  corpusManifestRef: S.optional(S.String),
  issueNumber: S.optional(S.Number),
  issueRef: S.optional(S.String),
  kind: OpenAgentsRepoStudiedKnowledgeNodeSourceKind,
  packetHash: S.String,
  packetRef: S.String,
  path: S.optional(S.String),
  rationaleKind: S.optional(OpenAgentsRepoStudyPacketRationaleKind),
  rationaleRef: S.optional(S.String),
  sourceDigest: S.String,
  sourceHash: S.optional(S.String),
  spanHash: S.optional(S.String),
  spanId: S.optional(S.String),
  subjectDigest: S.optional(S.String),
});
export type OpenAgentsRepoStudiedKnowledgeNodeSource =
  typeof OpenAgentsRepoStudiedKnowledgeNodeSource.Type;

export const OpenAgentsRepoStudiedKnowledgeNode = S.Struct({
  kind: OpenAgentsRepoStudiedKnowledgeNodeKind,
  label: S.String,
  nodeHash: S.String,
  ref: S.String,
  source: OpenAgentsRepoStudiedKnowledgeNodeSource,
  sourceAuthorityRefs: S.Array(S.String),
});
export type OpenAgentsRepoStudiedKnowledgeNode =
  typeof OpenAgentsRepoStudiedKnowledgeNode.Type;

export const OpenAgentsRepoStudiedKnowledgeEdge = S.Struct({
  edgeHash: S.String,
  fromNodeRef: S.String,
  kind: OpenAgentsRepoStudiedKnowledgeEdgeKind,
  rationaleRef: S.String,
  ref: S.String,
  sourceEvidenceNodeRefs: S.Array(S.String),
  toNodeRef: S.String,
});
export type OpenAgentsRepoStudiedKnowledgeEdge =
  typeof OpenAgentsRepoStudiedKnowledgeEdge.Type;

export const OpenAgentsRepoStudiedKnowledgeGraph = S.Struct({
  commit: S.String,
  corpusManifestHash: S.String,
  corpusManifestRef: S.String,
  edges: S.Array(OpenAgentsRepoStudiedKnowledgeEdge),
  generatedAt: S.String,
  graphHash: S.String,
  graphRef: S.String,
  nodes: S.Array(OpenAgentsRepoStudiedKnowledgeNode),
  packetHash: S.String,
  packetRef: S.String,
  repo: S.String,
  schemaRef: S.Literal(OPENAGENTS_REPO_STUDIED_KNOWLEDGE_GRAPH_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
});
export type OpenAgentsRepoStudiedKnowledgeGraph =
  typeof OpenAgentsRepoStudiedKnowledgeGraph.Type;

export const OpenAgentsRepoStudiedKnowledgeTraversal = S.Struct({
  auditNodeRefs: S.Array(S.String),
  commitNodeRefs: S.Array(S.String),
  fromNodeRef: S.String,
  graphHash: S.String,
  invariantNodeRefs: S.Array(S.String),
  issueNodeRefs: S.Array(S.String),
  rejectedLineageNodeRefs: S.Array(S.String),
  schemaRef: S.Literal(OPENAGENTS_REPO_STUDIED_KNOWLEDGE_TRAVERSAL_SCHEMA_REF),
  traversedEdgeRefs: S.Array(S.String),
});
export type OpenAgentsRepoStudiedKnowledgeTraversal =
  typeof OpenAgentsRepoStudiedKnowledgeTraversal.Type;

export interface BuildOpenAgentsRepoStudiedKnowledgeGraphInput {
  readonly editSitePaths?: ReadonlyArray<string>;
  readonly generatedAt?: string;
  readonly graphRef?: string;
  readonly issueRefs?: ReadonlyArray<OpenAgentsRepoStudiedKnowledgeIssueRef>;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly trackingIssueNumber?: number;
}

export interface OpenAgentsRepoStudiedKnowledgeIssueRef {
  readonly issueNumber: number;
  readonly issueRef: string;
  readonly label: string;
}

export interface TraverseOpenAgentsRepoStudiedKnowledgeGraphInput {
  readonly fromNodeRef?: string;
  readonly path?: string;
}

const DEFAULT_STUDIED_KNOWLEDGE_ISSUE_REFS = [
  {
    issueNumber: 5313,
    issueRef: "github_issue.OpenAgentsInc.openagents.5313",
    label: "Tassadar roadmap epic",
  },
  {
    issueNumber: 5315,
    issueRef: "github_issue.OpenAgentsInc.openagents.5315",
    label: "S2 studied knowledge graph issue",
  },
] as const;

const DEFAULT_EDIT_SITE_PATHS = [
  "packages/probe/packages/runtime/src/benchmark/openagents-autopilot-coder-studied-context.ts",
  "packages/probe/packages/runtime/src/benchmark/openagents-studybench-eval-harness.ts",
  "packages/probe/packages/runtime/src/benchmark/openagents-study-packet.ts",
  "packages/probe/packages/runtime/src/benchmark/repo-corpus-manifest.ts",
] as const;

export function buildOpenAgentsRepoStudiedKnowledgeGraph(
  input: BuildOpenAgentsRepoStudiedKnowledgeGraphInput,
): Effect.Effect<
  OpenAgentsRepoStudiedKnowledgeGraph,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const packet = input.packet;
    const packetHash = openAgentsRepoStudyPacketHash(packet);

    if (packet.packetHash !== packetHash) {
      return yield* studyGraphError("studyGraph.packetHash", "input packet hash must match packet content");
    }

    const nodeBuilder = new StudyGraphNodeBuilder(packet);

    for (const section of packet.sections) {
      for (const path of section.corpusEntryPaths) {
        nodeBuilder.addCorpusEntry(path, section.sourceAuthorityRefs);
      }
    }

    for (const span of packet.evidenceSpans) {
      nodeBuilder.addEvidenceSpan(span);
    }

    for (const commit of packet.commitHistory) {
      nodeBuilder.addCommit(commit);
    }

    for (const source of packet.rationaleSources) {
      nodeBuilder.addRationaleSource(source);
    }

    for (const issue of input.issueRefs ?? DEFAULT_STUDIED_KNOWLEDGE_ISSUE_REFS) {
      nodeBuilder.addIssue(issue);
    }

    const nodes = nodeBuilder.nodes();
    const edges = buildStudiedKnowledgeEdges({
      editSitePaths: input.editSitePaths ?? DEFAULT_EDIT_SITE_PATHS,
      nodeBuilder,
      packet,
      trackingIssueNumber: input.trackingIssueNumber ?? 5315,
    });

    const baseGraph: OpenAgentsRepoStudiedKnowledgeGraph = {
      commit: packet.commit,
      corpusManifestHash: packet.corpusManifestHash,
      corpusManifestRef: packet.corpusManifestRef,
      edges,
      generatedAt: input.generatedAt ?? "generated_at.withheld_for_stable_study_graph_hash",
      graphHash: "sha256:pending",
      graphRef: "openagents_repo_studied_knowledge_graph.pending",
      nodes,
      packetHash: packet.packetHash,
      packetRef: packet.packetRef,
      repo: packet.repo,
      schemaRef: OPENAGENTS_REPO_STUDIED_KNOWLEDGE_GRAPH_SCHEMA_REF,
      sourceBoundary: "public_refs_only",
    };
    const graphHash = openAgentsRepoStudiedKnowledgeGraphHash(baseGraph);
    const graph: OpenAgentsRepoStudiedKnowledgeGraph = {
      ...baseGraph,
      graphHash,
      graphRef: input.graphRef ?? `openagents_repo_studied_knowledge_graph.${shortHash(graphHash)}`,
    };

    return yield* decodeOpenAgentsRepoStudiedKnowledgeGraph(graph);
  });
}

export function decodeOpenAgentsRepoStudiedKnowledgeGraph(
  value: unknown,
): Effect.Effect<
  OpenAgentsRepoStudiedKnowledgeGraph,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studyGraph");
    const graph = yield* decodeStudyGraphSchema(
      OpenAgentsRepoStudiedKnowledgeGraph,
      value,
      "studyGraph",
    );
    yield* validateOpenAgentsRepoStudiedKnowledgeGraph(graph);
    return graph;
  });
}

export function traverseOpenAgentsRepoStudiedKnowledgeGraph(
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
  input: TraverseOpenAgentsRepoStudiedKnowledgeGraphInput,
): Effect.Effect<OpenAgentsRepoStudiedKnowledgeTraversal, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const decodedGraph = yield* decodeOpenAgentsRepoStudiedKnowledgeGraph(graph);
    const fromNode = resolveTraversalStart(decodedGraph, input);

    if (fromNode === undefined) {
      return yield* studyGraphError("studyTraversal.fromNodeRef", "start node must resolve in the graph");
    }

    const outgoing = decodedGraph.edges.filter((edge) => edge.fromNodeRef === fromNode.ref);
    const result: OpenAgentsRepoStudiedKnowledgeTraversal = {
      auditNodeRefs: collectTargets(outgoing, ["code_explained_by_audit", "code_explained_by_roadmap"]),
      commitNodeRefs: collectTargets(outgoing, ["edit_site_commit_context"]),
      fromNodeRef: fromNode.ref,
      graphHash: decodedGraph.graphHash,
      invariantNodeRefs: collectTargets(outgoing, ["edit_site_respects_invariant"]),
      issueNodeRefs: collectTargets(outgoing, ["issue_tracks_edit_site"]),
      rejectedLineageNodeRefs: collectTargets(outgoing, ["code_warned_by_rejected_lineage"]),
      schemaRef: OPENAGENTS_REPO_STUDIED_KNOWLEDGE_TRAVERSAL_SCHEMA_REF,
      traversedEdgeRefs: outgoing
        .filter((edge) =>
          [
            "code_explained_by_audit",
            "code_explained_by_roadmap",
            "code_warned_by_rejected_lineage",
            "edit_site_commit_context",
            "issue_tracks_edit_site",
            "edit_site_respects_invariant",
          ].includes(edge.kind),
        )
        .map((edge) => edge.ref)
        .sort((left, right) => left.localeCompare(right)),
    };

    yield* validateProbeBenchmarkPublicProjection(result, "studyTraversal");
    return yield* decodeStudyGraphSchema(
      OpenAgentsRepoStudiedKnowledgeTraversal,
      result,
      "studyTraversal",
    );
  });
}

export function openAgentsRepoStudiedKnowledgeGraphHash(
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
): string {
  const {
    generatedAt: _generatedAt,
    graphHash: _graphHash,
    graphRef: _graphRef,
    ...stable
  } = graph;
  return sha256Ref(stableJson(stable));
}

export function openAgentsRepoStudiedKnowledgeNodeHash(
  node: Omit<OpenAgentsRepoStudiedKnowledgeNode, "nodeHash" | "ref">,
): string {
  return sha256Ref(stableJson(node));
}

export function openAgentsRepoStudiedKnowledgeEdgeHash(
  edge: Omit<OpenAgentsRepoStudiedKnowledgeEdge, "edgeHash" | "ref">,
): string {
  return sha256Ref(stableJson(edge));
}

class StudyGraphNodeBuilder {
  private readonly nodeByCommit = new Map<string, OpenAgentsRepoStudiedKnowledgeNode>();
  private readonly nodeByIssueRef = new Map<string, OpenAgentsRepoStudiedKnowledgeNode>();
  private readonly nodeByPath = new Map<string, OpenAgentsRepoStudiedKnowledgeNode>();
  private readonly nodeByRationaleKind = new Map<string, OpenAgentsRepoStudiedKnowledgeNode>();
  private readonly nodeBySpanPath = new Map<string, OpenAgentsRepoStudiedKnowledgeNode>();
  private readonly nodeByRef = new Map<string, OpenAgentsRepoStudiedKnowledgeNode>();

  constructor(private readonly packet: OpenAgentsRepoStudyPacket) {}

  addCorpusEntry(path: string, sourceAuthorityRefs: ReadonlyArray<string>): OpenAgentsRepoStudiedKnowledgeNode {
    const existing = this.nodeByPath.get(path);

    if (existing !== undefined) {
      return existing;
    }

    const sourceDigest = sha256Ref(stableJson({
      corpusManifestHash: this.packet.corpusManifestHash,
      path,
    }));
    const node = makeStudyGraphNode({
      kind: corpusNodeKind(path),
      label: path,
      source: {
        corpusManifestHash: this.packet.corpusManifestHash,
        corpusManifestRef: this.packet.corpusManifestRef,
        kind: "corpus_entry",
        packetHash: this.packet.packetHash,
        packetRef: this.packet.packetRef,
        path,
        sourceDigest,
      },
      sourceAuthorityRefs,
    });
    this.remember(node);
    this.nodeByPath.set(path, node);
    return node;
  }

  addEvidenceSpan(span: OpenAgentsRepoStudyPacket["evidenceSpans"][number]): OpenAgentsRepoStudiedKnowledgeNode {
    const existing = this.nodeBySpanPath.get(span.evidence.path);

    if (existing !== undefined) {
      return existing;
    }

    const node = makeStudyGraphNode({
      kind: "evidence_span",
      label: `${span.evidence.path}:${span.evidence.start_line}-${span.evidence.end_line}`,
      source: {
        corpusManifestHash: this.packet.corpusManifestHash,
        corpusManifestRef: this.packet.corpusManifestRef,
        kind: "evidence_span",
        packetHash: this.packet.packetHash,
        packetRef: this.packet.packetRef,
        path: span.evidence.path,
        sourceDigest: span.spanHash,
        spanHash: span.spanHash,
        spanId: span.evidence.span_id,
      },
      sourceAuthorityRefs: ["authority.openagents.repo_study.evidence_span"],
    });
    this.remember(node);
    this.nodeBySpanPath.set(span.evidence.path, node);
    return node;
  }

  addCommit(commit: OpenAgentsRepoStudyPacket["commitHistory"][number]): OpenAgentsRepoStudiedKnowledgeNode {
    const existing = this.nodeByCommit.get(commit.commit);

    if (existing !== undefined) {
      return existing;
    }

    const node = makeStudyGraphNode({
      kind: "commit",
      label: `commit.${commit.commit.slice(0, 12)}`,
      source: {
        commit: commit.commit,
        kind: "commit_history",
        packetHash: this.packet.packetHash,
        packetRef: this.packet.packetRef,
        sourceDigest: sha256Ref(stableJson(commit)),
        subjectDigest: commit.subjectDigest,
      },
      sourceAuthorityRefs: ["authority.openagents.repo_study.commit_history"],
    });
    this.remember(node);
    this.nodeByCommit.set(commit.commit, node);
    return node;
  }

  addRationaleSource(source: OpenAgentsRepoStudyPacket["rationaleSources"][number]): OpenAgentsRepoStudiedKnowledgeNode {
    const existing = this.nodeByRationaleKind.get(source.kind);

    if (existing !== undefined) {
      return existing;
    }

    const sourceDigest = source.sourceHash ?? sha256Ref(stableJson(source));
    const node = makeStudyGraphNode({
      kind: "rationale",
      label: `rationale.${source.kind}`,
      source: {
        commit: source.commit,
        kind: "rationale_source",
        packetHash: this.packet.packetHash,
        packetRef: this.packet.packetRef,
        path: source.path,
        rationaleKind: source.kind,
        rationaleRef: source.ref,
        sourceDigest,
        sourceHash: source.sourceHash,
      },
      sourceAuthorityRefs: [`authority.openagents.repo_study.rationale.${source.kind}`],
    });
    this.remember(node);
    this.nodeByRationaleKind.set(source.kind, node);
    return node;
  }

  addIssue(issue: OpenAgentsRepoStudiedKnowledgeIssueRef): OpenAgentsRepoStudiedKnowledgeNode {
    const existing = this.nodeByIssueRef.get(issue.issueRef);

    if (existing !== undefined) {
      return existing;
    }

    const node = makeStudyGraphNode({
      kind: "issue",
      label: issue.label,
      source: {
        issueNumber: issue.issueNumber,
        issueRef: issue.issueRef,
        kind: "github_issue",
        packetHash: this.packet.packetHash,
        packetRef: this.packet.packetRef,
        sourceDigest: sha256Ref(stableJson({
          issueNumber: issue.issueNumber,
          issueRef: issue.issueRef,
          packetHash: this.packet.packetHash,
        })),
      },
      sourceAuthorityRefs: ["authority.openagents.repo_study.github_issue"],
    });
    this.remember(node);
    this.nodeByIssueRef.set(issue.issueRef, node);
    return node;
  }

  byCommit(commit: string): OpenAgentsRepoStudiedKnowledgeNode | undefined {
    return this.nodeByCommit.get(commit);
  }

  byIssueNumber(issueNumber: number): OpenAgentsRepoStudiedKnowledgeNode | undefined {
    return [...this.nodeByIssueRef.values()].find((node) => node.source.issueNumber === issueNumber);
  }

  byPath(path: string): OpenAgentsRepoStudiedKnowledgeNode | undefined {
    return this.nodeByPath.get(path);
  }

  byRationaleKind(kind: OpenAgentsRepoStudyPacketRationaleKind): OpenAgentsRepoStudiedKnowledgeNode | undefined {
    return this.nodeByRationaleKind.get(kind);
  }

  evidenceByPath(path: string): OpenAgentsRepoStudiedKnowledgeNode | undefined {
    return this.nodeBySpanPath.get(path);
  }

  nodes(): ReadonlyArray<OpenAgentsRepoStudiedKnowledgeNode> {
    return [...this.nodeByRef.values()].sort((left, right) => left.ref.localeCompare(right.ref));
  }

  private remember(node: OpenAgentsRepoStudiedKnowledgeNode): void {
    this.nodeByRef.set(node.ref, node);
  }
}

function buildStudiedKnowledgeEdges(input: {
  readonly editSitePaths: ReadonlyArray<string>;
  readonly nodeBuilder: StudyGraphNodeBuilder;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly trackingIssueNumber: number;
}): ReadonlyArray<OpenAgentsRepoStudiedKnowledgeEdge> {
  const edges: OpenAgentsRepoStudiedKnowledgeEdge[] = [];
  const latestCommit = input.packet.commitHistory[0];
  const latestCommitNode = latestCommit === undefined ? undefined : input.nodeBuilder.byCommit(latestCommit.commit);
  const auditNode = input.nodeBuilder.byRationaleKind("tassadar_audit");
  const roadmapNode = input.nodeBuilder.byRationaleKind("machine_studying_roadmap");
  const backroomNode = input.nodeBuilder.byRationaleKind("backroom_archive");
  const s2IssueNode = input.nodeBuilder.byIssueNumber(input.trackingIssueNumber);
  const invariantNodes = input.nodeBuilder.nodes().filter((node) => node.kind === "invariant");
  const auditEvidence = input.nodeBuilder.evidenceByPath(
    "docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md",
  );
  const roadmapEvidence = input.nodeBuilder.evidenceByPath(
    "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md",
  );

  for (const path of input.editSitePaths) {
    const editSite = input.nodeBuilder.byPath(path);

    if (editSite === undefined) {
      continue;
    }

    for (const invariantNode of invariantNodes) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: editSite.ref,
        kind: "edit_site_respects_invariant",
        rationaleRef: "rationale.openagents.repo_study.edit_site_invariant_boundary",
        sourceEvidenceNodeRefs: evidenceRefsFor(input.nodeBuilder, invariantNode.source.path),
        toNodeRef: invariantNode.ref,
      }));
    }

    if (latestCommitNode !== undefined) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: editSite.ref,
        kind: "edit_site_commit_context",
        rationaleRef: "rationale.openagents.repo_study.s1_commit_history_context",
        sourceEvidenceNodeRefs: [],
        toNodeRef: latestCommitNode.ref,
      }));
    }

    if (auditNode !== undefined) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: editSite.ref,
        kind: "code_explained_by_audit",
        rationaleRef: "rationale.openagents.repo_study.tassadar_audit_explains_edit_site",
        sourceEvidenceNodeRefs: auditEvidence === undefined ? [] : [auditEvidence.ref],
        toNodeRef: auditNode.ref,
      }));
    }

    if (roadmapNode !== undefined) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: editSite.ref,
        kind: "code_explained_by_roadmap",
        rationaleRef: "rationale.openagents.repo_study.machine_studying_roadmap_explains_edit_site",
        sourceEvidenceNodeRefs: roadmapEvidence === undefined ? [] : [roadmapEvidence.ref],
        toNodeRef: roadmapNode.ref,
      }));
    }

    if (backroomNode !== undefined) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: editSite.ref,
        kind: "code_warned_by_rejected_lineage",
        rationaleRef: "rationale.openagents.repo_study.backroom_rejected_lineage_boundary",
        sourceEvidenceNodeRefs: [],
        toNodeRef: backroomNode.ref,
      }));
    }

    if (s2IssueNode !== undefined) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: editSite.ref,
        kind: "issue_tracks_edit_site",
        rationaleRef: "rationale.openagents.repo_study.issue_acceptance_tracks_edit_site",
        sourceEvidenceNodeRefs: [],
        toNodeRef: s2IssueNode.ref,
      }));
      edges.push(makeStudyGraphEdge({
        fromNodeRef: s2IssueNode.ref,
        kind: "issue_depends_on_packet",
        rationaleRef: "rationale.openagents.repo_study.issue_depends_on_s1_packet",
        sourceEvidenceNodeRefs: [],
        toNodeRef: editSite.ref,
      }));
    }
  }

  for (const node of input.nodeBuilder.nodes()) {
    if (node.source.kind !== "corpus_entry") {
      continue;
    }

    const evidenceNode = input.nodeBuilder.evidenceByPath(node.source.path ?? "");

    if (evidenceNode !== undefined) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: evidenceNode.ref,
        kind: "evidence_span_supports_node",
        rationaleRef: "rationale.openagents.repo_study.evidence_span_resolves_corpus_entry",
        sourceEvidenceNodeRefs: [evidenceNode.ref],
        toNodeRef: node.ref,
      }));
    }
  }

  for (const rationaleNode of input.nodeBuilder.nodes().filter((node) => node.kind === "rationale")) {
    const path = rationaleNode.source.path;
    const docNode = path === undefined ? undefined : input.nodeBuilder.byPath(path);

    if (docNode !== undefined) {
      edges.push(makeStudyGraphEdge({
        fromNodeRef: docNode.ref,
        kind: "doc_explains_rationale",
        rationaleRef: "rationale.openagents.repo_study.doc_resolves_rationale_source",
        sourceEvidenceNodeRefs: evidenceRefsFor(input.nodeBuilder, path),
        toNodeRef: rationaleNode.ref,
      }));
    }
  }

  for (const section of input.packet.sections) {
    for (const path of section.corpusEntryPaths) {
      const node = input.nodeBuilder.byPath(path);

      if (node === undefined) {
        continue;
      }

      const sectionDigest = sha256Ref(stableJson({
        corpusManifestHash: input.packet.corpusManifestHash,
        path,
        sectionRef: section.ref,
      }));
      edges.push(makeStudyGraphEdge({
        fromNodeRef: node.ref,
        kind: "section_indexes_entry",
        rationaleRef: `rationale.openagents.repo_study.section.${shortHash(sectionDigest)}`,
        sourceEvidenceNodeRefs: evidenceRefsFor(input.nodeBuilder, path),
        toNodeRef: node.ref,
      }));
    }
  }

  return dedupeEdges(edges).sort((left, right) => left.ref.localeCompare(right.ref));
}

function validateOpenAgentsRepoStudiedKnowledgeGraph(
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(graph.repo, "studyGraph.repo");
    yield* requireNonEmpty(graph.commit, "studyGraph.commit");
    yield* requireNonEmpty(graph.packetRef, "studyGraph.packetRef");
    yield* requireNonEmpty(graph.graphRef, "studyGraph.graphRef");
    yield* requireSha256(graph.graphHash, "studyGraph.graphHash");
    yield* requireSha256(graph.packetHash, "studyGraph.packetHash");
    yield* requireSha256(graph.corpusManifestHash, "studyGraph.corpusManifestHash");

    if (graph.graphHash !== openAgentsRepoStudiedKnowledgeGraphHash(graph)) {
      return yield* studyGraphError("studyGraph.graphHash", "must match deterministic graph content hash");
    }

    const nodeRefs = new Set<string>();
    const edgeRefs = new Set<string>();
    const edgeHashes = new Set<string>();

    if (graph.nodes.length === 0) {
      return yield* studyGraphError("studyGraph.nodes", "must include nodes");
    }

    if (graph.edges.length === 0) {
      return yield* studyGraphError("studyGraph.edges", "must include edges");
    }

    for (const [index, node] of graph.nodes.entries()) {
      const path = `studyGraph.nodes[${index}]`;
      yield* requireNonEmpty(node.ref, `${path}.ref`);
      yield* requireNonEmpty(node.label, `${path}.label`);
      yield* requireSha256(node.nodeHash, `${path}.nodeHash`);
      yield* requireSha256(node.source.sourceDigest, `${path}.source.sourceDigest`);
      yield* requireNonEmptyRefs(node.sourceAuthorityRefs, `${path}.sourceAuthorityRefs`);

      if (node.nodeHash !== openAgentsRepoStudiedKnowledgeNodeHash(stripNodeIdentity(node))) {
        return yield* studyGraphError(`${path}.nodeHash`, "must match deterministic node content hash");
      }

      if (nodeRefs.has(node.ref)) {
        return yield* studyGraphError(`${path}.ref`, "must be unique");
      }

      nodeRefs.add(node.ref);
    }

    for (const [index, edge] of graph.edges.entries()) {
      const path = `studyGraph.edges[${index}]`;
      yield* requireNonEmpty(edge.ref, `${path}.ref`);
      yield* requireNonEmpty(edge.fromNodeRef, `${path}.fromNodeRef`);
      yield* requireNonEmpty(edge.toNodeRef, `${path}.toNodeRef`);
      yield* requireNonEmpty(edge.rationaleRef, `${path}.rationaleRef`);
      yield* requireSha256(edge.edgeHash, `${path}.edgeHash`);

      if (edge.edgeHash !== openAgentsRepoStudiedKnowledgeEdgeHash(stripEdgeIdentity(edge))) {
        return yield* studyGraphError(`${path}.edgeHash`, "must match deterministic edge content hash");
      }

      if (edgeRefs.has(edge.ref)) {
        return yield* studyGraphError(`${path}.ref`, "must be unique");
      }

      if (edgeHashes.has(edge.edgeHash)) {
        return yield* studyGraphError(`${path}.edgeHash`, "must be unique");
      }

      if (!nodeRefs.has(edge.fromNodeRef)) {
        return yield* studyGraphError(`${path}.fromNodeRef`, "must resolve to a graph node");
      }

      if (!nodeRefs.has(edge.toNodeRef)) {
        return yield* studyGraphError(`${path}.toNodeRef`, "must resolve to a graph node");
      }

      for (const [sourceIndex, sourceRef] of edge.sourceEvidenceNodeRefs.entries()) {
        if (!nodeRefs.has(sourceRef)) {
          return yield* studyGraphError(
            `${path}.sourceEvidenceNodeRefs[${sourceIndex}]`,
            "must resolve to a graph node",
          );
        }
      }

      edgeRefs.add(edge.ref);
      edgeHashes.add(edge.edgeHash);
    }

    const nodeKinds = new Set(graph.nodes.map((node) => node.kind));
    for (const requiredKind of ["code", "commit", "doc", "issue", "rationale", "invariant"] as const) {
      if (!nodeKinds.has(requiredKind)) {
        return yield* studyGraphError("studyGraph.nodes", `missing ${requiredKind} node`);
      }
    }

    const codeNodeRefs = new Set(graph.nodes.filter((node) => node.kind === "code").map((node) => node.ref));
    const codeEdgeKinds = graph.edges
      .filter((edge) => codeNodeRefs.has(edge.fromNodeRef))
      .reduce((map, edge) => {
        const set = map.get(edge.fromNodeRef) ?? new Set<OpenAgentsRepoStudiedKnowledgeEdgeKind>();
        set.add(edge.kind);
        map.set(edge.fromNodeRef, set);
        return map;
      }, new Map<string, Set<OpenAgentsRepoStudiedKnowledgeEdgeKind>>());
    const completeEditSite = [...codeEdgeKinds.values()].some((kinds) =>
      [
        "code_explained_by_audit",
        "code_warned_by_rejected_lineage",
        "edit_site_commit_context",
        "issue_tracks_edit_site",
        "edit_site_respects_invariant",
      ].every((kind) => kinds.has(kind)),
    );

    if (!completeEditSite) {
      return yield* studyGraphError(
        "studyGraph.edges",
        "must include an edit-site traversal to invariant, commit, audit, issue, and rejected lineage",
      );
    }
  });
}

function makeStudyGraphNode(input: {
  readonly kind: OpenAgentsRepoStudiedKnowledgeNodeKind;
  readonly label: string;
  readonly source: OpenAgentsRepoStudiedKnowledgeNodeSource;
  readonly sourceAuthorityRefs: ReadonlyArray<string>;
}): OpenAgentsRepoStudiedKnowledgeNode {
  const body = {
    kind: input.kind,
    label: input.label,
    source: input.source,
    sourceAuthorityRefs: [...new Set(input.sourceAuthorityRefs)].sort((left, right) => left.localeCompare(right)),
  };
  const nodeHash = openAgentsRepoStudiedKnowledgeNodeHash(body);

  return {
    ...body,
    nodeHash,
    ref: `study_node.${input.kind}.${shortHash(nodeHash)}`,
  };
}

function makeStudyGraphEdge(input: {
  readonly fromNodeRef: string;
  readonly kind: OpenAgentsRepoStudiedKnowledgeEdgeKind;
  readonly rationaleRef: string;
  readonly sourceEvidenceNodeRefs: ReadonlyArray<string>;
  readonly toNodeRef: string;
}): OpenAgentsRepoStudiedKnowledgeEdge {
  const body = {
    fromNodeRef: input.fromNodeRef,
    kind: input.kind,
    rationaleRef: input.rationaleRef,
    sourceEvidenceNodeRefs: [...new Set(input.sourceEvidenceNodeRefs)].sort((left, right) => left.localeCompare(right)),
    toNodeRef: input.toNodeRef,
  };
  const edgeHash = openAgentsRepoStudiedKnowledgeEdgeHash(body);

  return {
    ...body,
    edgeHash,
    ref: `study_edge.${input.kind}.${shortHash(edgeHash)}`,
  };
}

function resolveTraversalStart(
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
  input: TraverseOpenAgentsRepoStudiedKnowledgeGraphInput,
): OpenAgentsRepoStudiedKnowledgeNode | undefined {
  if (input.fromNodeRef !== undefined) {
    return graph.nodes.find((node) => node.ref === input.fromNodeRef);
  }

  if (input.path !== undefined) {
    return graph.nodes.find((node) => node.source.path === input.path && node.source.kind === "corpus_entry");
  }

  return undefined;
}

function collectTargets(
  edges: ReadonlyArray<OpenAgentsRepoStudiedKnowledgeEdge>,
  kinds: ReadonlyArray<OpenAgentsRepoStudiedKnowledgeEdgeKind>,
): ReadonlyArray<string> {
  const kindSet = new Set(kinds);
  return [...new Set(edges.filter((edge) => kindSet.has(edge.kind)).map((edge) => edge.toNodeRef))].sort(
    (left, right) => left.localeCompare(right),
  );
}

function corpusNodeKind(path: string): OpenAgentsRepoStudiedKnowledgeNodeKind {
  const lower = path.toLowerCase();

  if (lower.endsWith("invariants.md")) {
    return "invariant";
  }

  if (
    lower.endsWith(".ts") ||
    lower.endsWith(".tsx") ||
    lower.endsWith(".js") ||
    lower.endsWith(".jsx") ||
    lower.endsWith(".json") ||
    lower.endsWith(".jsonl")
  ) {
    return "code";
  }

  return "doc";
}

function evidenceRefsFor(
  nodeBuilder: StudyGraphNodeBuilder,
  path: string | undefined,
): ReadonlyArray<string> {
  if (path === undefined) {
    return [];
  }

  const node = nodeBuilder.evidenceByPath(path);
  return node === undefined ? [] : [node.ref];
}

function dedupeEdges(
  edges: ReadonlyArray<OpenAgentsRepoStudiedKnowledgeEdge>,
): ReadonlyArray<OpenAgentsRepoStudiedKnowledgeEdge> {
  const byHash = new Map<string, OpenAgentsRepoStudiedKnowledgeEdge>();

  for (const edge of edges) {
    byHash.set(edge.edgeHash, edge);
  }

  return [...byHash.values()];
}

function stripNodeIdentity(
  node: OpenAgentsRepoStudiedKnowledgeNode,
): Omit<OpenAgentsRepoStudiedKnowledgeNode, "nodeHash" | "ref"> {
  const { nodeHash: _nodeHash, ref: _ref, ...body } = node;
  return body;
}

function stripEdgeIdentity(
  edge: OpenAgentsRepoStudiedKnowledgeEdge,
): Omit<OpenAgentsRepoStudiedKnowledgeEdge, "edgeHash" | "ref"> {
  const { edgeHash: _edgeHash, ref: _ref, ...body } = edge;
  return body;
}

function decodeStudyGraphSchema<A, I>(
  schema: S.Schema<A, I>,
  value: unknown,
  path: string,
): Effect.Effect<A, ProbeBenchmarkContractError> {
  return S.decodeUnknownEffect(schema)(value).pipe(
    Effect.mapError(
      (error) =>
        new ProbeBenchmarkContractError({
          path,
          reason: String(error),
        }),
    ),
  );
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0 ? studyGraphError(path, "must be a non-empty string") : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return studyGraphError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1 ? Effect.void : studyGraphError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : studyGraphError(path, "must be a sha256 ref");
}

function studyGraphError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
