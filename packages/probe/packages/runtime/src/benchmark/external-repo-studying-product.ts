import { resolve } from "node:path";
import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  buildOpenAgentsAutopilotCoderStudiedContext,
  type OpenAgentsAutopilotCoderStudiedContext,
} from "./openagents-autopilot-coder-studied-context";
import {
  buildOpenAgentsRepoStudiedKnowledgeGraph,
  type OpenAgentsRepoStudiedKnowledgeGraph,
  type OpenAgentsRepoStudiedKnowledgeIssueRef,
} from "./openagents-study-graph";
import {
  buildOpenAgentsRepoStudyPacket,
  type OpenAgentsRepoStudyPacket,
  type OpenAgentsRepoStudyPacketCommit,
  type OpenAgentsRepoStudyPacketRationaleSource,
  type OpenAgentsRepoStudyPacketSection,
  type OpenAgentsRepoStudyPacketSectionKind,
} from "./openagents-study-packet";
import {
  verifyOpenAgentsRepoStudiedKnowledgeClaims,
  type OpenAgentsRepoStudiedKnowledgeVerificationReport,
} from "./openagents-study-verification";
import {
  runOpenAgentsStudybenchEvalHarness,
  type OpenAgentsStudybenchEvalHarnessReport,
} from "./openagents-studybench-eval-harness";
import {
  buildOpenAgentsRepoCorpusManifest,
  type OpenAgentsRepoCorpusManifest,
} from "./repo-corpus-manifest";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

export const OPENAGENTS_EXTERNAL_REPO_STUDY_PRODUCT_SURFACE_SCHEMA_REF =
  "openagents.external_repo_study_product_surface.v0" as const;

export const OpenAgentsExternalRepoStudyProductState = S.Literals([
  "pilot_ready",
  "blocked",
]);
export type OpenAgentsExternalRepoStudyProductState =
  typeof OpenAgentsExternalRepoStudyProductState.Type;

export const OpenAgentsExternalRepoStudyProductSurface = S.Struct({
  blockerRefs: S.Array(S.String),
  commit: S.String,
  contextHash: S.String,
  contextPackRef: S.String,
  customerPublicClaimAllowed: S.Literal(false),
  evalReportHash: S.String,
  evalReportRef: S.String,
  evidenceRefs: S.Array(S.String),
  generatedAt: S.String,
  graphHash: S.String,
  graphRef: S.String,
  manifestHash: S.String,
  manifestRef: S.String,
  marketplacePackageAllowed: S.Literal(false),
  packetHash: S.String,
  packetRef: S.String,
  payoutEligible: S.Literal(false),
  productSurfaceHash: S.String,
  productSurfaceRef: S.String,
  repo: S.String,
  safeCopy: S.String,
  schemaRef: S.Literal(OPENAGENTS_EXTERNAL_REPO_STUDY_PRODUCT_SURFACE_SCHEMA_REF),
  sourceBoundary: S.Literal("public_refs_only"),
  state: OpenAgentsExternalRepoStudyProductState,
  studiedBeatsBaseline: S.Boolean,
  unsafeCopyRefs: S.Array(S.String),
  verificationHash: S.String,
  verificationRef: S.String,
});
export type OpenAgentsExternalRepoStudyProductSurface =
  typeof OpenAgentsExternalRepoStudyProductSurface.Type;

export interface BuildOpenAgentsExternalRepoStudyPilotInput {
  readonly commit: string;
  readonly commitHistory?: ReadonlyArray<OpenAgentsRepoStudyPacketCommit>;
  readonly editSitePath: string;
  readonly evidenceSpanPaths?: ReadonlyArray<string>;
  readonly generatedAt?: string;
  readonly repo: string;
  readonly rootDir: string;
}

export interface BuildOpenAgentsExternalRepoStudyPilotResult {
  readonly coderContext: OpenAgentsAutopilotCoderStudiedContext;
  readonly evalReport: OpenAgentsStudybenchEvalHarnessReport;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly manifest: OpenAgentsRepoCorpusManifest;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly productSurface: OpenAgentsExternalRepoStudyProductSurface;
  readonly verification: OpenAgentsRepoStudiedKnowledgeVerificationReport;
}

type SectionPlan = Readonly<{
  description: string;
  kind: OpenAgentsRepoStudyPacketSectionKind;
  refSuffix: string;
  selectors: ReadonlyArray<string>;
  sourceAuthorityRefs: ReadonlyArray<string>;
}>;

