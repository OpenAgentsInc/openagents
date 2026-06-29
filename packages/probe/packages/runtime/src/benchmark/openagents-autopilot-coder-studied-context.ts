import { createHash } from "node:crypto";
import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import { type ProbeToolMenuPlannerInput } from "../blueprint/tool-menu";
import {
  OpenAgentsRepoStudiedKnowledgeGraph,
  decodeOpenAgentsRepoStudiedKnowledgeGraph,
  traverseOpenAgentsRepoStudiedKnowledgeGraph,
  type OpenAgentsRepoStudiedKnowledgeEdgeKind,
  type OpenAgentsRepoStudiedKnowledgeTraversal,
} from "./openagents-study-graph";
import {
  OpenAgentsRepoStudyPacket,
  decodeOpenAgentsRepoStudyPacket,
  openAgentsRepoStudyPacketHash,
} from "./openagents-study-packet";

export const OPENAGENTS_AUTOPILOT_CODER_STUDIED_CONTEXT_SCHEMA_REF =
  "openagents.autopilot_coder_studied_context.v0" as const;
export const OPENAGENTS_AUTOPILOT_CODER_STUDIED_PLAN_CONTEXT_SCHEMA_REF =
  "openagents.autopilot_coder_studied_plan_context.v0" as const;

export const OpenAgentsAutopilotCoderStudiedContext = S.Struct({
  auditNodeRefs: S.Array(S.String),
  blockedApproachRefs: S.Array(S.String),
  caveatRefs: S.Array(S.String),
  contextHash: S.String,
  contextPackRef: S.String,
  editSiteNodeRef: S.String,
  editSitePath: S.String,
  graphHash: S.String,
  graphRef: S.String,
  introducingCommitNodeRefs: S.Array(S.String),
  invariantNodeRefs: S.Array(S.String),
  issueNodeRefs: S.Array(S.String),
  keywordRoutingAllowed: S.Literal(false),
  mutationAuthority: S.Literal(false),
  packetHash: S.String,
  packetRef: S.String,
  rejectedLineageNodeRefs: S.Array(S.String),
  schemaRef: S.Literal(OPENAGENTS_AUTOPILOT_CODER_STUDIED_CONTEXT_SCHEMA_REF),
  sourceAuthorityRefs: S.Array(S.String),
  sourceBoundary: S.Literal("public_refs_only"),
  sourceEvidenceNodeRefs: S.Array(S.String),
  traversalEdgeRefs: S.Array(S.String),
  traversalRef: S.String,
});
export type OpenAgentsAutopilotCoderStudiedContext =
  typeof OpenAgentsAutopilotCoderStudiedContext.Type;

export const OpenAgentsAutopilotCoderStudiedPlanContext = S.Struct({
  auditNodeRefs: S.Array(S.String),
  blockedApproachRefs: S.Array(S.String),
  contextPackRef: S.String,
  contextPackRefs: S.Array(S.String),
  editSitePath: S.String,
  graphRef: S.String,
  introducingCommitNodeRefs: S.Array(S.String),
  invariantNodeRefs: S.Array(S.String),
  keywordRoutingAllowed: S.Literal(false),
  mutationAuthority: S.Literal(false),
  packetRef: S.String,
  planContextHash: S.String,
  planContextRef: S.String,
  readFirstFileRefs: S.Array(S.String),
  rejectedLineageNodeRefs: S.Array(S.String),
  schemaRef: S.Literal(OPENAGENTS_AUTOPILOT_CODER_STUDIED_PLAN_CONTEXT_SCHEMA_REF),
  sourceAuthorityRefs: S.Array(S.String),
  toolMenuHintRefs: S.Array(S.String),
  traversalRef: S.String,
});
export type OpenAgentsAutopilotCoderStudiedPlanContext =
  typeof OpenAgentsAutopilotCoderStudiedPlanContext.Type;

export interface BuildOpenAgentsAutopilotCoderStudiedContextInput {
  readonly editSitePath?: string;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly packet: OpenAgentsRepoStudyPacket;
}

