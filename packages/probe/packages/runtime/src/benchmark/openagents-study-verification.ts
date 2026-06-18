import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  buildOpenAgentsRepoCorpusManifest,
  extractOpenAgentsRepoCorpusEvidenceSpan,
  OpenAgentsRepoCorpusManifest,
} from "./repo-corpus-manifest";
import {
  buildOpenAgentsRepoStudiedKnowledgeGraph,
  decodeOpenAgentsRepoStudiedKnowledgeGraph,
  OpenAgentsRepoStudiedKnowledgeEdgeKind,
  OpenAgentsRepoStudiedKnowledgeGraph,
  OpenAgentsRepoStudiedKnowledgeIssueRef,
  openAgentsRepoStudiedKnowledgeEdgeHash,
} from "./openagents-study-graph";
import {
  OpenAgentsRepoStudyPacket,
  openAgentsRepoStudyPacketHash,
} from "./openagents-study-packet";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

export const OPENAGENTS_REPO_STUDIED_KNOWLEDGE_VERIFICATION_SCHEMA_REF =
  "openagents.repo_studied_knowledge_verification.v0" as const;

export const OpenAgentsRepoStudiedKnowledgeVerificationClaimKind = S.Literals([
  "edge",
  "evidence_span",
  "validator_review_remainder",
]);
export type OpenAgentsRepoStudiedKnowledgeVerificationClaimKind =
  typeof OpenAgentsRepoStudiedKnowledgeVerificationClaimKind.Type;

export const OpenAgentsRepoStudiedKnowledgeVerificationStatus = S.Literals([
  "accepted",
  "rejected",
  "needs_validator_review",
]);
export type OpenAgentsRepoStudiedKnowledgeVerificationStatus =
  typeof OpenAgentsRepoStudiedKnowledgeVerificationStatus.Type;

export const OpenAgentsRepoStudiedKnowledgeVerificationClaim = S.Struct({
  claimRef: S.String,
  claimedEdgeKind: S.optional(OpenAgentsRepoStudiedKnowledgeEdgeKind),
  claimedFromNodeRef: S.optional(S.String),
  claimedHash: S.optional(S.String),
  claimedToNodeRef: S.optional(S.String),
  edgeRef: S.optional(S.String),
  endLine: S.optional(S.Number),
  kind: OpenAgentsRepoStudiedKnowledgeVerificationClaimKind,
  path: S.optional(S.String),
  spanHash: S.optional(S.String),
  spanId: S.optional(S.String),
  startLine: S.optional(S.Number),
  validatorReviewRef: S.optional(S.String),
});
export type OpenAgentsRepoStudiedKnowledgeVerificationClaim =
  typeof OpenAgentsRepoStudiedKnowledgeVerificationClaim.Type;

export const OpenAgentsRepoStudiedKnowledgeVerificationResult = S.Struct({
  blockerRefs: S.Array(S.String),
  claimRef: S.String,
  claimedHash: S.optional(S.String),
  derivedHash: S.optional(S.String),
  kind: OpenAgentsRepoStudiedKnowledgeVerificationClaimKind,
  sourceRefs: S.Array(S.String),
  status: OpenAgentsRepoStudiedKnowledgeVerificationStatus,
  validatorReviewRefs: S.Array(S.String),
});
export type OpenAgentsRepoStudiedKnowledgeVerificationResult =
  typeof OpenAgentsRepoStudiedKnowledgeVerificationResult.Type;

