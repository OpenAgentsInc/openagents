import { resolve } from "node:path";
import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  buildOpenAgentsRepoStudiedKnowledgeGraph,
  OpenAgentsRepoStudiedKnowledgeGraph,
} from "./openagents-study-graph";
import {
  buildOpenAgentsRepoStudyPacket,
  OpenAgentsRepoStudyPacket,
  OpenAgentsRepoStudyPacketCommit,
  readOpenAgentsRepoCommitHistory,
} from "./openagents-study-packet";
import {
  OpenAgentsRepoStudiedKnowledgeVerificationReport,
  verifyOpenAgentsRepoStudiedKnowledgeClaims,
} from "./openagents-study-verification";
import {
  OpenAgentsStudybenchEvalHarnessReport,
  runOpenAgentsStudybenchEvalHarness,
} from "./openagents-studybench-eval-harness";
import {
  buildOpenAgentsRepoCorpusManifest,
  openAgentsRepoCorpusContentHash,
} from "./repo-corpus-manifest";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

export const OPENAGENTS_REPO_STUDY_ARTIFACT_INDEX_SCHEMA_REF =
  "openagents.repo_study_artifact_index.v0" as const;

export const OPENAGENTS_REPO_STUDY_ARTIFACT_DEFAULT_REPO = "OpenAgentsInc/openagents" as const;

export const OPENAGENTS_REPO_STUDY_ARTIFACT_DEFAULT_COMMIT_HISTORY_LIMIT = 200 as const;

export const OpenAgentsRepoStudyArtifactEvalLift = S.Struct({
  firstDivergenceStepLift: S.Number,
  passRateLiftBps: S.Number,
  rubricScoreLiftBps: S.Number,
});
export type OpenAgentsRepoStudyArtifactEvalLift = typeof OpenAgentsRepoStudyArtifactEvalLift.Type;