export interface BuildOpenAgentsAutopilotCoderStudiedPlanContextInput {
  readonly context: OpenAgentsAutopilotCoderStudiedContext;
  readonly existingContextPackRefs?: ReadonlyArray<string>;
  readonly planContextRef?: string;
}

const PLAN_EDGE_KINDS: ReadonlyArray<OpenAgentsRepoStudiedKnowledgeEdgeKind> = [
  "code_explained_by_audit",
  "code_explained_by_roadmap",
  "code_warned_by_rejected_lineage",
  "edit_site_commit_context",
  "edit_site_respects_invariant",
  "issue_tracks_edit_site",
];

export function buildOpenAgentsAutopilotCoderStudiedContext(
  input: BuildOpenAgentsAutopilotCoderStudiedContextInput,
): Effect.Effect<
  OpenAgentsAutopilotCoderStudiedContext,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const packet = yield* decodeOpenAgentsRepoStudyPacket(input.packet);
    const graph = yield* decodeOpenAgentsRepoStudiedKnowledgeGraph(input.graph);
    yield* validatePacketGraphPair(packet, graph);

    const editSitePath = input.editSitePath ?? firstCompleteEditSitePath(graph);

    if (editSitePath === undefined) {
      return yield* studiedContextError("autopilotCoderStudiedContext.editSitePath", "no complete edit-site traversal is available");
    }

    const traversal = yield* traverseOpenAgentsRepoStudiedKnowledgeGraph(graph, { path: editSitePath });
    const editSiteNode = graph.nodes.find((node) => node.ref === traversal.fromNodeRef);

    if (editSiteNode === undefined) {
      return yield* studiedContextError("autopilotCoderStudiedContext.editSiteNodeRef", "edit site node must resolve");
    }

    const sourceEvidenceNodeRefs = sourceEvidenceNodeRefsForTraversal(graph, traversal);
    const baseContext: OpenAgentsAutopilotCoderStudiedContext = {
      auditNodeRefs: traversal.auditNodeRefs,
      blockedApproachRefs: blockedApproachRefsFor(traversal),
      caveatRefs: ["caveat.openagents.autopilot_coder.studied_context_evidence_only"],
      contextHash: "sha256:pending",
      contextPackRef: "context_pack.openagents.autopilot_coder.studied_context.pending",
      editSiteNodeRef: traversal.fromNodeRef,
      editSitePath,
      graphHash: graph.graphHash,
      graphRef: graph.graphRef,
      introducingCommitNodeRefs: traversal.commitNodeRefs,
      invariantNodeRefs: traversal.invariantNodeRefs,
      issueNodeRefs: traversal.issueNodeRefs,
      keywordRoutingAllowed: false,
      mutationAuthority: false,
      packetHash: packet.packetHash,
      packetRef: packet.packetRef,
      rejectedLineageNodeRefs: traversal.rejectedLineageNodeRefs,
      schemaRef: OPENAGENTS_AUTOPILOT_CODER_STUDIED_CONTEXT_SCHEMA_REF,
      sourceAuthorityRefs: sourceAuthorityRefsFor(traversal),
      sourceBoundary: "public_refs_only",
      sourceEvidenceNodeRefs,
      traversalEdgeRefs: traversal.traversedEdgeRefs,
      traversalRef: traversalRefFor(editSitePath, graph.graphHash),
    };
    const contextHash = openAgentsAutopilotCoderStudiedContextHash(baseContext);
    const context: OpenAgentsAutopilotCoderStudiedContext = {
      ...baseContext,
      contextHash,
      contextPackRef: `context_pack.openagents.autopilot_coder.studied_context.${shortHash(contextHash)}`,
    };

    return yield* decodeOpenAgentsAutopilotCoderStudiedContext(context);
  });
}

