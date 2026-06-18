import { Effect, Schema as S } from "effect";
import {
  ProbeBenchmarkContractError,
  validateProbeBenchmarkPublicProjection,
} from "../contracts/benchmark";
import { type ProbePublicProjectionUnsafe } from "../contracts/provider-account";
import {
  OpenAgentsRepoStudiedKnowledgeGraph,
  decodeOpenAgentsRepoStudiedKnowledgeGraph,
  traverseOpenAgentsRepoStudiedKnowledgeGraph,
  type OpenAgentsRepoStudiedKnowledgeTraversal,
} from "./openagents-study-graph";
import {
  OpenAgentsRepoStudyPacket,
  decodeOpenAgentsRepoStudyPacket,
  openAgentsRepoStudyPacketHash,
} from "./openagents-study-packet";
import {
  OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  OpenAgentsStudybenchTask as OpenAgentsStudybenchTaskSchema,
  ProbeStudybenchClaimScore as ProbeStudybenchClaimScoreSchema,
  ProbeStudybenchRubricScore as ProbeStudybenchRubricScoreSchema,
  type OpenAgentsStudybenchEvidenceSpan,
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchClaimScore,
  type ProbeStudybenchRubricScore,
  decodeOpenAgentsStudybenchTask,
  decodeProbeStudybenchRubricScore,
} from "./studybench";
import {
  PROBE_STUDYBENCH_SCORER_REFS,
  buildProbeStudybenchRubricScore,
} from "./studybench-score";
import { sha256Ref, shortHash, stableJson } from "./stable-hash";

export const OPENAGENTS_STUDYBENCH_HIDDEN_EDIT_EXAM_SCHEMA_REF =
  "openagents.studybench_hidden_edit_exam.v0" as const;
export const OPENAGENTS_STUDYBENCH_HIDDEN_EDIT_EXAM_SET_SCHEMA_REF =
  "openagents.studybench_hidden_edit_exam_set.v0" as const;
export const OPENAGENTS_STUDYBENCH_EVAL_HARNESS_REPORT_SCHEMA_REF =
  "openagents.studybench_eval_harness_report.v0" as const;

export const OpenAgentsStudybenchEvalCandidateProfile = S.Literals([
  "studied_substrate",
  "baseline_grep_and_guess",
]);
export type OpenAgentsStudybenchEvalCandidateProfile =
  typeof OpenAgentsStudybenchEvalCandidateProfile.Type;

export const OpenAgentsStudybenchHiddenEditExam = S.Struct({
  deterministicCheckRefs: S.Array(S.String),
  examRef: S.String,
  expectedGraphPath: S.String,
  idealTrajectoryRefs: S.Array(S.String),
  retainedFailureRefs: S.Array(S.String),
  schemaRef: S.Literal(OPENAGENTS_STUDYBENCH_HIDDEN_EDIT_EXAM_SCHEMA_REF),
  sourceGraphRef: S.String,
  sourcePacketRef: S.String,
  studyGraphTraversalRef: S.String,
  task: OpenAgentsStudybenchTaskSchema,
});
export type OpenAgentsStudybenchHiddenEditExam =
  typeof OpenAgentsStudybenchHiddenEditExam.Type;

export const OpenAgentsStudybenchHiddenEditExamSet = S.Struct({
  examSetHash: S.String,
  examSetRef: S.String,
  exams: S.Array(OpenAgentsStudybenchHiddenEditExam),
  graphHash: S.String,
  graphRef: S.String,
  packetHash: S.String,
  packetRef: S.String,
  repo: S.String,
  schemaRef: S.Literal(OPENAGENTS_STUDYBENCH_HIDDEN_EDIT_EXAM_SET_SCHEMA_REF),
  sourceBoundary: S.Literal("private_refs_withheld"),
});
export type OpenAgentsStudybenchHiddenEditExamSet =
  typeof OpenAgentsStudybenchHiddenEditExamSet.Type;

export const OpenAgentsStudybenchEvalHarnessAttemptScore = S.Struct({
  candidateHash: S.String,
  candidateProfile: OpenAgentsStudybenchEvalCandidateProfile,
  candidateRef: S.String,
  claimScores: S.Array(ProbeStudybenchClaimScoreSchema),
  deterministicCheckRefs: S.Array(S.String),
  examRef: S.String,
  expectedFileRefs: S.Array(S.String),
  firstDivergenceRef: S.String,
  firstDivergenceStep: S.Number,
  passAtFixedBudget: S.Boolean,
  retainedFailureRefs: S.Array(S.String),
  rubricScore: ProbeStudybenchRubricScoreSchema,
  selectedFileRefs: S.Array(S.String),
  studyGraphTraversalRef: S.String,
  studiedSubstrateAvailable: S.Boolean,
  taskId: S.String,
  taskRef: S.String,
  wrongFileReadCount: S.Number,
});
export type OpenAgentsStudybenchEvalHarnessAttemptScore =
  typeof OpenAgentsStudybenchEvalHarnessAttemptScore.Type;