export const OpenAgentsRepoStudyArtifactIndex = S.Struct({
  acceptedClaimCount: S.Number,
  commit: S.String,
  commitHistoryLimit: S.Number,
  // Commit-INDEPENDENT digest of the admitted file content. Stable across pure
  // commit drift; changes only when an admitted file actually changes. This is
  // the field the SA-4 standing-freshness "content drift" signal compares.
  corpusContentHash: S.String,
  corpusEntryCount: S.Number,
  corpusManifestHash: S.String,
  corpusManifestRef: S.String,
  correctnessGatePassed: S.Boolean,
  edgeCount: S.Number,
  evalLift: OpenAgentsRepoStudyArtifactEvalLift,
  evalReportHash: S.String,
  evalReportRef: S.String,
  evidenceSpanCount: S.Number,
  graphHash: S.String,
  graphRef: S.String,
  indexHash: S.String,
  indexRef: S.String,
  nodeCount: S.Number,
  packetHash: S.String,
  packetRef: S.String,
  rejectedClaimCount: S.Number,
  repo: S.String,
  schemaRef: S.Literal(OPENAGENTS_REPO_STUDY_ARTIFACT_INDEX_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
  verificationHash: S.String,
  verificationRef: S.String,
});
export type OpenAgentsRepoStudyArtifactIndex = typeof OpenAgentsRepoStudyArtifactIndex.Type;

export interface OpenAgentsRepoStudyArtifact {
  readonly evalReport: OpenAgentsStudybenchEvalHarnessReport;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly index: OpenAgentsRepoStudyArtifactIndex;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly verification: OpenAgentsRepoStudiedKnowledgeVerificationReport;
}

export interface GenerateOpenAgentsRepoStudyArtifactInput {
  readonly backroomRootDir?: string;
  readonly commit?: string;
  readonly commitHistory?: ReadonlyArray<OpenAgentsRepoStudyPacketCommit>;
  readonly commitHistoryLimit?: number;
  readonly indexRef?: string;
  readonly repo?: string;
  readonly rootDir: string;
}

export function generateOpenAgentsRepoStudyArtifact(
  input: GenerateOpenAgentsRepoStudyArtifactInput,
): Effect.Effect<OpenAgentsRepoStudyArtifact, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const rootDir = resolve(input.rootDir);
    const repo = input.repo ?? OPENAGENTS_REPO_STUDY_ARTIFACT_DEFAULT_REPO;
    const commitHistoryLimit = input.commitHistoryLimit ?? OPENAGENTS_REPO_STUDY_ARTIFACT_DEFAULT_COMMIT_HISTORY_LIMIT;

    if (!Number.isInteger(commitHistoryLimit) || commitHistoryLimit < 1) {
      return yield* studyArtifactError("studyArtifact.commitHistoryLimit", "must be a positive integer");
    }

    const fullHistory =
      input.commitHistory === undefined
        ? yield* readOpenAgentsRepoCommitHistory(rootDir)
        : input.commitHistory;

    if (fullHistory.length === 0) {
      return yield* studyArtifactError("studyArtifact.commitHistory", "must include at least one commit");
    }

    const commitHistory = fullHistory.slice(0, commitHistoryLimit);
    const commit = input.commit ?? commitHistory[0]!.commit;
    const backroomRootDir = input.backroomRootDir ?? resolve(rootDir, "..", "backroom");

    const manifest = yield* buildOpenAgentsRepoCorpusManifest({ commit, repo, rootDir });
    const corpusContentHash = openAgentsRepoCorpusContentHash(manifest);

    const packet = yield* buildOpenAgentsRepoStudyPacket({
      backroomRootDir,
      commit,
      commitHistory,
      manifest,
      repo,
      rootDir,
    });
    const graph = yield* buildOpenAgentsRepoStudiedKnowledgeGraph({ packet });
    const verification = yield* verifyOpenAgentsRepoStudiedKnowledgeClaims({ graph, packet, rootDir });
    const { report: evalReport } = yield* runOpenAgentsStudybenchEvalHarness({ graph, packet });

    const index = yield* buildOpenAgentsRepoStudyArtifactIndex({
      commitHistoryLimit,
      corpusContentHash,
      corpusEntryCount: packet.sections.reduce((total, section) => total + section.corpusEntryPaths.length, 0),
      evalReport,
      graph,
      indexRef: input.indexRef,
      packet,
      verification,
    });

    return { evalReport, graph, index, packet, verification };
  });
}

export function buildOpenAgentsRepoStudyArtifactIndex(input: {
  readonly commitHistoryLimit: number;
  readonly corpusContentHash: string;
  readonly corpusEntryCount: number;
  readonly evalReport: OpenAgentsStudybenchEvalHarnessReport;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly indexRef?: string;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly verification: OpenAgentsRepoStudiedKnowledgeVerificationReport;
}): Effect.Effect<OpenAgentsRepoStudyArtifactIndex, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const baseIndex: OpenAgentsRepoStudyArtifactIndex = {
      acceptedClaimCount: input.verification.acceptedCount,
      commit: input.packet.commit,
      commitHistoryLimit: input.commitHistoryLimit,
      corpusContentHash: input.corpusContentHash,
      corpusEntryCount: input.corpusEntryCount,
      corpusManifestHash: input.packet.corpusManifestHash,
      corpusManifestRef: input.packet.corpusManifestRef,
      correctnessGatePassed: input.verification.correctnessGatePassed,
      edgeCount: input.graph.edges.length,
      evalLift: {
        firstDivergenceStepLift: input.evalReport.comparison.firstDivergenceStepLift,
        passRateLiftBps: input.evalReport.comparison.passRateLiftBps,
        rubricScoreLiftBps: input.evalReport.comparison.rubricScoreLiftBps,
      },
      evalReportHash: input.evalReport.reportHash,
      evalReportRef: input.evalReport.reportRef,
      evidenceSpanCount: input.packet.evidenceSpans.length,
      graphHash: input.graph.graphHash,
      graphRef: input.graph.graphRef,
      indexHash: "sha256:pending",
      indexRef: "openagents_repo_study_artifact_index.pending",
      nodeCount: input.graph.nodes.length,
      packetHash: input.packet.packetHash,
      packetRef: input.packet.packetRef,
      rejectedClaimCount: input.verification.rejectedCount,
      repo: input.packet.repo,
      schemaRef: OPENAGENTS_REPO_STUDY_ARTIFACT_INDEX_SCHEMA_REF,
      sourceBoundary: "public_refs_only",
      verificationHash: input.verification.verificationHash,
      verificationRef: input.verification.verificationRef,
    };
    const indexHash = openAgentsRepoStudyArtifactIndexHash(baseIndex);
    const index: OpenAgentsRepoStudyArtifactIndex = {
      ...baseIndex,
      indexHash,
      indexRef: input.indexRef ?? `openagents_repo_study_artifact_index.${shortHash(indexHash)}`,
    };

    return yield* decodeOpenAgentsRepoStudyArtifactIndex(index);
  });
}