export function buildOpenAgentsAutopilotCoderStudiedPlanContext(
  input: BuildOpenAgentsAutopilotCoderStudiedPlanContextInput,
): Effect.Effect<
  OpenAgentsAutopilotCoderStudiedPlanContext,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const context = yield* decodeOpenAgentsAutopilotCoderStudiedContext(input.context);
    const contextPackRefs = uniqueRefs([
      ...(input.existingContextPackRefs ?? []),
      context.contextPackRef,
    ]);
    const basePlanContext: OpenAgentsAutopilotCoderStudiedPlanContext = {
      auditNodeRefs: context.auditNodeRefs,
      blockedApproachRefs: context.blockedApproachRefs,
      contextPackRef: context.contextPackRef,
      contextPackRefs,
      editSitePath: context.editSitePath,
      graphRef: context.graphRef,
      introducingCommitNodeRefs: context.introducingCommitNodeRefs,
      invariantNodeRefs: context.invariantNodeRefs,
      keywordRoutingAllowed: false,
      mutationAuthority: false,
      packetRef: context.packetRef,
      planContextHash: "sha256:pending",
      planContextRef: "autopilot_coder_studied_plan_context.pending",
      readFirstFileRefs: [context.editSitePath],
      rejectedLineageNodeRefs: context.rejectedLineageNodeRefs,
      schemaRef: OPENAGENTS_AUTOPILOT_CODER_STUDIED_PLAN_CONTEXT_SCHEMA_REF,
      sourceAuthorityRefs: context.sourceAuthorityRefs,
      toolMenuHintRefs: [
        "tool_hint.openagents.autopilot_coder.read_studied_edit_site_first",
        "tool_hint.openagents.autopilot_coder.record_studied_context_evidence",
      ],
      traversalRef: context.traversalRef,
    };
    const planContextHash = openAgentsAutopilotCoderStudiedPlanContextHash(basePlanContext);
    const planContext: OpenAgentsAutopilotCoderStudiedPlanContext = {
      ...basePlanContext,
      planContextHash,
      planContextRef: input.planContextRef ?? `autopilot_coder_studied_plan_context.${shortHash(planContextHash)}`,
    };

    return yield* decodeOpenAgentsAutopilotCoderStudiedPlanContext(planContext);
  });
}

export function applyOpenAgentsAutopilotCoderStudiedContextToToolMenuInput(
  input: ProbeToolMenuPlannerInput,
  context: OpenAgentsAutopilotCoderStudiedContext,
): ProbeToolMenuPlannerInput {
  return {
    ...input,
    contextPackRefs: uniqueRefs([...input.contextPackRefs, context.contextPackRef]),
    sourceAuthorityRefs: uniqueRefs([...input.sourceAuthorityRefs, ...context.sourceAuthorityRefs]),
  };
}

export function decodeOpenAgentsAutopilotCoderStudiedContext(
  value: unknown,
): Effect.Effect<OpenAgentsAutopilotCoderStudiedContext, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "autopilotCoderStudiedContext");
    const context = yield* decodeStudiedContextSchema(
      OpenAgentsAutopilotCoderStudiedContext,
      value,
      "autopilotCoderStudiedContext",
    );
    yield* validateOpenAgentsAutopilotCoderStudiedContext(context);
    return context;
  });
}

export function decodeOpenAgentsAutopilotCoderStudiedPlanContext(
  value: unknown,
): Effect.Effect<OpenAgentsAutopilotCoderStudiedPlanContext, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "autopilotCoderStudiedPlanContext");
    const planContext = yield* decodeStudiedContextSchema(
      OpenAgentsAutopilotCoderStudiedPlanContext,
      value,
      "autopilotCoderStudiedPlanContext",
    );
    yield* validateOpenAgentsAutopilotCoderStudiedPlanContext(planContext);
    return planContext;
  });
}

export function openAgentsAutopilotCoderStudiedContextHash(
  context: OpenAgentsAutopilotCoderStudiedContext,
): string {
  const {
    contextHash: _contextHash,
    contextPackRef: _contextPackRef,
    ...stable
  } = context;
  return sha256Ref(stableJson(stable));
}

export function openAgentsAutopilotCoderStudiedPlanContextHash(
  planContext: OpenAgentsAutopilotCoderStudiedPlanContext,
): string {
  const {
    planContextHash: _planContextHash,
    planContextRef: _planContextRef,
    ...stable
  } = planContext;
  return sha256Ref(stableJson(stable));
}