export const OpenAgentsStudybenchEvalHarnessAggregateScore = S.Struct({
  candidateProfile: OpenAgentsStudybenchEvalCandidateProfile,
  candidateRef: S.String,
  meanFirstDivergenceStep: S.Number,
  meanRubricScoreBps: S.Number,
  passCount: S.Number,
  passRateBps: S.Number,
  taskCount: S.Number,
  totalWrongFileReadCount: S.Number,
});
export type OpenAgentsStudybenchEvalHarnessAggregateScore =
  typeof OpenAgentsStudybenchEvalHarnessAggregateScore.Type;

export const OpenAgentsStudybenchEvalHarnessComparison = S.Struct({
  baselineCandidateRef: S.String,
  distinguishingMetricRefs: S.Array(S.String),
  firstDivergenceStepLift: S.Number,
  passRateLiftBps: S.Number,
  rubricScoreLiftBps: S.Number,
  studiedBeatsBaseline: S.Boolean,
  studiedCandidateRef: S.String,
  wrongFileReadReduction: S.Number,
});
export type OpenAgentsStudybenchEvalHarnessComparison =
  typeof OpenAgentsStudybenchEvalHarnessComparison.Type;

export const OpenAgentsStudybenchEvalHarnessReport = S.Struct({
  aggregateScores: S.Array(OpenAgentsStudybenchEvalHarnessAggregateScore),
  attemptScores: S.Array(OpenAgentsStudybenchEvalHarnessAttemptScore),
  comparison: OpenAgentsStudybenchEvalHarnessComparison,
  commit: S.String,
  examSetHash: S.String,
  examSetRef: S.String,
  generatedAt: S.String,
  graphHash: S.String,
  graphRef: S.String,
  packetHash: S.String,
  packetRef: S.String,
  reportHash: S.String,
  reportRef: S.String,
  repo: S.String,
  schemaRef: S.Literal(OPENAGENTS_STUDYBENCH_EVAL_HARNESS_REPORT_SCHEMA_REF),
  sourceBoundary: S.Literal("private_refs_withheld"),
});
export type OpenAgentsStudybenchEvalHarnessReport =
  typeof OpenAgentsStudybenchEvalHarnessReport.Type;

export interface BuildOpenAgentsStudybenchHiddenEditExamSetInput {
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly maxExams?: number;
  readonly packet: OpenAgentsRepoStudyPacket;
}

export interface OpenAgentsStudybenchEvalHarnessCandidateAttemptInput {
  readonly examRef: string;
  readonly firstDivergenceStep?: number;
  readonly satisfiedClaimIds?: ReadonlyArray<string>;
  readonly selectedFileRefs?: ReadonlyArray<string>;
  readonly wrongFileReadCount?: number;
}

export interface OpenAgentsStudybenchEvalHarnessCandidateInput {
  readonly attempts?: ReadonlyArray<OpenAgentsStudybenchEvalHarnessCandidateAttemptInput>;
  readonly candidateRef: string;
  readonly profile: OpenAgentsStudybenchEvalCandidateProfile;
  readonly studiedSubstrateAvailable?: boolean;
}

export interface RunOpenAgentsStudybenchEvalHarnessInput {
  readonly candidates?: ReadonlyArray<OpenAgentsStudybenchEvalHarnessCandidateInput>;
  readonly examSet?: OpenAgentsStudybenchHiddenEditExamSet;
  readonly generatedAt?: string;
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly maxExams?: number;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly reportRef?: string;
}

export interface RunOpenAgentsStudybenchEvalHarnessResult {
  readonly examSet: OpenAgentsStudybenchHiddenEditExamSet;
  readonly report: OpenAgentsStudybenchEvalHarnessReport;
}

const REQUIRED_EDIT_SITE_EDGE_KINDS = [
  "code_explained_by_audit",
  "code_warned_by_rejected_lineage",
  "edit_site_commit_context",
  "edit_site_respects_invariant",
  "issue_tracks_edit_site",
] as const;

const DEFAULT_MAX_EXAMS = 2;

export function buildOpenAgentsStudybenchHiddenEditExamSet(
  input: BuildOpenAgentsStudybenchHiddenEditExamSetInput,
): Effect.Effect<
  OpenAgentsStudybenchHiddenEditExamSet,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const packet = yield* decodeOpenAgentsRepoStudyPacket(input.packet);
    const graph = yield* decodeOpenAgentsRepoStudiedKnowledgeGraph(input.graph);
    yield* validatePacketGraphPair(packet, graph);

    const editSitePaths = selectCompleteEditSitePaths(graph).slice(0, input.maxExams ?? DEFAULT_MAX_EXAMS);

    if (editSitePaths.length === 0) {
      return yield* harnessError("studybenchEvalHarness.examSet.exams", "must include at least one complete edit-site traversal");
    }

    const exams: OpenAgentsStudybenchHiddenEditExam[] = [];

    for (const path of editSitePaths) {
      const traversal = yield* traverseOpenAgentsRepoStudiedKnowledgeGraph(graph, { path });
      exams.push(yield* buildHiddenEditExam({ graph, packet, path, traversal }));
    }

    const baseExamSet: OpenAgentsStudybenchHiddenEditExamSet = {
      examSetHash: "sha256:pending",
      examSetRef: "openagents_studybench_hidden_edit_exam_set.pending",
      exams,
      graphHash: graph.graphHash,
      graphRef: graph.graphRef,
      packetHash: packet.packetHash,
      packetRef: packet.packetRef,
      repo: packet.repo,
      schemaRef: OPENAGENTS_STUDYBENCH_HIDDEN_EDIT_EXAM_SET_SCHEMA_REF,
      sourceBoundary: "private_refs_withheld",
    };
    const examSetHash = openAgentsStudybenchHiddenEditExamSetHash(baseExamSet);
    const examSet: OpenAgentsStudybenchHiddenEditExamSet = {
      ...baseExamSet,
      examSetHash,
      examSetRef: `openagents_studybench_hidden_edit_exam_set.${shortHash(examSetHash)}`,
    };

    return yield* decodeOpenAgentsStudybenchHiddenEditExamSet(examSet);
  });
}