const EXTERNAL_TRACKING_ISSUE: OpenAgentsRepoStudiedKnowledgeIssueRef = {
  issueNumber: 5320,
  issueRef: "github_issue.OpenAgentsInc.openagents.5320",
  label: "S7 external repo studying product issue",
};

export function buildOpenAgentsExternalRepoStudyPilot(
  input: BuildOpenAgentsExternalRepoStudyPilotInput,
): Effect.Effect<
  BuildOpenAgentsExternalRepoStudyPilotResult,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    if (input.repo === "OpenAgentsInc/openagents") {
      return yield* externalStudyError("externalRepoStudy.repo", "pilot repo must not be OpenAgentsInc/openagents");
    }

    const rootDir = resolve(input.rootDir);
    const manifest = yield* buildOpenAgentsRepoCorpusManifest({
      commit: input.commit,
      defaultSourceAuthorityRefs: ["authority.external_repo_study.public_source"],
      generatedAt: input.generatedAt,
      manifestRef: `external_repo_corpus_manifest.${slugRepo(input.repo)}.${shortHash(input.commit)}`,
      repo: input.repo,
      rootDir,
    });

    if (!manifest.entries.some((entry) => entry.path === input.editSitePath)) {
      return yield* externalStudyError("externalRepoStudy.editSitePath", "edit site must be admitted by the corpus manifest");
    }

    const sections = buildExternalStudySections(manifest, input.editSitePath, input.repo);
    const rationaleSources = buildExternalRationaleSources(manifest, input);
    const evidenceSpanPaths = input.evidenceSpanPaths ?? defaultExternalEvidenceSpanPaths({
      editSitePath: input.editSitePath,
      manifest,
      rationaleSources,
    });
    const packet = yield* buildOpenAgentsRepoStudyPacket({
      commit: input.commit,
      commitHistory: input.commitHistory,
      evidenceSpanPaths,
      generatedAt: input.generatedAt,
      manifest,
      packetRef: `external_repo_study_packet.${slugRepo(input.repo)}.${shortHash(manifest.manifestHash)}`,
      rationaleSources,
      repo: input.repo,
      rootDir,
      sections,
    });
    const graph = yield* buildOpenAgentsRepoStudiedKnowledgeGraph({
      editSitePaths: [input.editSitePath],
      generatedAt: input.generatedAt,
      graphRef: `external_repo_studied_knowledge_graph.${slugRepo(input.repo)}.${shortHash(packet.packetHash)}`,
      issueRefs: [EXTERNAL_TRACKING_ISSUE],
      packet,
      trackingIssueNumber: EXTERNAL_TRACKING_ISSUE.issueNumber,
    });
    const verification = yield* verifyOpenAgentsRepoStudiedKnowledgeClaims({
      editSitePaths: [input.editSitePath],
      generatedAt: input.generatedAt,
      graph,
      issueRefs: [EXTERNAL_TRACKING_ISSUE],
      manifest,
      packet,
      rootDir,
      trackingIssueNumber: EXTERNAL_TRACKING_ISSUE.issueNumber,
      verificationRef: `external_repo_study_verification.${slugRepo(input.repo)}.${shortHash(graph.graphHash)}`,
    });
    const { report: evalReport } = yield* runOpenAgentsStudybenchEvalHarness({
      generatedAt: input.generatedAt,
      graph,
      maxExams: 1,
      packet,
      reportRef: `external_repo_studybench_eval.${slugRepo(input.repo)}.${shortHash(graph.graphHash)}`,
    });
    const coderContext = yield* buildOpenAgentsAutopilotCoderStudiedContext({
      editSitePath: input.editSitePath,
      graph,
      packet,
    });
    const productSurface = yield* buildExternalProductSurface({
      coderContext,
      commit: input.commit,
      evalReport,
      generatedAt: input.generatedAt,
      graph,
      manifest,
      packet,
      repo: input.repo,
      verification,
    });

    return {
      coderContext,
      evalReport,
      graph,
      manifest,
      packet,
      productSurface,
      verification,
    };
  });
}