export const OpenAgentsRepoStudiedKnowledgeVerificationReport = S.Struct({
  acceptedCount: S.Number,
  commit: S.String,
  correctnessGatePassed: S.Boolean,
  generatedAt: S.String,
  graphHash: S.String,
  graphRef: S.String,
  packetHash: S.String,
  packetRef: S.String,
  rejectedCount: S.Number,
  repo: S.String,
  results: S.Array(OpenAgentsRepoStudiedKnowledgeVerificationResult),
  schemaRef: S.Literal(OPENAGENTS_REPO_STUDIED_KNOWLEDGE_VERIFICATION_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
  validatorReviewRequired: S.Boolean,
  verificationHash: S.String,
  verificationRef: S.String,
});
export type OpenAgentsRepoStudiedKnowledgeVerificationReport =
  typeof OpenAgentsRepoStudiedKnowledgeVerificationReport.Type;

export interface VerifyOpenAgentsRepoStudiedKnowledgeClaimsInput {
  readonly claims?: ReadonlyArray<OpenAgentsRepoStudiedKnowledgeVerificationClaim>;
  readonly editSitePaths?: ReadonlyArray<string>;
  readonly generatedAt?: string;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly issueRefs?: ReadonlyArray<OpenAgentsRepoStudiedKnowledgeIssueRef>;
  readonly manifest?: OpenAgentsRepoCorpusManifest;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly rootDir?: string;
  readonly trackingIssueNumber?: number;
  readonly verificationRef?: string;
}

export function verifyOpenAgentsRepoStudiedKnowledgeClaims(
  input: VerifyOpenAgentsRepoStudiedKnowledgeClaimsInput,
): Effect.Effect<
  OpenAgentsRepoStudiedKnowledgeVerificationReport,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const packetHash = openAgentsRepoStudyPacketHash(input.packet);

    if (input.packet.packetHash !== packetHash) {
      return yield* studyVerificationError("studyVerification.packetHash", "packet hash must match packet content");
    }

    const graph = yield* decodeOpenAgentsRepoStudiedKnowledgeGraph(input.graph);

    if (graph.packetHash !== input.packet.packetHash || graph.packetRef !== input.packet.packetRef) {
      return yield* studyVerificationError("studyVerification.graph", "graph must be built from the supplied packet");
    }

    const derivedGraph = yield* buildOpenAgentsRepoStudiedKnowledgeGraph({
      editSitePaths: input.editSitePaths,
      generatedAt: input.generatedAt,
      issueRefs: input.issueRefs,
      packet: input.packet,
      trackingIssueNumber: input.trackingIssueNumber,
    });

    if (graph.graphHash !== derivedGraph.graphHash) {
      return yield* studyVerificationError("studyVerification.graphHash", "graph must match derived graph replay");
    }

    const manifest = yield* resolveReplayManifest(input);
    const claims = input.claims ?? defaultVerificationClaims(input.packet, graph);
    const results: OpenAgentsRepoStudiedKnowledgeVerificationResult[] = [];

    for (const claim of claims) {
      results.push(yield* verifyClaim({
        claim,
        derivedGraph,
        manifest,
        packet: input.packet,
        rootDir: input.rootDir,
      }));
    }

    const acceptedCount = results.filter((result) => result.status === "accepted").length;
    const rejectedCount = results.filter((result) => result.status === "rejected").length;
    const validatorReviewRequired = results.some((result) => result.status === "needs_validator_review");
    const baseReport: OpenAgentsRepoStudiedKnowledgeVerificationReport = {
      acceptedCount,
      commit: input.packet.commit,
      correctnessGatePassed: rejectedCount === 0 && !validatorReviewRequired,
      generatedAt: input.generatedAt ?? "generated_at.withheld_for_stable_study_verification_hash",
      graphHash: graph.graphHash,
      graphRef: graph.graphRef,
      packetHash: input.packet.packetHash,
      packetRef: input.packet.packetRef,
      rejectedCount,
      repo: input.packet.repo,
      results: results.sort((left, right) => left.claimRef.localeCompare(right.claimRef)),
      schemaRef: OPENAGENTS_REPO_STUDIED_KNOWLEDGE_VERIFICATION_SCHEMA_REF,
      sourceBoundary: "public_refs_only",
      validatorReviewRequired,
      verificationHash: "sha256:pending",
      verificationRef: "openagents_repo_studied_knowledge_verification.pending",
    };
    const verificationHash = openAgentsRepoStudiedKnowledgeVerificationHash(baseReport);
    const report: OpenAgentsRepoStudiedKnowledgeVerificationReport = {
      ...baseReport,
      verificationHash,
      verificationRef: input.verificationRef ?? `openagents_repo_studied_knowledge_verification.${shortHash(verificationHash)}`,
    };

    return yield* decodeOpenAgentsRepoStudiedKnowledgeVerificationReport(report);
  });
}

export function decodeOpenAgentsRepoStudiedKnowledgeVerificationReport(
  value: unknown,
): Effect.Effect<
  OpenAgentsRepoStudiedKnowledgeVerificationReport,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studyVerification");
    const report = yield* decodeStudyVerificationSchema(
      OpenAgentsRepoStudiedKnowledgeVerificationReport,
      value,
      "studyVerification",
    );
    yield* validateOpenAgentsRepoStudiedKnowledgeVerificationReport(report);
    return report;
  });
}