export function runOpenAgentsStudybenchEvalHarness(
  input: RunOpenAgentsStudybenchEvalHarnessInput,
): Effect.Effect<
  RunOpenAgentsStudybenchEvalHarnessResult,
  ProbeBenchmarkContractError | ProbePublicProjectionUnsafe
> {
  return Effect.gen(function* () {
    const packet = yield* decodeOpenAgentsRepoStudyPacket(input.packet);
    const graph = yield* decodeOpenAgentsRepoStudiedKnowledgeGraph(input.graph);
    yield* validatePacketGraphPair(packet, graph);

    const examSet = input.examSet === undefined
      ? yield* buildOpenAgentsStudybenchHiddenEditExamSet({
          graph,
          maxExams: input.maxExams,
          packet,
        })
      : yield* decodeOpenAgentsStudybenchHiddenEditExamSet(input.examSet);

    if (examSet.packetHash !== packet.packetHash || examSet.graphHash !== graph.graphHash) {
      return yield* harnessError("studybenchEvalHarness.examSet", "exam set must be built from the supplied packet and graph");
    }

    const candidates = input.candidates ?? defaultHarnessCandidates();
    const attemptScores: OpenAgentsStudybenchEvalHarnessAttemptScore[] = [];

    for (const candidate of candidates) {
      for (const exam of examSet.exams) {
        attemptScores.push(yield* scoreCandidateExamAttempt(candidate, exam));
      }
    }

    const aggregateScores = aggregateAttemptScores(attemptScores);
    const comparison = compareStudiedAgainstBaseline(aggregateScores);
    const baseReport: OpenAgentsStudybenchEvalHarnessReport = {
      aggregateScores,
      attemptScores: attemptScores.sort(compareAttemptScores),
      commit: packet.commit,
      comparison,
      examSetHash: examSet.examSetHash,
      examSetRef: examSet.examSetRef,
      generatedAt: input.generatedAt ?? "generated_at.withheld_for_stable_studybench_eval_harness_hash",
      graphHash: graph.graphHash,
      graphRef: graph.graphRef,
      packetHash: packet.packetHash,
      packetRef: packet.packetRef,
      reportHash: "sha256:pending",
      reportRef: "openagents_studybench_eval_harness.pending",
      repo: packet.repo,
      schemaRef: OPENAGENTS_STUDYBENCH_EVAL_HARNESS_REPORT_SCHEMA_REF,
      sourceBoundary: "private_refs_withheld",
    };
    const reportHash = openAgentsStudybenchEvalHarnessReportHash(baseReport);
    const report: OpenAgentsStudybenchEvalHarnessReport = {
      ...baseReport,
      reportHash,
      reportRef: input.reportRef ?? `openagents_studybench_eval_harness.${shortHash(reportHash)}`,
    };

    return {
      examSet,
      report: yield* decodeOpenAgentsStudybenchEvalHarnessReport(report),
    };
  });
}

export function decodeOpenAgentsStudybenchHiddenEditExamSet(
  value: unknown,
): Effect.Effect<OpenAgentsStudybenchHiddenEditExamSet, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studybenchHiddenEditExamSet");
    const examSet = yield* decodeHarnessSchema(
      OpenAgentsStudybenchHiddenEditExamSet,
      value,
      "studybenchHiddenEditExamSet",
    );
    yield* validateOpenAgentsStudybenchHiddenEditExamSet(examSet);
    return examSet;
  });
}

export function decodeOpenAgentsStudybenchEvalHarnessReport(
  value: unknown,
): Effect.Effect<OpenAgentsStudybenchEvalHarnessReport, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* validateProbeBenchmarkPublicProjection(value, "studybenchEvalHarnessReport");
    const report = yield* decodeHarnessSchema(
      OpenAgentsStudybenchEvalHarnessReport,
      value,
      "studybenchEvalHarnessReport",
    );
    yield* validateOpenAgentsStudybenchEvalHarnessReport(report);
    return report;
  });
}

export function openAgentsStudybenchHiddenEditExamSetHash(
  examSet: OpenAgentsStudybenchHiddenEditExamSet,
): string {
  const {
    examSetHash: _examSetHash,
    examSetRef: _examSetRef,
    ...stable
  } = examSet;
  return sha256Ref(stableJson(stable));
}

export function openAgentsStudybenchEvalHarnessReportHash(
  report: OpenAgentsStudybenchEvalHarnessReport,
): string {
  const {
    generatedAt: _generatedAt,
    reportHash: _reportHash,
    reportRef: _reportRef,
    ...stable
  } = report;
  return sha256Ref(stableJson(stable));
}

