import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_SCORER_REFS,
  type OpenAgentsStudybenchTask,
  type ProbeStudybenchClaimScore,
  buildProbeStudybenchRubricScore,
} from "../src";

const task = (): OpenAgentsStudybenchTask => ({
  schemaRef: OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  id: "openagents_launch_0001",
  topic: "launch_claims_and_promises",
  question: "Update launch copy without overclaiming repo studying.",
  gold_answer: "Keep repo studying internal, evidence-linked, and product-promise gated.",
  rubric: [
    {
      claim_id: "c1",
      claim_type: "core",
      weight: 60,
      statement: "The answer keeps upstream StudyBench as public calibration.",
      span_ids: ["s1"],
    },
    {
      claim_id: "c2",
      claim_type: "supporting",
      weight: 40,
      statement: "The answer preserves the private holdout boundary.",
      span_ids: ["s2"],
    },
  ],
  evidence: [
    {
      span_id: "s1",
      path: "docs/research/machine-studying/README.md",
      start_line: 38,
      end_line: 40,
      excerpt: "0038: Upstream rows are external public calibration only.",
    },
    {
      span_id: "s2",
      path: "docs/research/machine-studying/README.md",
      start_line: 43,
      end_line: 46,
      excerpt: "0043: Private holdout rows are not committed.",
    },
  ],
  repo: "OpenAgentsInc/openagents",
  commit: "06fb0e335",
  corpusRef: "openagents_repo_corpus_manifest.v0.sha256_abc",
  visibility: "openagents_public_retained",
  authorityRefs: ["authority.openagents.product_promises"],
  testRefs: ["test.probe.studybench.score"],
  forbiddenClaimRefs: ["blocked_claim.repo_studying_public_product"],
  privateMaterialPolicyRefs: ["policy.openagents.no_private_holdout_leakage"],
  expectedFiles: ["docs/research/machine-studying/README.md"],
  budgetClass: "small",
});

const claimScore = (overrides: Partial<ProbeStudybenchClaimScore> = {}): ProbeStudybenchClaimScore => ({
  schemaRef: PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  claimId: "c1",
  claimType: "core",
  evidenceSpanIds: ["s1"],
  rationaleRef: "rationale.probe.studybench.openagents_launch_0001.c1",
  satisfied: true,
  scoreBps: 10_000,
  scorerRef: PROBE_STUDYBENCH_SCORER_REFS.manual_or_judge_supplied,
  weight: 60,
  ...overrides,
});

const supportingClaimScore = (overrides: Partial<ProbeStudybenchClaimScore> = {}): ProbeStudybenchClaimScore =>
  claimScore({
    claimId: "c2",
    claimType: "supporting",
    evidenceSpanIds: ["s2"],
    rationaleRef: "rationale.probe.studybench.openagents_launch_0001.c2",
    scoreBps: 5_000,
    weight: 40,
    ...overrides,
  });

describe("StudyBench rubric scorer", () => {
  test("computes weighted score from task rubric weights", async () => {
    const score = await Effect.runPromise(
      buildProbeStudybenchRubricScore({
        candidateHash: "sha256:candidate",
        claimScores: [claimScore(), supportingClaimScore()],
        evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
        goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
        task: task(),
      }),
    );

    expect(score.weightedScoreBps).toBe(8_000);
    expect(score.coreGatePassed).toBe(true);
    expect(score.finalScoreBps).toBe(8_000);
  });

  test("zeros final score when a strict core claim gate fails", async () => {
    const score = await Effect.runPromise(
      buildProbeStudybenchRubricScore({
        candidateHash: "sha256:candidate",
        claimScores: [
          claimScore({
            satisfied: false,
            scoreBps: 0,
          }),
          supportingClaimScore({ scoreBps: 10_000 }),
        ],
        evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
        goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
        task: task(),
      }),
    );

    expect(score.weightedScoreBps).toBe(4_000);
    expect(score.coreGatePassed).toBe(false);
    expect(score.finalScoreBps).toBe(0);
  });

  test("supports deterministic-check scoring mode", async () => {
    const score = await Effect.runPromise(
      buildProbeStudybenchRubricScore({
        candidateHash: "sha256:candidate",
        claimScores: [
          claimScore({ scorerRef: PROBE_STUDYBENCH_SCORER_REFS.deterministic_check }),
          supportingClaimScore({ scorerRef: PROBE_STUDYBENCH_SCORER_REFS.deterministic_check }),
        ],
        evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
        goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
        scoringMode: "deterministic_check",
        task: task(),
      }),
    );

    expect(score.finalScoreBps).toBe(8_000);
  });

  test("rejects missing claim scores", async () => {
    await expect(
      Effect.runPromise(
        buildProbeStudybenchRubricScore({
          candidateHash: "sha256:candidate",
          claimScores: [claimScore()],
          evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
          goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
          task: task(),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchScore.claimScores",
    });
  });

  test("rejects unknown claim ids", async () => {
    await expect(
      Effect.runPromise(
        buildProbeStudybenchRubricScore({
          candidateHash: "sha256:candidate",
          claimScores: [
            claimScore({
              claimId: "c3",
              weight: 60,
            }),
            supportingClaimScore(),
          ],
          evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
          goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
          task: task(),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchScore.claimScores[0].claimId",
    });
  });

  test("rejects raw judge rationale text in public summaries", async () => {
    await expect(
      Effect.runPromise(
        buildProbeStudybenchRubricScore({
          candidateHash: "sha256:candidate",
          claimScores: [
            claimScore({
              rationaleRef: "The judge rejected this because it missed the boundary.",
            }),
            supportingClaimScore(),
          ],
          evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
          goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
          task: task(),
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchScore.claimScores[0].rationaleRef",
    });
  });
});