function firstCompleteEditSitePath(
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
): string | undefined {
  const outgoingKindsByNode = graph.edges.reduce((map, edge) => {
    const kinds = map.get(edge.fromNodeRef) ?? new Set<string>();
    kinds.add(edge.kind);
    map.set(edge.fromNodeRef, kinds);
    return map;
  }, new Map<string, Set<string>>());

  return graph.nodes
    .filter((node) => node.kind === "code" && node.source.kind === "corpus_entry" && node.source.path !== undefined)
    .filter((node) => {
      const kinds = outgoingKindsByNode.get(node.ref) ?? new Set<string>();
      return PLAN_EDGE_KINDS.every((kind) => kind === "code_explained_by_roadmap" || kinds.has(kind));
    })
    .map((node) => node.source.path ?? "")
    .sort((left, right) => left.localeCompare(right))[0];
}

function sourceEvidenceNodeRefsForTraversal(
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
  traversal: OpenAgentsRepoStudiedKnowledgeTraversal,
): ReadonlyArray<string> {
  const traversedEdgeRefs = new Set(traversal.traversedEdgeRefs);
  return uniqueRefs(
    graph.edges
      .filter((edge) => traversedEdgeRefs.has(edge.ref))
      .flatMap((edge) => edge.sourceEvidenceNodeRefs),
  );
}

function blockedApproachRefsFor(
  traversal: OpenAgentsRepoStudiedKnowledgeTraversal,
): ReadonlyArray<string> {
  return traversal.rejectedLineageNodeRefs.length === 0
    ? []
    : [
        "blocked_approach.openagents.autopilot_coder.reintroduce_backroom_rejected_lineage",
        ...traversal.rejectedLineageNodeRefs,
      ];
}

function sourceAuthorityRefsFor(
  traversal: OpenAgentsRepoStudiedKnowledgeTraversal,
): ReadonlyArray<string> {
  return uniqueRefs([
    "authority.openagents.autopilot_coder.studied_context",
    "authority.openagents.repo_study.packet",
    "authority.openagents.repo_study.graph",
    ...(traversal.invariantNodeRefs.length === 0 ? [] : ["authority.openagents.repo_study.invariants"]),
    ...(traversal.commitNodeRefs.length === 0 ? [] : ["authority.openagents.repo_study.commit_history"]),
    ...(traversal.auditNodeRefs.length === 0 ? [] : ["authority.openagents.repo_study.audit"]),
    ...(traversal.rejectedLineageNodeRefs.length === 0 ? [] : ["authority.openagents.repo_study.rejected_lineage"]),
  ]);
}

function traversalRefFor(editSitePath: string, graphHash: string): string {
  return `study_traversal.openagents.autopilot_coder.${slugPath(editSitePath)}.${shortHash(graphHash)}`;
}

function validatePacketGraphPair(
  packet: OpenAgentsRepoStudyPacket,
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  const packetHash = openAgentsRepoStudyPacketHash(packet);

  if (packet.packetHash !== packetHash) {
    return studiedContextError("autopilotCoderStudiedContext.packetHash", "packet hash must match packet content");
  }

  if (graph.packetHash !== packet.packetHash || graph.packetRef !== packet.packetRef) {
    return studiedContextError("autopilotCoderStudiedContext.graph", "graph must be built from the supplied packet");
  }

  return Effect.void;
}