function buildHiddenEditExam(input: {
  readonly graph: OpenAgentsRepoStudiedKnowledgeGraph;
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly path: string;
  readonly traversal: OpenAgentsRepoStudiedKnowledgeTraversal;
}): Effect.Effect<OpenAgentsStudybenchHiddenEditExam, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const task = yield* buildHiddenEditTask({
      packet: input.packet,
      path: input.path,
    });
    const exam: OpenAgentsStudybenchHiddenEditExam = {
      deterministicCheckRefs: [
        `deterministic_check.openagents.studybench.right_edit_site.${shortHash(task.id)}`,
        `deterministic_check.openagents.studybench.graph_traversal.${shortHash(input.graph.graphHash)}`,
      ],
      examRef: `studybench_hidden_edit_exam.openagents.${task.id}`,
      expectedGraphPath: input.path,
      idealTrajectoryRefs: [
        `ideal_trajectory.openagents.studybench.${task.id}.load_packet.${shortHash(input.packet.packetHash)}`,
        `ideal_trajectory.openagents.studybench.${task.id}.traverse_graph.${shortHash(input.traversal.graphHash)}`,
        `ideal_trajectory.openagents.studybench.${task.id}.check_invariants`,
        `ideal_trajectory.openagents.studybench.${task.id}.select_tests`,
      ],
      retainedFailureRefs: retainedFailureRefsFor(input.packet, task),
      schemaRef: OPENAGENTS_STUDYBENCH_HIDDEN_EDIT_EXAM_SCHEMA_REF,
      sourceGraphRef: input.graph.graphRef,
      sourcePacketRef: input.packet.packetRef,
      studyGraphTraversalRef: `study_traversal.openagents.${task.id}.${shortHash(input.traversal.graphHash)}`,
      task,
    };

    yield* decodeOpenAgentsStudybenchTask(task);
    yield* validateProbeBenchmarkPublicProjection(exam, "studybenchHiddenEditExam");
    return yield* decodeHarnessSchema(OpenAgentsStudybenchHiddenEditExam, exam, "studybenchHiddenEditExam");
  });
}

function buildHiddenEditTask(input: {
  readonly packet: OpenAgentsRepoStudyPacket;
  readonly path: string;
}): Effect.Effect<OpenAgentsStudybenchTask, ProbeBenchmarkContractError> {
  const editEvidence = evidenceForPath(input.packet, input.path) ?? input.packet.evidenceSpans[0];
  const invariantEvidence = evidenceForPath(input.packet, "INVARIANTS.md") ?? editEvidence;
  const trapEvidence =
    evidenceForPath(input.packet, "docs/tassadar/2026-06-18-tassadar-run-actual-state-and-real-training-gap-audit.md") ??
    evidenceForPath(input.packet, "docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md") ??
    invariantEvidence;

  if (editEvidence === undefined || invariantEvidence === undefined || trapEvidence === undefined) {
    return harnessError("studybenchHiddenEditExam.task.evidence", "packet must contain evidence spans for hidden edit exams");
  }

  const taskId = `openagents_hidden_edit_${slugPath(input.path)}`;
  const evidence = uniqueEvidence([
    editEvidence.evidence,
    invariantEvidence.evidence,
    trapEvidence.evidence,
  ]);

  return Effect.succeed({
    authorityRefs: [
      "authority.openagents.repo_study.hidden_edit_exam",
      "authority.openagents.repo_study.graph_traversal",
    ],
    budgetClass: "small",
    commit: input.packet.commit,
    corpusRef: input.packet.corpusManifestRef,
    evidence,
    expectedFiles: [input.path],
    forbiddenClaimRefs: [
      "blocked_claim.wrong_edit_site",
      "blocked_claim.keyword_routing",
      "blocked_claim.reintroduced_retained_failure",
    ],
    gold_answer:
      "Use the study packet and graph traversal to choose the expected edit site, preserve invariants, avoid retained failures, and run the focused test refs.",
    id: taskId,
    privateMaterialPolicyRefs: [
      "policy.openagents.private_validation_rows_withheld",
      "policy.openagents.no_private_holdout_leakage",
    ],
    question: `Patch the OpenAgents repo-studying surface at ${opaquePathRef(input.path)} while preserving the packet, graph, and invariant boundaries.`,
    repo: input.packet.repo,
    rubric: [
      {
        claim_id: "right_edit_site",
        claim_type: "core",
        weight: 45,
        statement: "The attempt edits the graph-selected authority file rather than an adjacent or deprecated surface.",
        span_ids: [editEvidence.evidence.span_id],
      },
      {
        claim_id: "invariant_graph_path",
        claim_type: "core",
        weight: 35,
        statement: "The attempt uses the packet-to-graph traversal before editing and preserves the repo invariants.",
        span_ids: [invariantEvidence.evidence.span_id],
      },
      {
        claim_id: "retained_failure_fixture",
        claim_type: "supporting",
        weight: 20,
        statement: "The attempt avoids the retained failure fixtures and trap catalog for the selected edit surface.",
        span_ids: [trapEvidence.evidence.span_id],
      },
    ],
    schemaRef: OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
    testRefs: [
      "test.probe.openagents_studybench_eval_harness",
      "test.probe.openagents_study_graph",
    ],
    topic: "openagents_repo_studying_hidden_edit",
    visibility: "openagents_private_validation",
  });
}

