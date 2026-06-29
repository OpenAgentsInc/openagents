import { describe, expect, test } from "bun:test";
import { Effect } from "effect";
import {
  OPENAGENTS_STUDYBENCH_DATASET_PACKAGE_SCHEMA_REF,
  OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
  PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
  decodeOpenAgentsStudybenchDatasetPackage,
  decodeOpenAgentsStudybenchTask,
  decodeProbeStudybenchClaimScore,
  decodeProbeStudybenchRubricScore,
} from "../src";

const publicRetainedTask = () => ({
  schemaRef: OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF,
  id: "openagents_launch_0001",
  topic: "launch_claims_and_promises",
  question: "Update a launch claim without implying repo studying is a public product.",
  gold_answer: "Keep the copy internal, evidence-linked, and product-promise gated.",
  rubric: [
    {
      claim_id: "c1",
      claim_type: "core",
      weight: 60,
      statement: "The answer keeps upstream StudyBench as calibration instead of product-claim evidence.",
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
      start_line: 20,
      end_line: 24,
      excerpt: "0020: Upstream rows are external public calibration only.",
    },
    {
      span_id: "s2",
      path: "docs/research/machine-studying/README.md",
      start_line: 29,
      end_line: 33,
      excerpt: "0029: Private holdout rows cannot feed study packets.",
    },
  ],
  repo: "OpenAgentsInc/openagents",
  commit: "86ed83e1e",
  corpusRef: "openagents_repo_corpus_manifest.v0.sha256_abc",
  visibility: "openagents_public_retained",
  authorityRefs: ["authority.openagents.product_promises"],
  testRefs: ["test.probe.studybench.contracts"],
  forbiddenClaimRefs: ["blocked_claim.repo_studying_public_product"],
  privateMaterialPolicyRefs: ["policy.openagents.no_private_holdout_leakage"],
  expectedFiles: ["docs/research/machine-studying/README.md"],
  budgetClass: "small",
});

const publicPackage = () => ({
  schemaRef: OPENAGENTS_STUDYBENCH_DATASET_PACKAGE_SCHEMA_REF,
  datasetRef: "dataset.openagents_studybench.public_retained.v0",
  packageRef: "dataset_package.openagents_studybench.public_retained.v0",
  packageVisibility: "openagents_public_retained",
  sourceBoundary: "public_refs_only",
  tasks: [publicRetainedTask()],
});

describe("OpenAgents StudyBench contracts", () => {
  test("decodes a public-retained OpenAgents StudyBench task and package", async () => {
    const task = await Effect.runPromise(decodeOpenAgentsStudybenchTask(publicRetainedTask()));
    const packageRecord = await Effect.runPromise(decodeOpenAgentsStudybenchDatasetPackage(publicPackage()));

    expect(task.schemaRef).toBe(OPENAGENTS_STUDYBENCH_TASK_SCHEMA_REF);
    expect(task.rubric.map((claim) => claim.claim_id)).toEqual(["c1", "c2"]);
    expect(packageRecord.tasks[0]?.id).toBe("openagents_launch_0001");
  });

  test("rejects rubric weights that do not sum to 100", async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsStudybenchTask({
          ...publicRetainedTask(),
          rubric: [
            {
              ...publicRetainedTask().rubric[0],
              weight: 99,
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchTask.rubric",
    });
  });

  test("rejects rubric claims with missing evidence spans", async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsStudybenchTask({
          ...publicRetainedTask(),
          rubric: [
            {
              ...publicRetainedTask().rubric[0],
              span_ids: ["missing_span"],
              weight: 100,
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchTask.rubric[0].span_ids[0]",
    });
  });

  test("rejects private validation or holdout rows inside public packages", async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsStudybenchDatasetPackage({
          ...publicPackage(),
          tasks: [
            {
              ...publicRetainedTask(),
              id: "openagents_private_0001",
              visibility: "openagents_private_holdout",
            },
          ],
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
      path: "studybenchDatasetPackage.tasks[0].visibility",
    });
  });

  test("rejects unsafe public material in tasks", async () => {
    await expect(
      Effect.runPromise(
        decodeOpenAgentsStudybenchTask({
          ...publicRetainedTask(),
          question: "Use this access_token to fetch the private answer.",
        }),
      ),
    ).rejects.toMatchObject({
      _tag: "ProbeBenchmarkContractError",
    });
  });

  test("decodes StudyBench claim and rubric score contracts", async () => {
    const claimScore = await Effect.runPromise(
      decodeProbeStudybenchClaimScore({
        schemaRef: PROBE_STUDYBENCH_CLAIM_SCORE_SCHEMA_REF,
        claimId: "c1",
        claimType: "core",
        evidenceSpanIds: ["s1"],
        rationaleRef: "rationale.probe.studybench.openagents_launch_0001.c1",
        satisfied: true,
        scoreBps: 10_000,
        scorerRef: "scorer.probe.studybench.manual_or_judge_supplied.v0",
        weight: 60,
      }),
    );
    const rubricScore = await Effect.runPromise(
      decodeProbeStudybenchRubricScore({
        schemaRef: PROBE_STUDYBENCH_RUBRIC_SCORE_SCHEMA_REF,
        candidateHash: "sha256:candidate",
        claimScores: [claimScore],
        coreGatePassed: true,
        evidenceUseRefs: ["evidence_use.probe.studybench.openagents_launch_0001"],
        finalScoreBps: 10_000,
        goldAnswerRef: "gold_answer.openagents_studybench.public_retained.openagents_launch_0001",
        redactionState: "public_safe",
        taskId: "openagents_launch_0001",
        weightedScoreBps: 10_000,
      }),
    );

    expect(rubricScore.claimScores[0]?.claimId).toBe("c1");
    expect(rubricScore.coreGatePassed).toBe(true);
  });
});