export function decodeOpenAgentsExternalRepoStudyProductSurface(
  value: unknown,
): Effect.Effect<
  OpenAgentsExternalRepoStudyProductSurface,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "externalRepoStudyProductSurface");
    const surface = yield* S.decodeUnknownEffect(OpenAgentsExternalRepoStudyProductSurface)(value).pipe(
      Effect.mapError((error) =>
        new ProbeBenchmarkContractError({
          path: "externalRepoStudyProductSurface",
          reason: String(error),
        }),
      ),
    );
    yield* validateExternalProductSurface(surface);
    return surface;
  });
}

export function openAgentsExternalRepoStudyProductSurfaceHash(
  surface: OpenAgentsExternalRepoStudyProductSurface,
): string {
  const {
    generatedAt: _generatedAt,
    productSurfaceHash: _productSurfaceHash,
    productSurfaceRef: _productSurfaceRef,
    ...stable
  } = surface;
  return sha256Ref(stableJson(stable));
}

function buildExternalProductSurface(input: {
  readonly coderContext: OpenAgentsAutopilotCoderStudiedContext;
  readonly commit: string;
  readonly evalReport: OpenAgentsStudybenchEvalHarnessReport;
  readonly generatedAt: string | undefined;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly manifest: OpenAgentsRepoCorpusManifest;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly repo: string;
  readonly verification: OpenAgentsRepoStudiedKnowledgeVerificationReport;
}): Effect.Effect<OpenAgentsExternalRepoStudyProductSurface, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  const blockerRefs = [
    "blocker.external_repo_study.customer_private_policy_required",
    "blocker.external_repo_study.customer_review_required",
    "blocker.external_repo_study.marketplace_metering_required",
    "blocker.external_repo_study.pricing_required",
    "blocker.external_repo_study.payout_settlement_required",
  ];
  const baseSurface: OpenAgentsExternalRepoStudyProductSurface = {
    blockerRefs,
    commit: input.commit,
    contextHash: input.coderContext.contextHash,
    contextPackRef: input.coderContext.contextPackRef,
    customerPublicClaimAllowed: false,
    evalReportHash: input.evalReport.reportHash,
    evalReportRef: input.evalReport.reportRef,
    evidenceRefs: [
      input.manifest.manifestRef,
      input.packet.packetRef,
      input.graph.graphRef,
      input.verification.verificationRef,
      input.evalReport.reportRef,
      input.coderContext.contextPackRef,
      "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md#phase-6",
    ],
    generatedAt: input.generatedAt ?? "generated_at.withheld_for_stable_external_repo_study_product_hash",
    graphHash: input.graph.graphHash,
    graphRef: input.graph.graphRef,
    manifestHash: input.manifest.manifestHash,
    manifestRef: input.manifest.manifestRef,
    marketplacePackageAllowed: false,
    packetHash: input.packet.packetHash,
    packetRef: input.packet.packetRef,
    payoutEligible: false,
    productSurfaceHash: "sha256:pending",
    productSurfaceRef: "external_repo_study_product_surface.pending",
    repo: input.repo,
    safeCopy:
      "External repo studying is exposed as a refs-only pilot surface with corpus, packet, verification, eval, and coder-context refs. Customer-private ingestion, marketplace packaging, pricing, payout, settlement, and green public claims remain blocked.",
    schemaRef: OPENAGENTS_EXTERNAL_REPO_STUDY_PRODUCT_SURFACE_SCHEMA_REF,
    sourceBoundary: "public_refs_only",
    state: input.verification.correctnessGatePassed && input.evalReport.comparison.studiedBeatsBaseline
      ? "pilot_ready"
      : "blocked",
    studiedBeatsBaseline: input.evalReport.comparison.studiedBeatsBaseline,
    unsafeCopyRefs: [
      "blocked_claim.customer_repo_studying_live",
      "blocked_claim.trained_repo_expert",
      "blocked_claim.study_packet_marketplace_package",
      "blocked_claim.machine_studying_payout_eligible",
    ],
    verificationHash: input.verification.verificationHash,
    verificationRef: input.verification.verificationRef,
  };
  const productSurfaceHash = openAgentsExternalRepoStudyProductSurfaceHash(baseSurface);

  return decodeOpenAgentsExternalRepoStudyProductSurface({
    ...baseSurface,
    productSurfaceHash,
    productSurfaceRef: `external_repo_study_product_surface.${slugRepo(input.repo)}.${shortHash(productSurfaceHash)}`,
  });
}