function scoreCandidateExamAttempt(
  candidate: OpenAgentsStudybenchEvalHarnessCandidateInput,
  exam: OpenAgentsStudybenchHiddenEditExam,
): Effect.Effect<OpenAgentsStudybenchEvalHarnessAttemptScore, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    const override = candidate.attempts?.find((attempt) => attempt.examRef === exam.examRef);
    const selectedFileRefs = override?.selectedFileRefs === undefined
      ? defaultSelectedFileRefs(candidate.profile, exam)
      : [...override.selectedFileRefs];
    const expectedFileSet = new Set(exam.task.expectedFiles);
    const selectedFileSet = new Set(selectedFileRefs);
    const rightEditSite = exam.task.expectedFiles.every((path) => selectedFileSet.has(path));
    const studiedSubstrateAvailable = candidate.studiedSubstrateAvailable ?? candidate.profile === "studied_substrate";
    const satisfiedClaimIds = new Set(
      override?.satisfiedClaimIds ?? defaultSatisfiedClaimIds(candidate.profile, exam, rightEditSite, studiedSubstrateAvailable),
    );
    const candidateHash = sha256Ref(stableJson({
      candidateRef: candidate.candidateRef,
      examRef: exam.examRef,
      profile: candidate.profile,
      selectedFileRefs,
      satisfiedClaimIds: [...satisfiedClaimIds].sort((left, right) => left.localeCompare(right)),
    }));
    const claimScores = claimScoresFor({
      candidateRef: candidate.candidateRef,
      exam,
      rightEditSite,
      satisfiedClaimIds,
      studiedSubstrateAvailable,
    });
    const rubricScore = yield* buildProbeStudybenchRubricScore({
      candidateHash,
      claimScores,
      evidenceUseRefs: [
        `evidence_use.openagents.studybench.${exam.task.id}.${shortHash(candidateHash)}`,
        exam.studyGraphTraversalRef,
      ],
      goldAnswerRef: `gold_answer.openagents_studybench.private_validation.${exam.task.id}`,
      scoringMode: "deterministic_check",
      task: exam.task,
    });
    const wrongFileReadCount = override?.wrongFileReadCount ?? countWrongFileReads(selectedFileRefs, expectedFileSet, candidate.profile);
    const firstDivergenceStep =
      override?.firstDivergenceStep ?? defaultFirstDivergenceStep(candidate.profile, exam, rightEditSite, rubricScore);
    const attemptScore: OpenAgentsStudybenchEvalHarnessAttemptScore = {
      candidateHash,
      candidateProfile: candidate.profile,
      candidateRef: candidate.candidateRef,
      claimScores,
      deterministicCheckRefs: [...exam.deterministicCheckRefs],
      examRef: exam.examRef,
      expectedFileRefs: [...exam.task.expectedFiles],
      firstDivergenceRef: `first_divergence.openagents.studybench.${exam.task.id}.${shortHash(candidateHash)}.step_${firstDivergenceStep}`,
      firstDivergenceStep,
      passAtFixedBudget: rubricScore.finalScoreBps === 10_000,
      retainedFailureRefs: [...exam.retainedFailureRefs],
      rubricScore,
      selectedFileRefs,
      studyGraphTraversalRef: exam.studyGraphTraversalRef,
      studiedSubstrateAvailable,
      taskId: exam.task.id,
      taskRef: `studybench_task.${exam.task.visibility}.${exam.task.id}`,
      wrongFileReadCount,
    };

    return yield* decodeHarnessSchema(
      OpenAgentsStudybenchEvalHarnessAttemptScore,
      attemptScore,
      "studybenchEvalHarnessAttemptScore",
    );
  });
}

function claimScoresFor(input: {
  readonly candidateRef: string;
  readonly exam: OpenAgentsStudybenchHiddenEditExam;
  readonly rightEditSite: boolean;
  readonly satisfiedClaimIds: ReadonlySet<string>;
  readonly studiedSubstrateAvailable: boolean;
}): ReadonlyArray<ProbeStudybenchClaimScore> {
  return input.exam.task.rubric.map((claim) => {
    const satisfiedByCandidate = input.satisfiedClaimIds.has(claim.claim_id);
    const satisfied = claim.claim_id === "right_edit_site"
      ? satisfiedByCandidate && input.rightEditSite
      : claim.claim_id === "invariant_graph_path"
        ? satisfiedByCandidate && input.rightEditSite && input.studiedSubstrateAvailable
        : satisfiedByCandidate;

    return {
      claimId: claim.claim_id,
      claimType: claim.claim_type,
      evidenceSpanIds: [...claim.span_ids],
      rationaleRef: `rationale.openagents.studybench_eval.${input.exam.task.id}.${claim.claim_id}.${shortHash(input.candidateRef)}`,
      satisfied,
      schemaRef: PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
      scoreBps: satisfied ? 10_000 : 0,
      scorerRef: PROBE_STUDYBENCH_SCORER_REFS.deterministic_check,
      weight: claim.weight,
    };
  });
}