function validateOpenAgentsAutopilotCoderStudiedContext(
  context: OpenAgentsAutopilotCoderStudiedContext,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(context.contextPackRef, "autopilotCoderStudiedContext.contextPackRef");
    yield* requireNonEmpty(context.editSiteNodeRef, "autopilotCoderStudiedContext.editSiteNodeRef");
    yield* requireNonEmpty(context.editSitePath, "autopilotCoderStudiedContext.editSitePath");
    yield* requireNonEmpty(context.traversalRef, "autopilotCoderStudiedContext.traversalRef");
    yield* requireSha256(context.contextHash, "autopilotCoderStudiedContext.contextHash");
    yield* requireSha256(context.graphHash, "autopilotCoderStudiedContext.graphHash");
    yield* requireSha256(context.packetHash, "autopilotCoderStudiedContext.packetHash");
    yield* requireNonEmptyRefs(context.invariantNodeRefs, "autopilotCoderStudiedContext.invariantNodeRefs");
    yield* requireNonEmptyRefs(context.introducingCommitNodeRefs, "autopilotCoderStudiedContext.introducingCommitNodeRefs");
    yield* requireNonEmptyRefs(context.auditNodeRefs, "autopilotCoderStudiedContext.auditNodeRefs");
    yield* requireNonEmptyRefs(context.rejectedLineageNodeRefs, "autopilotCoderStudiedContext.rejectedLineageNodeRefs");
    yield* requireNonEmptyRefs(context.traversalEdgeRefs, "autopilotCoderStudiedContext.traversalEdgeRefs");
    yield* requireNonEmptyRefs(context.sourceAuthorityRefs, "autopilotCoderStudiedContext.sourceAuthorityRefs");

    if (context.contextHash !== openAgentsAutopilotCoderStudiedContextHash(context)) {
      return yield* studiedContextError("autopilotCoderStudiedContext.contextHash", "must match deterministic context content hash");
    }
  });
}

function validateOpenAgentsAutopilotCoderStudiedPlanContext(
  planContext: OpenAgentsAutopilotCoderStudiedPlanContext,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(planContext.planContextRef, "autopilotCoderStudiedPlanContext.planContextRef");
    yield* requireNonEmpty(planContext.contextPackRef, "autopilotCoderStudiedPlanContext.contextPackRef");
    yield* requireNonEmptyRefs(planContext.contextPackRefs, "autopilotCoderStudiedPlanContext.contextPackRefs");
    yield* requireNonEmptyRefs(planContext.readFirstFileRefs, "autopilotCoderStudiedPlanContext.readFirstFileRefs");
    yield* requireNonEmptyRefs(planContext.invariantNodeRefs, "autopilotCoderStudiedPlanContext.invariantNodeRefs");
    yield* requireNonEmptyRefs(planContext.introducingCommitNodeRefs, "autopilotCoderStudiedPlanContext.introducingCommitNodeRefs");
    yield* requireNonEmptyRefs(planContext.auditNodeRefs, "autopilotCoderStudiedPlanContext.auditNodeRefs");
    yield* requireNonEmptyRefs(planContext.rejectedLineageNodeRefs, "autopilotCoderStudiedPlanContext.rejectedLineageNodeRefs");
    yield* requireSha256(planContext.planContextHash, "autopilotCoderStudiedPlanContext.planContextHash");

    if (!planContext.contextPackRefs.includes(planContext.contextPackRef)) {
      return yield* studiedContextError(
        "autopilotCoderStudiedPlanContext.contextPackRefs",
        "must include the studied context pack ref",
      );
    }

    if (planContext.planContextHash !== openAgentsAutopilotCoderStudiedPlanContextHash(planContext)) {
      return yield* studiedContextError(
        "autopilotCoderStudiedPlanContext.planContextHash",
        "must match deterministic plan context content hash",
      );
    }
  });
}

function decodeStudiedContextSchema<A, I>(
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

function uniqueRefs(refs: ReadonlyArray<string>): ReadonlyArray<string> {
  return [...new Set(refs.filter((ref) => ref.trim().length > 0))].sort((left, right) => left.localeCompare(right));
}

function slugPath(path: string): string {
  return path
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 96);
}

function shortHash(value: string): string {
  return value.replace(/^sha256:/, "").slice(0, 16);
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? studiedContextError(path, "must be a non-empty ref")
    : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return studiedContextError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1
    ? Effect.void
    : studiedContextError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return /^sha256:[a-f0-9]{64}$/.test(value)
    ? Effect.void
    : studiedContextError(path, "must be a sha256 hash ref");
}

function studiedContextError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}

function stableJson(value: unknown): string {
  return JSON.stringify(sortStable(value));
}

function sortStable(value: unknown): unknown {
  if (Array.isArray(value)) {
    return value.map((entry) => sortStable(entry));
  }

  if (value === null || typeof value !== "object") {
    return value;
  }

  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => [key, sortStable(entry)]),
  );
}

function sha256Ref(value: string): string {
  return `sha256:${createHash("sha256").update(value).digest("hex")}`;
}