function buildExternalStudySections(
  manifest: OpenAgentsRepoCorpusManifest,
  editSitePath: string,
  repo: string,
): ReadonlyArray<OpenAgentsRepoStudyPacketSection> {
  const fallback = fallbackPaths(manifest, editSitePath);
  return sectionPlans(editSitePath).map((plan) => {
    const selected = selectManifestPaths(manifest, plan.selectors);
    return {
      corpusEntryPaths: selected.length === 0 ? fallback : selected,
      description: plan.description,
      kind: plan.kind,
      ref: `repo_study_section.external.${slugRepo(repo)}.${plan.refSuffix}.v1`,
      sourceAuthorityRefs: plan.sourceAuthorityRefs,
    };
  });
}

function sectionPlans(editSitePath: string): ReadonlyArray<SectionPlan> {
  return [
    {
      description: "External repo source map covering docs, policy, tests, and the selected edit site.",
      kind: "source_map",
      refSuffix: "source_map",
      selectors: ["README.md", "AGENTS.md", "docs/", "src/", editSitePath],
      sourceAuthorityRefs: ["authority.external_repo_study.source_map"],
    },
    {
      description: "External repo invariant map for policy and authority boundaries.",
      kind: "invariant_map",
      refSuffix: "invariant_map",
      selectors: ["INVARIANTS.md", "AGENTS.md", "docs/policy.md"],
      sourceAuthorityRefs: ["authority.external_repo_study.invariants"],
    },
    {
      description: "External repo typed-ref glossary covering package, test, and API refs.",
      kind: "typed_ref_glossary",
      refSuffix: "typed_ref_glossary",
      selectors: ["package.json", "README.md", "docs/architecture.md", "src/"],
      sourceAuthorityRefs: ["authority.external_repo_study.typed_refs"],
    },
    {
      description: "External repo trap catalog for rejected approaches and unsafe shortcuts.",
      kind: "trap_catalog",
      refSuffix: "trap_catalog",
      selectors: ["docs/retained-failures.md", "docs/rejected.md", "INVARIANTS.md"],
      sourceAuthorityRefs: ["authority.external_repo_study.traps"],
    },
    {
      description: "External repo focused test command catalog.",
      kind: "test_command_catalog",
      refSuffix: "test_command_catalog",
      selectors: ["package.json", "tests/", "test/"],
      sourceAuthorityRefs: ["authority.external_repo_study.tests"],
    },
    {
      description: "External repo edit playbook for safe, scoped changes.",
      kind: "edit_playbook",
      refSuffix: "edit_playbook",
      selectors: ["docs/playbook.md", "docs/repo-studying.md", "README.md"],
      sourceAuthorityRefs: ["authority.external_repo_study.playbooks"],
    },
    {
      description: "External repo retained failure fixture refs.",
      kind: "retained_failure_fixture",
      refSuffix: "retained_failure_fixture",
      selectors: ["docs/retained-failures.md", "tests/", "test/"],
      sourceAuthorityRefs: ["authority.external_repo_study.retained_failures"],
    },
  ];
}

function buildExternalRationaleSources(
  manifest: OpenAgentsRepoCorpusManifest,
  input: BuildOpenAgentsExternalRepoStudyPilotInput,
): ReadonlyArray<OpenAgentsRepoStudyPacketRationaleSource> {
  const architecturePath = firstManifestPath(manifest, ["docs/architecture.md", "README.md", "AGENTS.md"]);
  const roadmapPath = firstManifestPath(manifest, ["docs/repo-studying.md", "docs/playbook.md", "README.md"]);
  const retainedFailurePath = firstManifestPath(manifest, ["docs/retained-failures.md", "docs/rejected.md", "INVARIANTS.md"]);
  return [
    {
      availability: "available",
      commit: input.commit,
      kind: "openagents_repo",
      ref: `rationale_source.external_repo.corpus.${shortHash(manifest.manifestHash)}`,
      repo: input.repo,
      sourceHash: manifest.manifestHash,
    },
    {
      availability: "available",
      commit: input.commit,
      kind: "commit_history",
      ref: `rationale_source.external_repo.commit_history.${shortHash(input.commit)}`,
      repo: input.repo,
    },
    rationaleSourceForPath(manifest, input.repo, input.commit, "tassadar_audit", architecturePath),
    rationaleSourceForPath(manifest, input.repo, input.commit, "machine_studying_roadmap", roadmapPath),
    rationaleSourceForPath(manifest, input.repo, input.commit, "backroom_archive", retainedFailurePath),
  ];
}