function aggregateAttemptScores(
  attempts: ReadonlyArray<OpenAgentsStudybenchEvalHarnessAttemptScore>,
): ReadonlyArray<OpenAgentsStudybenchEvalHarnessAggregateScore> {
  const byCandidateRef = new Map<string, OpenAgentsStudybenchEvalHarnessAttemptScore[]>();

  for (const attempt of attempts) {
    const existing = byCandidateRef.get(attempt.candidateRef) ?? [];
    existing.push(attempt);
    byCandidateRef.set(attempt.candidateRef, existing);
  }

  return [...byCandidateRef.entries()]
    .map(([candidateRef, candidateAttempts]) => {
      const taskCount = candidateAttempts.length;
      const passCount = candidateAttempts.filter((attempt) => attempt.passAtFixedBudget).length;
      const finalScoreTotal = candidateAttempts.reduce((total, attempt) => total + attempt.rubricScore.finalScoreBps, 0);
      const firstDivergenceTotal = candidateAttempts.reduce((total, attempt) => total + attempt.firstDivergenceStep, 0);

      return {
        candidateProfile: candidateAttempts[0]?.candidateProfile ?? "baseline_grep_and_guess",
        candidateRef,
        meanFirstDivergenceStep: Math.round(firstDivergenceTotal / taskCount),
        meanRubricScoreBps: Math.round(finalScoreTotal / taskCount),
        passCount,
        passRateBps: Math.round((passCount / taskCount) * 10_000),
        taskCount,
        totalWrongFileReadCount: candidateAttempts.reduce((total, attempt) => total + attempt.wrongFileReadCount, 0),
      };
    })
    .sort((left, right) => left.candidateRef.localeCompare(right.candidateRef));
}

function compareStudiedAgainstBaseline(
  aggregateScores: ReadonlyArray<OpenAgentsStudybenchEvalHarnessAggregateScore>,
): OpenAgentsStudybenchEvalHarnessComparison {
  const studied = aggregateScores.find((score) => score.candidateProfile === "studied_substrate");
  const baseline = aggregateScores.find((score) => score.candidateProfile === "baseline_grep_and_guess");

  if (studied === undefined || baseline === undefined) {
    return {
      baselineCandidateRef: baseline?.candidateRef ?? "candidate.openagents.studybench.baseline_grep_and_guess.missing",
      distinguishingMetricRefs: [],
      firstDivergenceStepLift: 0,
      passRateLiftBps: 0,
      rubricScoreLiftBps: 0,
      studiedBeatsBaseline: false,
      studiedCandidateRef: studied?.candidateRef ?? "candidate.openagents.studybench.studied_substrate.missing",
      wrongFileReadReduction: 0,
    };
  }

  const passRateLiftBps = studied.passRateBps - baseline.passRateBps;
  const rubricScoreLiftBps = studied.meanRubricScoreBps - baseline.meanRubricScoreBps;
  const wrongFileReadReduction = baseline.totalWrongFileReadCount - studied.totalWrongFileReadCount;
  const firstDivergenceStepLift = studied.meanFirstDivergenceStep - baseline.meanFirstDivergenceStep;
  const distinguishingMetricRefs: string[] = [];

  if (passRateLiftBps > 0) {
    distinguishingMetricRefs.push("metric.openagents.studybench.pass_at_fixed_budget_lift");
  }

  if (rubricScoreLiftBps > 0) {
    distinguishingMetricRefs.push("metric.openagents.studybench.rubric_score_lift");
  }

  if (wrongFileReadReduction > 0) {
    distinguishingMetricRefs.push("metric.openagents.studybench.wrong_file_read_reduction");
  }

  if (firstDivergenceStepLift > 0) {
    distinguishingMetricRefs.push("metric.openagents.studybench.first_divergence_step_lift");
  }

  return {
    baselineCandidateRef: baseline.candidateRef,
    distinguishingMetricRefs,
    firstDivergenceStepLift,
    passRateLiftBps,
    rubricScoreLiftBps,
    studiedBeatsBaseline: distinguishingMetricRefs.length > 0,
    studiedCandidateRef: studied.candidateRef,
    wrongFileReadReduction,
  };
}

function defaultHarnessCandidates(): ReadonlyArray<OpenAgentsStudybenchEvalHarnessCandidateInput> {
  return [
    {
      candidateRef: "candidate.openagents.studybench.studied_substrate.v0",
      profile: "studied_substrate",
      studiedSubstrateAvailable: true,
    },
    {
      candidateRef: "candidate.openagents.studybench.baseline_grep_and_guess.v0",
      profile: "baseline_grep_and_guess",
      studiedSubstrateAvailable: false,
    },
  ];
}

function selectCompleteEditSitePaths(graph: OpenAgentsRepoStudiedKnowledgeGraph): ReadonlyArray<string> {
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
      return REQUIRED_EDIT_SITE_EDGE_KINDS.every((kind) => kinds.has(kind));
    })
    .map((node) => node.source.path ?? "")
    .sort((left, right) => left.localeCompare(right));
}

function defaultSelectedFileRefs(
  profile: OpenAgentsStudybenchEvalCandidateProfile,
  exam: OpenAgentsStudybenchHiddenEditExam,
): ReadonlyArray<string> {
  if (profile === "studied_substrate") {
    return [...exam.task.expectedFiles];
  }

  return ["packages/probe/packages/runtime/src/benchmark/studybench.ts"];
}