export function openAgentsRepoStudiedKnowledgeVerificationHash(
  report: OpenAgentsRepoStudiedKnowledgeVerificationReport,
): string {
  const {
    generatedAt: _generatedAt,
    verificationHash: _verificationHash,
    verificationRef: _verificationRef,
    ...stable
  } = report;
  return sha256Ref(stableJson(stable));
}

function verifyClaim(input: {
  readonly claim: OpenAgentsRepoStudiedKnowledgeVerificationClaim;
  readonly derivedGraph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly manifest: OpenAgentsRepoCorpusManifest | undefined;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly rootDir: string | undefined;
}): Effect.Effect<OpenAgentsRepoStudiedKnowledgeVerificationResult, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  if (input.claim.kind === "edge") {
    return verifyEdgeClaim(input.claim, input.derivedGraph);
  }

  if (input.claim.kind === "evidence_span") {
    return verifyEvidenceSpanClaim(input);
  }

  return Effect.succeed({
    blockerRefs: ["blocker.public.study_verification.validator_review_required"],
    claimRef: input.claim.claimRef,
    kind: input.claim.kind,
    sourceRefs: [input.packet.packetRef],
    status: "needs_validator_review",
    validatorReviewRefs: [validatorReviewRef(input.claim)],
  });
}

function verifyEdgeClaim(
  claim: OpenAgentsRepoStudiedKnowledgeVerificationClaim,
  derivedGraph: OpenAgentsRepoStudiedKnowledgeGraph,
): Effect.Effect<OpenAgentsRepoStudiedKnowledgeVerificationResult, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    const edge = findClaimedEdge(claim, derivedGraph);

    if (edge === undefined) {
      return rejectedResult({
        blockerRefs: ["blocker.public.study_verification.edge_not_derived"],
        claim,
        claimedHash: claim.claimedHash,
        sourceRefs: [derivedGraph.graphRef],
      });
    }

    const claimedBody = {
      fromNodeRef: claim.claimedFromNodeRef ?? edge.fromNodeRef,
      kind: claim.claimedEdgeKind ?? edge.kind,
      rationaleRef: edge.rationaleRef,
      sourceEvidenceNodeRefs: edge.sourceEvidenceNodeRefs,
      toNodeRef: claim.claimedToNodeRef ?? edge.toNodeRef,
    };
    const claimedHash = claim.claimedHash ?? openAgentsRepoStudiedKnowledgeEdgeHash(claimedBody);
    const match =
      claimedHash === edge.edgeHash &&
      claimedBody.fromNodeRef === edge.fromNodeRef &&
      claimedBody.kind === edge.kind &&
      claimedBody.toNodeRef === edge.toNodeRef;

    return {
      blockerRefs: match ? [] : ["blocker.public.study_verification.edge_replay_mismatch"],
      claimRef: claim.claimRef,
      claimedHash,
      derivedHash: edge.edgeHash,
      kind: claim.kind,
      sourceRefs: [derivedGraph.graphRef, edge.ref, ...edge.sourceEvidenceNodeRefs].sort((left, right) =>
        left.localeCompare(right),
      ),
      status: match ? "accepted" : "rejected",
      validatorReviewRefs: [],
    };
  });
}