export function decodeOpenAgentsRepoStudyArtifactIndex(
  value: unknown,
): Effect.Effect<OpenAgentsRepoStudyArtifactIndex, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studyArtifactIndex");
    const index = yield* decodeStudyArtifactSchema(OpenAgentsRepoStudyArtifactIndex, value, "studyArtifactIndex");
    yield* validateOpenAgentsRepoStudyArtifactIndex(index);
    return index;
  });
}

export function openAgentsRepoStudyArtifactIndexHash(index: OpenAgentsRepoStudyArtifactIndex): string {
  const { indexHash: _indexHash, indexRef: _indexRef, ...stable } = index;
  return sha256Ref(stableJson(stable));
}

function validateOpenAgentsRepoStudyArtifactIndex(
  index: OpenAgentsRepoStudyArtifactIndex,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(index.repo, "studyArtifactIndex.repo");
    yield* requireNonEmpty(index.commit, "studyArtifactIndex.commit");
    yield* requireNonEmpty(index.indexRef, "studyArtifactIndex.indexRef");
    yield* requireSha256(index.indexHash, "studyArtifactIndex.indexHash");
    yield* requireSha256(index.packetHash, "studyArtifactIndex.packetHash");
    yield* requireSha256(index.graphHash, "studyArtifactIndex.graphHash");
    yield* requireSha256(index.corpusManifestHash, "studyArtifactIndex.corpusManifestHash");
    yield* requireSha256(index.corpusContentHash, "studyArtifactIndex.corpusContentHash");
    yield* requireSha256(index.verificationHash, "studyArtifactIndex.verificationHash");
    yield* requireSha256(index.evalReportHash, "studyArtifactIndex.evalReportHash");

    if (index.indexHash !== openAgentsRepoStudyArtifactIndexHash(index)) {
      return yield* studyArtifactError("studyArtifactIndex.indexHash", "must match deterministic index content hash");
    }

    if (!Number.isInteger(index.commitHistoryLimit) || index.commitHistoryLimit < 1) {
      return yield* studyArtifactError("studyArtifactIndex.commitHistoryLimit", "must be a positive integer");
    }

    for (const [field, count] of [
      ["acceptedClaimCount", index.acceptedClaimCount],
      ["corpusEntryCount", index.corpusEntryCount],
      ["edgeCount", index.edgeCount],
      ["evidenceSpanCount", index.evidenceSpanCount],
      ["nodeCount", index.nodeCount],
      ["rejectedClaimCount", index.rejectedClaimCount],
    ] as const) {
      if (!Number.isInteger(count) || count < 0) {
        return yield* studyArtifactError(`studyArtifactIndex.${field}`, "must be a non-negative integer");
      }
    }
  });
}

function decodeStudyArtifactSchema<A, I>(
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
  return value.trim().length === 0 ? studyArtifactError(path, "must be a non-empty string") : Effect.void;
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : studyArtifactError(path, "must be a sha256 ref");
}

function studyArtifactError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