function defaultSatisfiedClaimIds(
  profile: OpenAgentsStudybenchEvalCandidateProfile,
  exam: OpenAgentsStudybenchHiddenEditExam,
  rightEditSite: boolean,
  studiedSubstrateAvailable: boolean,
): ReadonlyArray<string> {
  if (profile === "studied_substrate" && rightEditSite && studiedSubstrateAvailable) {
    return exam.task.rubric.map((claim) => claim.claim_id);
  }

  return ["retained_failure_fixture"];
}

function countWrongFileReads(
  selectedFileRefs: ReadonlyArray<string>,
  expectedFileSet: ReadonlySet<string>,
  profile: OpenAgentsStudybenchEvalCandidateProfile,
): number {
  const selectedWrongFiles = selectedFileRefs.filter((path) => !expectedFileSet.has(path)).length;
  return selectedWrongFiles + (profile === "baseline_grep_and_guess" ? 2 : 0);
}

function defaultFirstDivergenceStep(
  profile: OpenAgentsStudybenchEvalCandidateProfile,
  exam: OpenAgentsStudybenchHiddenEditExam,
  rightEditSite: boolean,
  rubricScore: ProbeStudybenchRubricScore,
): number {
  if (profile === "studied_substrate" && rightEditSite && rubricScore.finalScoreBps === 10_000) {
    return exam.idealTrajectoryRefs.length + 1;
  }

  return 1;
}

function evidenceForPath(
  packet: OpenAgentsRepoStudyPacket,
  path: string,
): OpenAgentsRepoStudyPacket["evidenceSpans"][number] | undefined {
  return packet.evidenceSpans.find((span) => span.evidence.path === path);
}

function retainedFailureRefsFor(
  packet: OpenAgentsRepoStudyPacket,
  task: OpenAgentsStudybenchTask,
): ReadonlyArray<string> {
  const fixtureSection = packet.sections.find((section) => section.kind === "retained_failure_fixture");
  const sectionRef = fixtureSection === undefined
    ? "repo_study_section.openagents.retained_failure_fixture.missing"
    : fixtureSection.ref;

  return [
    `retained_failure.openagents.studybench.${task.id}.wrong_edit_site`,
    `retained_failure.openagents.studybench.${task.id}.trap_catalog.${shortHash(sectionRef)}`,
  ];
}

function uniqueEvidence(
  evidence: ReadonlyArray<OpenAgentsStudybenchEvidenceSpan>,
): ReadonlyArray<OpenAgentsStudybenchEvidenceSpan> {
  const bySpanId = new Map<string, OpenAgentsStudybenchEvidenceSpan>();

  for (const span of evidence) {
    bySpanId.set(span.span_id, span);
  }

  return [...bySpanId.values()].sort((left, right) => left.span_id.localeCompare(right.span_id));
}

function validatePacketGraphPair(
  packet: OpenAgentsRepoStudyPacket,
  graph: OpenAgentsRepoStudiedKnowledgeGraph,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  const packetHash = openAgentsRepoStudyPacketHash(packet);

  if (packet.packetHash !== packetHash) {
    return harnessError("studybenchEvalHarness.packetHash", "packet hash must match packet content");
  }

  if (graph.packetHash !== packet.packetHash || graph.packetRef !== packet.packetRef) {
    return harnessError("studybenchEvalHarness.graph", "graph must be built from the supplied packet");
  }

  return Effect.void;
}

function validateOpenAgentsStudybenchHiddenEditExamSet(
  examSet: OpenAgentsStudybenchHiddenEditExamSet,
): Effect.Effect<void, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(examSet.repo, "studybenchHiddenEditExamSet.repo");
    yield* requireNonEmpty(examSet.packetRef, "studybenchHiddenEditExamSet.packetRef");
    yield* requireNonEmpty(examSet.graphRef, "studybenchHiddenEditExamSet.graphRef");
    yield* requireSha256(examSet.packetHash, "studybenchHiddenEditExamSet.packetHash");
    yield* requireSha256(examSet.graphHash, "studybenchHiddenEditExamSet.graphHash");
    yield* requireSha256(examSet.examSetHash, "studybenchHiddenEditExamSet.examSetHash");

    if (examSet.exams.length === 0) {
      return yield* harnessError("studybenchHiddenEditExamSet.exams", "must include at least one exam");
    }

    if (examSet.examSetHash !== openAgentsStudybenchHiddenEditExamSetHash(examSet)) {
      return yield* harnessError("studybenchHiddenEditExamSet.examSetHash", "must match deterministic exam set content hash");
    }

    const seenExamRefs = new Set<string>();

    for (const [index, exam] of examSet.exams.entries()) {
      yield* validateHiddenEditExam(exam, `studybenchHiddenEditExamSet.exams[${index}]`);

      if (seenExamRefs.has(exam.examRef)) {
        return yield* harnessError(`studybenchHiddenEditExamSet.exams[${index}].examRef`, "must be unique");
      }

      seenExamRefs.add(exam.examRef);
    }
  });
}

function validateHiddenEditExam(
  exam: OpenAgentsStudybenchHiddenEditExam,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(exam.examRef, `${path}.examRef`);
    yield* requireNonEmpty(exam.expectedGraphPath, `${path}.expectedGraphPath`);
    yield* requireNonEmpty(exam.studyGraphTraversalRef, `${path}.studyGraphTraversalRef`);
    yield* requireNonEmptyRefs(exam.idealTrajectoryRefs, `${path}.idealTrajectoryRefs`);
    yield* requireNonEmptyRefs(exam.retainedFailureRefs, `${path}.retainedFailureRefs`);
    yield* requireNonEmptyRefs(exam.deterministicCheckRefs, `${path}.deterministicCheckRefs`);
    yield* decodeOpenAgentsStudybenchTask(exam.task);

    if (!exam.task.expectedFiles.includes(exam.expectedGraphPath)) {
      return yield* harnessError(`${path}.expectedGraphPath`, "must be one of the task expected files");
    }
  });
}