function rationaleSourceForPath(
  manifest: OpenAgentsRepoCorpusManifest,
  repo: string,
  commit: string,
  kind: OpenAgentsRepoStudyPacketRationaleSource["kind"],
  path: string,
): OpenAgentsRepoStudyPacketRationaleSource {
  const entry = manifest.entries.find((candidate) => candidate.path === path);
  return {
    availability: "available",
    byteSize: entry?.byteSize,
    commit,
    kind,
    path,
    ref: `rationale_source.external_repo.${kind}.${shortHash(`${repo}:${path}`)}`,
    repo,
    sourceHash: entry?.sha256,
  };
}

function defaultExternalEvidenceSpanPaths(input: {
  readonly editSitePath: string;
  readonly manifest: OpenAgentsRepoCorpusManifest;
  readonly rationaleSources: ReadonlyArray<OpenAgentsRepoStudyPacketRationaleSource>;
}): ReadonlyArray<string> {
  return [
    "AGENTS.md",
    "INVARIANTS.md",
    input.editSitePath,
    ...input.rationaleSources.map((source) => source.path).filter((path): path is string => path !== undefined),
  ].filter((path, index, paths) => input.manifest.entries.some((entry) => entry.path === path) && paths.indexOf(path) === index);
}

function selectManifestPaths(
  manifest: OpenAgentsRepoCorpusManifest,
  selectors: ReadonlyArray<string>,
): ReadonlyArray<string> {
  return manifest.entries
    .map((entry) => entry.path)
    .filter((path) =>
      selectors.some((selector) =>
        selector.endsWith("/") ? path.startsWith(selector) : path === selector,
      ),
    )
    .sort((left, right) => left.localeCompare(right));
}

function firstManifestPath(
  manifest: OpenAgentsRepoCorpusManifest,
  selectors: ReadonlyArray<string>,
): string {
  const selected = selectManifestPaths(manifest, selectors);
  return selected[0] ?? manifest.entries[0]?.path ?? "README.md";
}

function fallbackPaths(
  manifest: OpenAgentsRepoCorpusManifest,
  editSitePath: string,
): ReadonlyArray<string> {
  return manifest.entries.some((entry) => entry.path === editSitePath)
    ? [editSitePath]
    : manifest.entries.slice(0, 1).map((entry) => entry.path);
}

function validateExternalProductSurface(
  surface: OpenAgentsExternalRepoStudyProductSurface,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(surface.repo, "externalRepoStudyProductSurface.repo");
    yield* requireNonEmpty(surface.productSurfaceRef, "externalRepoStudyProductSurface.productSurfaceRef");
    yield* requireSha256(surface.productSurfaceHash, "externalRepoStudyProductSurface.productSurfaceHash");
    yield* requireSha256(surface.manifestHash, "externalRepoStudyProductSurface.manifestHash");
    yield* requireSha256(surface.packetHash, "externalRepoStudyProductSurface.packetHash");
    yield* requireSha256(surface.graphHash, "externalRepoStudyProductSurface.graphHash");
    yield* requireSha256(surface.verificationHash, "externalRepoStudyProductSurface.verificationHash");
    yield* requireSha256(surface.evalReportHash, "externalRepoStudyProductSurface.evalReportHash");
    yield* requireSha256(surface.contextHash, "externalRepoStudyProductSurface.contextHash");

    if (surface.customerPublicClaimAllowed !== false || surface.marketplacePackageAllowed !== false || surface.payoutEligible !== false) {
      return yield* externalStudyError("externalRepoStudyProductSurface.claimGates", "external repo studying pilot must not grant customer, marketplace, payout, or settlement claims");
    }

    if (surface.productSurfaceHash !== openAgentsExternalRepoStudyProductSurfaceHash(surface)) {
      return yield* externalStudyError("externalRepoStudyProductSurface.productSurfaceHash", "must match deterministic product surface hash");
    }

    if (surface.state === "pilot_ready" && !surface.studiedBeatsBaseline) {
      return yield* externalStudyError("externalRepoStudyProductSurface.state", "pilot_ready requires measured studied-substrate lift");
    }
  });
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0 ? externalStudyError(path, "must be non-empty") : Effect.void;
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.startsWith("sha256:") ? Effect.void : externalStudyError(path, "must be a sha256 ref");
}

function slugRepo(repo: string): string {
  return repo
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 80);
}

function externalStudyError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