function verifyEvidenceSpanClaim(input: {
  readonly claim: OpenAgentsRepoStudiedKnowledgeVerificationClaim;
  readonly manifest: OpenAgentsRepoCorpusManifest | undefined;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly rootDir: string | undefined;
}): Effect.Effect<OpenAgentsRepoStudiedKnowledgeVerificationResult, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const span = findClaimedSpan(input.claim, input.packet);

    if (span === undefined) {
      return rejectedResult({
        blockerRefs: ["blocker.public.study_verification.span_not_in_packet"],
        claim: input.claim,
        claimedHash: input.claim.claimedHash ?? input.claim.spanHash,
        sourceRefs: [input.packet.packetRef],
      });
    }

    if (input.rootDir === undefined || input.manifest === undefined) {
      return {
        blockerRefs: ["blocker.public.study_verification.source_replay_unavailable"],
        claimRef: input.claim.claimRef,
        claimedHash: input.claim.claimedHash ?? span.spanHash,
        derivedHash: span.spanHash,
        kind: input.claim.kind,
        sourceRefs: [input.packet.packetRef, span.spanHash],
        status: "needs_validator_review",
        validatorReviewRefs: [validatorReviewRef(input.claim)],
      };
    }

    if (input.manifest.manifestHash !== input.packet.corpusManifestHash) {
      return rejectedResult({
        blockerRefs: ["blocker.public.study_verification.manifest_replay_mismatch"],
        claim: input.claim,
        claimedHash: input.claim.claimedHash ?? span.spanHash,
        derivedHash: input.manifest.manifestHash,
        sourceRefs: [input.packet.packetRef, input.packet.corpusManifestRef],
      });
    }

    const replayed = yield* extractOpenAgentsRepoCorpusEvidenceSpan({
      endLine: input.claim.endLine ?? span.evidence.end_line,
      manifest: input.manifest,
      path: input.claim.path ?? span.evidence.path,
      rootDir: input.rootDir,
      spanId: input.claim.spanId ?? span.evidence.span_id,
      startLine: input.claim.startLine ?? span.evidence.start_line,
    });
    const claimedHash = input.claim.claimedHash ?? input.claim.spanHash ?? span.spanHash;
    const match =
      claimedHash === span.spanHash &&
      replayed.spanHash === span.spanHash &&
      replayed.evidence.path === span.evidence.path &&
      replayed.evidence.start_line === span.evidence.start_line &&
      replayed.evidence.end_line === span.evidence.end_line;

    return {
      blockerRefs: match ? [] : ["blocker.public.study_verification.span_replay_mismatch"],
      claimRef: input.claim.claimRef,
      claimedHash,
      derivedHash: replayed.spanHash,
      kind: input.claim.kind,
      sourceRefs: [input.packet.packetRef, input.packet.corpusManifestRef, span.spanHash],
      status: match ? "accepted" : "rejected",
      validatorReviewRefs: [],
    };
  });
}

function resolveReplayManifest(input: VerifyOpenAgentsRepoStudiedKnowledgeClaimsInput): Effect.Effect<
  OpenAgentsRepoCorpusManifest | undefined,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  if (input.manifest !== undefined) {
    return Effect.succeed(input.manifest);
  }

  if (input.rootDir === undefined) {
    return Effect.succeed(undefined);
  }

  return buildOpenAgentsRepoCorpusManifest({
    commit: input.packet.commit,
    repo: input.packet.repo,
    rootDir: input.rootDir,
  });
}

function defaultVerificationClaims(
  packet: OpenAgentsRepoStudyPacket,
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
): ReadonlyArray<OpenAgentsRepoStudiedKnowledgeVerificationClaim> {
  const edgeClaims = graph.edges.map((edge) => ({
    claimRef: `claim.public.study_graph.edge.${shortHash(edge.edgeHash)}`,
    edgeRef: edge.ref,
    kind: "edge" as const,
  }));
  const spanClaims = packet.evidenceSpans.map((span) => ({
    claimRef: `claim.public.study_graph.span.${shortHash(span.spanHash)}`,
    kind: "evidence_span" as const,
    spanHash: span.spanHash,
  }));

  return [...edgeClaims, ...spanClaims];
}

function findClaimedEdge(
  claim: OpenAgentsRepoStudiedKnowledgeVerificationClaim,
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
) {
  return graph.edges.find((edge) =>
    claim.edgeRef !== undefined ? edge.ref === claim.edgeRef : edge.edgeHash === claim.claimedHash,
  );
}

function findClaimedSpan(
  claim: OpenAgentsRepoStudiedKnowledgeVerificationClaim,
  packet: OpenAgentsRepoStudyPacket,
) {
  return packet.evidenceSpans.find((span) => {
    if (claim.spanHash !== undefined) {
      return span.spanHash === claim.spanHash;
    }

    if (claim.spanId !== undefined) {
      return span.evidence.span_id === claim.spanId;
    }

    return claim.path !== undefined &&
      span.evidence.path === claim.path &&
      span.evidence.start_line === claim.startLine &&
      span.evidence.end_line === claim.endLine;
  });
}