function validateOpenAgentsStudybenchEvalHarnessReport(
  report: OpenAgentsStudybenchEvalHarnessReport,
): Effect.Effect<void, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(report.repo, "studybenchEvalHarnessReport.repo");
    yield* requireNonEmpty(report.reportRef, "studybenchEvalHarnessReport.reportRef");
    yield* requireNonEmpty(report.examSetRef, "studybenchEvalHarnessReport.examSetRef");
    yield* requireSha256(report.packetHash, "studybenchEvalHarnessReport.packetHash");
    yield* requireSha256(report.graphHash, "studybenchEvalHarnessReport.graphHash");
    yield* requireSha256(report.examSetHash, "studybenchEvalHarnessReport.examSetHash");
    yield* requireSha256(report.reportHash, "studybenchEvalHarnessReport.reportHash");

    if (report.reportHash !== openAgentsStudybenchEvalHarnessReportHash(report)) {
      return yield* harnessError("studybenchEvalHarnessReport.reportHash", "must match deterministic report content hash");
    }

    if (report.attemptScores.length === 0) {
      return yield* harnessError("studybenchEvalHarnessReport.attemptScores", "must include score records");
    }

    if (report.aggregateScores.length === 0) {
      return yield* harnessError("studybenchEvalHarnessReport.aggregateScores", "must include aggregate score records");
    }

    for (const [index, attempt] of report.attemptScores.entries()) {
      yield* validateAttemptScore(attempt, `studybenchEvalHarnessReport.attemptScores[${index}]`);
    }

    if (!report.comparison.studiedBeatsBaseline || report.comparison.distinguishingMetricRefs.length === 0) {
      return yield* harnessError(
        "studybenchEvalHarnessReport.comparison",
        "studied substrate must distinguish itself from baseline on at least one metric",
      );
    }
  });
}

function validateAttemptScore(
  attempt: OpenAgentsStudybenchEvalHarnessAttemptScore,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError | ProbePublicProjectionUnsafe> {
  return Effect.gen(function* () {
    yield* requireNonEmpty(attempt.candidateRef, `${path}.candidateRef`);
    yield* requireSha256(attempt.candidateHash, `${path}.candidateHash`);
    yield* requireNonEmptyRefs(attempt.expectedFileRefs, `${path}.expectedFileRefs`);
    yield* requireNonEmptyRefs(attempt.selectedFileRefs, `${path}.selectedFileRefs`);
    yield* requireNonEmptyRefs(attempt.retainedFailureRefs, `${path}.retainedFailureRefs`);
    yield* requireNonEmptyRefs(attempt.deterministicCheckRefs, `${path}.deterministicCheckRefs`);
    yield* decodeProbeStudybenchRubricScore(attempt.rubricScore);

    if (attempt.wrongFileReadCount < 0 || !Number.isInteger(attempt.wrongFileReadCount)) {
      return yield* harnessError(`${path}.wrongFileReadCount`, "must be a non-negative integer");
    }

    if (attempt.firstDivergenceStep < 0 || !Number.isInteger(attempt.firstDivergenceStep)) {
      return yield* harnessError(`${path}.firstDivergenceStep`, "must be a non-negative integer");
    }
  });
}

function decodeHarnessSchema<A, I>(
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

function compareAttemptScores(
  left: OpenAgentsStudybenchEvalHarnessAttemptScore,
  right: OpenAgentsStudybenchEvalHarnessAttemptScore,
): number {
  return left.candidateRef.localeCompare(right.candidateRef) || left.examRef.localeCompare(right.examRef);
}

function opaquePathRef(path: string): string {
  return `repo_path.openagents.${slugPath(path)}`;
}

function slugPath(path: string): string {
  return path
    .replace(/[^a-zA-Z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .toLowerCase()
    .slice(0, 96);
}

function requireNonEmpty(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return value.trim().length === 0
    ? harnessError(path, "must be a non-empty ref")
    : Effect.void;
}

function requireNonEmptyRefs(
  refs: ReadonlyArray<string>,
  path: string,
): Effect.Effect<void, ProbeBenchmarkContractError> {
  if (refs.length === 0) {
    return harnessError(path, "must include at least one ref");
  }

  const blankIndex = refs.findIndex((ref) => ref.trim().length === 0);
  return blankIndex === -1
    ? Effect.void
    : harnessError(`${path}[${blankIndex}]`, "must be a non-empty ref");
}

function requireSha256(value: string, path: string): Effect.Effect<void, ProbeBenchmarkContractError> {
  return /^sha256:[a-f0-9]{64}$/.test(value)
    ? Effect.void
    : harnessError(path, "must be a sha256 hash ref");
}

function harnessError(path: string, reason: string): Effect.Effect<never, ProbeBenchmarkContractError> {
  return Effect.fail(new ProbeBenchmarkContractError({ path, reason }));
}