function rejectedResult(input: {
  readonly blockerRefs: ReadonlyArray<string>;
  readonly claim: OpenAgentsRepoStudiedKnowledgeVerificationClaim;
  readonly claimedHash?: string;
  readonly derivedHash?: string;
  readonly sourceRefs: ReadonlyArray<string>;
}): OpenAgentsRepoStudiedKnowledgeVerificationResult {
  return {
    blockerRefs: [...input.blockerRefs],
    claimRef: input.claim.claimRef,
    claimedHash: input.claimedHash,
    derivedHash: input.derivedHash,
    kind: input.claim.kind,
    sourceRefs: [...input.sourceRefs],
    status: "rejected",
    validatorReviewRefs: [],
  };
}

function validatorReviewRef(claim: OpenAgentsRepoStudiedKnowledgeVerificationClaim): string {
  return claim.validatorReviewRef ?? `validator_review.public.study_verification.${shortHash(sha256Ref(claim.claimRef))}`;
}

function validateOpenAgentsRepoStudiedKnowledgeVerificationReport(
  report: OpenAgentsRepoStudiedKnowledgeVerificationReport,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(report.repo, "studyVerification.repo");
    yield* requireNonEmpty(report.commit, "studyVerification.commit");
    yield* requireNonEmpty(report.packetRef, "studyVerification.packetRef");
    yield* requireNonEmpty(report.graphRef, "studyVerification.graphRef");
    yield* requireNonEmpty(report.verificationRef, "studyVerification.verificationRef");
    yield* requireSha256(report.packetHash, "studyVerification.packetHash");
    yield* requireSha256(report.graphHash, "studyVerification.graphHash");
    yield* requireSha256(report.verificationHash, "studyVerification.verificationHash");

    if (report.verificationHash !== openAgentsRepoStudiedKnowledgeVerificationHash(report)) {
      return yield* studyVerificationError(
        "studyVerification.verificationHash",
        "must match deterministic verification content hash",
      );
    }

    if (report.results.length === 0) {
      return yield* studyVerificationError("studyVerification.results", "must include at least one claim result");
    }

    const acceptedCount = report.results.filter((result) => result.status === "accepted").length;
    const rejectedCount = report.results.filter((result) => result.status === "rejected").length;
    const validatorReviewRequired = report.results.some((result) => result.status === "needs_validator_review");

    if (report.acceptedCount !== acceptedCount) {
      return yield* studyVerificationError("studyVerification.acceptedCount", "must match accepted results");
    }

    if (report.rejectedCount !== rejectedCount) {
      return yield* studyVerificationError("studyVerification.rejectedCount", "must match rejected results");
    }

    if (report.validatorReviewRequired !== validatorReviewRequired) {
      return yield* studyVerificationError(
        "studyVerification.validatorReviewRequired",
        "must match validator-review results",
      );
    }

    if (report.correctnessGatePassed !== (rejectedCount === 0 && !validatorReviewRequired)) {
      return yield* studyVerificationError(
        "studyVerification.correctnessGatePassed",
        "must require zero rejected results and no pending validator review",
      );
    }

    for (const [index, result] of report.results.entries()) {
      const path = `studyVerification.results[${index}]`;
      yield* requireNonEmpty(result.claimRef, `${path}.claimRef`);
      yield* requireNonEmptyRefs(result.sourceRefs, `${path}.sourceRefs`);

      if (result.claimedHash !== undefined) {
        yield* requireSha256(result.claimedHash, `${path}.claimedHash`);
      }

      if (result.derivedHash !== undefined) {
        yield* requireSha256(result.derivedHash, `${path}.derivedHash`);
      }

      if (result.status === "accepted" && result.blockerRefs.length !== 0) {
        return yield* studyVerificationError(`${path}.blockerRefs`, "accepted results must not carry blockers");
      }

      if (result.status === "rejected" && result.blockerRefs.length === 0) {
        return yield* studyVerificationError(`${path}.blockerRefs`, "rejected results must carry blockers");
      }

      if (result.status === "needs_validator_review" && result.validatorReviewRefs.length === 0) {
        return yield* studyVerificationError(
          `${path}.validatorReviewRefs`,
          "validator-review results must carry a validator review ref",
        );
      }
    }
  });
}

function decodeStudyVerificationSchema<A, I>(
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
  return value.trim().length === 0 ? studyVerificationError(path, "must be a non-empty string") : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return studyVerificationError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1 ? Effect.void : studyVerificationError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : studyVerificationError(path, "must be a sha256 ref");
}

function studyVerificationError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
