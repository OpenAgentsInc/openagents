import { describe, expect, test } from "vitest";

import { forensicSha256Digest } from "@openagentsinc/forensic-contract";
import { FROZEN_FORENSIC_METRIC_REGISTRY } from "@openagentsinc/forensic-contract/metrics";

import { FORENSIC_PROMPT_PARETO_AXES } from "../schemas/forensic-prompt-optimization";
import type { ForensicPromptGovernanceState } from "../schemas/forensic-prompt-optimization";
import {
  DOMINATING_AXIS_VALUES,
  compilerInput,
  digest,
  evaluationInputFor,
  genesisState,
  projectedScorecard,
  metricFreeze,
  paretoAxes,
  releaseGate,
  sourceArtifact,
  stateAfter,
} from "./test-forensic-prompt-fixtures";
import {
  compileForensicPromptCandidates,
  forensicPromptTransitionDigestMatches,
  promoteForensicPrompt,
  rollbackForensicPrompt,
  validateForensicPromptEvaluation,
} from "./forensic-prompt-compiler";


describe("forensic prompt compiler and Blueprint governance", () => {
  test("compiles immutable evidence-only candidates whose identity covers every input class", () => {
    const run = compileForensicPromptCandidates(compilerInput());
    const candidate = run.candidates[0];
    expect(candidate?.blueprintModuleVersion).toMatchObject({
      moduleKind: "optimizer_candidate",
      releaseState: "unpromoted",
      status: "candidate",
    });
    expect(candidate?.promptArtifact.parentPromptArtifactRef).toBe(
      sourceArtifact().promptArtifactRef,
    );
    expect(Object.isFrozen(run)).toBe(true);

    const changed = compilerInput();
    changed.optimizerConfiguration.configurationDigest = digest("f");
    expect(compileForensicPromptCandidates(changed).candidates[0]?.candidateDigest).not.toBe(
      candidate?.candidateDigest,
    );
  });

  test("keeps holdouts evaluator-only and freezes T5, censoring, eligibility, and metrics", () => {
    const holdoutRead = compilerInput();
    holdoutRead.candidateInputs[0]!.exampleRefs = ["example.holdout.secret"];
    expect(() => compileForensicPromptCandidates(holdoutRead)).toThrow(/holdout examples/);

    const original = compilerInput();
    const overlap = {
      ...original,
      datasets: original.datasets.map((dataset, index) =>
        index === 0 ? { ...dataset, exampleRefs: ["example.holdout.secret"] } : dataset,
      ),
    };
    expect(() => compileForensicPromptCandidates(overlap)).toThrow(/overlap/);
  });

  test("freezes every Pareto axis before any result is known", () => {
    const input = compilerInput();
    const partial = {
      ...input,
      metricFreeze: {
        ...metricFreeze,
        paretoAxes: paretoAxes.slice(0, 3),
      },
    };
    expect(() => compileForensicPromptCandidates(partial)).toThrow(/every Pareto axis/);
  });

  test("refuses a Pareto axis bound to a metric the frozen registry does not define", () => {
    const input = compilerInput();
    const invented = {
      ...input,
      metricFreeze: {
        ...metricFreeze,
        paretoAxes: paretoAxes.map((binding) =>
          binding.axis === "cost" ? { ...binding, metricRef: "metric.forensic.cost" } : binding,
        ),
      },
    };
    expect(() => compileForensicPromptCandidates(invented)).toThrow(
      /frozen forensic metric registry/,
    );

    // The same binding is refused at evaluation time, so a freeze that passed
    // compilation elsewhere cannot smuggle an invented axis into the verdict.
    const candidate = compileForensicPromptCandidates(input).candidates[0]!;
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInputFor(candidate),
        metricFreeze: invented.metricFreeze,
      }),
    ).toThrow(/frozen forensic metric registry/);

    // Every axis the fixtures bind is a real frozen definition, so the check
    // cannot pass merely because nothing exercises it.
    expect(() => compileForensicPromptCandidates(input)).not.toThrow();
  });

  test("never presents rule-based refinement as DSPy or GEPA", () => {
    const input = compilerInput();
    const mislabeled = {
      ...input,
      optimizerConfiguration: {
        ...input.optimizerConfiguration,
        kind: "gepa" as const,
      },
    };
    expect(() => compileForensicPromptCandidates(mislabeled)).toThrow(/integration receipt/);
  });

  test("requires independent fresh-worker holdout evaluation and blocks clean regressions", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluationInput = evaluationInputFor(candidate);
    const evaluation = validateForensicPromptEvaluation(evaluationInput);
    expect(evaluation.evaluatorIdentityRef).not.toBe(
      evaluationInput.candidate.blueprintModuleVersion.provenance.createdByRef,
    );
    expect(evaluation.evidenceReceiptDigest).toBe(
      forensicSha256Digest(evaluationInput.evidence),
    );
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evaluatorIdentityRef:
          evaluationInput.candidate.blueprintModuleVersion.provenance.createdByRef,
      }),
    ).toThrow(/distinct identities/);
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        metricFreeze: {
          ...metricFreeze,
          t5DefinitionDigest: digest("9"),
        },
      }),
    ).toThrow(/frozen metric definitions/);
  });


  test("refuses an empty untouched holdout, which every blindness check would pass vacuously", () => {
    const input = compilerInput();
    for (const split of ["holdout", "clean_holdout"] as const) {
      expect(() =>
        compileForensicPromptCandidates({
          ...input,
          datasets: input.datasets.map((dataset) =>
            dataset.split === split ? { ...dataset, exampleRefs: [] } : dataset,
          ),
        }),
      ).toThrow(/must retain examples/);
    }
    // The shape being refused is the one the repository actually ships:
    // `fixtures/forensics/coldcard/dataset-splits.v1.json` declares both holdout
    // splits with no benchmark arms, so there is nothing for a campaign to be
    // blind to. Constructing that corpus is tracked on #9300.
  });

  test("refuses a metric freeze that does not pin the registry revision its axes resolve against", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluationInput = evaluationInputFor(candidate);
    expect(metricFreeze.metricDefinitionRevisionDigest).toBe(
      FROZEN_FORENSIC_METRIC_REGISTRY.revisionDigest,
    );
    // Axis bindings are admitted against one exact registry revision. A freeze
    // naming a different revision would have its axes checked against these
    // definitions while every downstream scorecard-revision comparison was made
    // against the digest the caller chose.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        metricFreeze: { ...metricFreeze, metricDefinitionRevisionDigest: digest("7") },
      }),
    ).toThrow(/pin the frozen forensic metric registry revision/);
  });

  test("reaches a verdict on scorecards this repository's own projector built", () => {
    // Before this change `rebuildForensicScorecard` emitted no per-run value for
    // `metric.causal_chain_coverage.v1` or `metric.cost_to_identification.v1`,
    // so any projector-built scorecard forced `insufficient_evidence` and the
    // gate refused every candidate regardless of its merit. The evidence below
    // is synthetic, but the projection is production's, so this proves the gate
    // is satisfiable rather than proving any candidate is good.
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const run = (
      runRef: string,
      qualified: boolean,
      overrides: { analysisMillis?: number } = {},
    ) => ({
      runRef,
      split: "holdout" as const,
      population: "vulnerable" as const,
      qualified,
      analysisMillis: overrides.analysisMillis ?? 900_000,
      totalTokens: 120_000,
      providerCostMicros: 40_000,
      infrastructureCostMicros: 6_000,
      reviewerMillis: 720_000,
    });
    const baselineHoldoutScorecard = projectedScorecard({
      scorecardRef: "scorecard.projected.baseline.v1",
      datasetRevisionDigest: candidate.holdoutDatasetDigest,
      candidateDigest: digest("0"),
      runs: [run("run.projected.baseline.hit", true), run("run.projected.baseline.miss", false)],
    });
    const holdoutScorecard = projectedScorecard({
      scorecardRef: "scorecard.holdout.v1",
      datasetRevisionDigest: candidate.holdoutDatasetDigest,
      candidateDigest: candidate.candidateDigest,
      runs: [
        run("run.projected.candidate.1", true, { analysisMillis: 800_000 }),
        run("run.projected.candidate.2", true, { analysisMillis: 800_000 }),
      ],
    });
    const cleanHoldoutScorecard = projectedScorecard({
      scorecardRef: "scorecard.projected.clean.v1",
      datasetRevisionDigest: candidate.cleanHoldoutDatasetDigest,
      candidateDigest: candidate.candidateDigest,
      runs: [
        {
          ...run("run.projected.clean.1", false),
          split: "clean_holdout" as const,
          population: "clean_control" as const,
        },
      ],
    });

    const evaluation = validateForensicPromptEvaluation({
      ...evaluationInputFor(candidate),
      baselineHoldoutScorecard,
      cleanHoldoutScorecard,
      holdoutScorecard,
    });

    // Every frozen axis resolved; none reported `unavailable`.
    expect(evaluation.paretoComparisons).toHaveLength(FORENSIC_PROMPT_PARETO_AXES.length);
    expect(
      evaluation.paretoComparisons.filter((comparison) => comparison.verdict === "unavailable"),
    ).toEqual([]);
    expect(evaluation.paretoStatus).toBe("dominates");
    expect(
      evaluation.paretoComparisons.find((comparison) => comparison.axis === "cost")?.candidateValue,
    ).toBe(46_000);
    expect(
      evaluation.paretoComparisons.find((comparison) => comparison.axis === "causal_coverage")
        ?.candidateValue,
    ).toBe(1);

    // Strip the infrastructure half of the cost measurement and the projector
    // reports the axis unavailable, which the gate must read as
    // `insufficient_evidence` rather than as a tie it can promote through.
    const withoutInfrastructureCost = {
      ...holdoutScorecard,
      runs: holdoutScorecard.runs.map((scorecardRun) => ({
        ...scorecardRun,
        values: scorecardRun.values.map((value) =>
          value.metricRef === "metric.cost_to_identification.v1"
            ? {
                metricRef: value.metricRef,
                exactness: "unavailable" as const,
                unavailableReasonRef: "unavailable.infrastructure_cost.missing",
                sourceEventRefs: [],
                sourceReceiptRefs: [],
              }
            : value,
        ),
      })),
    };
    expect(
      validateForensicPromptEvaluation({
        ...evaluationInputFor(candidate),
        baselineHoldoutScorecard,
        cleanHoldoutScorecard,
        holdoutScorecard: withoutInfrastructureCost,
      }).paretoStatus,
    ).toBe("insufficient_evidence");
  });

  test("resolves worker and source evidence as typed records rather than caller strings", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluationInput = evaluationInputFor(candidate);
    const evidence = evaluationInput.evidence;

    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: { ...evidence, workerPlacements: [evidence.workerPlacements[0]!] },
      }),
    ).toThrow(/fresh OpenAgents Cloud workers/);

    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          workerPlacements: [evidence.workerPlacements[0]!, evidence.workerPlacements[0]!],
        },
      }),
    ).toThrow(/must be distinct/);

    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          workerPlacements: [
            evidence.workerPlacements[0]!,
            { ...evidence.workerPlacements[1]!, ownerRef: "owner.someone-else" },
          ],
        },
      }),
    ).toThrow(/one owner scope/);

    // A worker that only reached readiness after its scorecard existed cannot
    // be the worker that produced it.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          workerPlacements: [
            evidence.workerPlacements[0]!,
            { ...evidence.workerPlacements[1]!, updatedAt: "2026-08-01T17:00:00.000Z" },
          ],
        },
      }),
    ).toThrow(/before their scorecards exist/);

    // Resolved source states must carry the candidate's exact untouched
    // holdout revisions, not arbitrary refs.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          sourceStates: [
            evidence.sourceStates[0]!,
            { ...evidence.sourceStates[1]!, datasetDigest: digest("f") },
          ],
        },
      }),
    ).toThrow(/untouched holdout revisions/);

    // Mechanical evidence must cover all three kinds and bind a real scorecard.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: { ...evidence, mechanicalEvidence: evidence.mechanicalEvidence.slice(0, 2) },
      }),
    ).toThrow(/independent mechanical evidence/);
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: {
          ...evidence,
          mechanicalEvidence: evidence.mechanicalEvidence.map((receipt) => ({
            ...receipt,
            scorecardRef: "scorecard.some-other-run",
          })),
        },
      }),
    ).toThrow(/one of the evaluated scorecards/);

    // The evidence must name the exact evaluation and candidate it is offered for.
    expect(() =>
      validateForensicPromptEvaluation({
        ...evaluationInput,
        evidence: { ...evidence, candidateDigest: digest("0") },
      }),
    ).toThrow(/bind the exact evaluation and candidate/);
  });

  test("derives the Pareto verdict from measured scorecards instead of accepting a claim", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;

    const dominating = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    expect(dominating.paretoStatus).toBe("dominates");
    expect(dominating.paretoComparisons).toHaveLength(FORENSIC_PROMPT_PARETO_AXES.length);
    expect(
      dominating.paretoComparisons.find((comparison) => comparison.axis === "hit_rate"),
    ).toMatchObject({ candidateValue: 0.7, baselineValue: 0.4, verdict: "better" });
    expect(
      dominating.paretoComparisons.find((comparison) => comparison.axis === "tokens")?.verdict,
    ).toBe("equal");

    // Spending more tokens for the same discovery win is a trade-off, not
    // domination, and the derived status has to say so.
    const tradedOff = validateForensicPromptEvaluation(
      evaluationInputFor(candidate, { ...DOMINATING_AXIS_VALUES, tokens: 200_000 }),
    );
    expect(tradedOff.paretoStatus).toBe("non_dominated");
    expect(
      tradedOff.paretoComparisons.find((comparison) => comparison.axis === "tokens")?.verdict,
    ).toBe("worse");

    // An axis with no measured value on either side is reported as missing
    // evidence, never silently scored as a tie.
    const { reviewer_load: _omitted, ...withoutReviewerLoad } = DOMINATING_AXIS_VALUES;
    const incomplete = validateForensicPromptEvaluation(
      evaluationInputFor(candidate, withoutReviewerLoad),
    );
    expect(incomplete.paretoStatus).toBe("insufficient_evidence");
    expect(
      incomplete.paretoComparisons.find((comparison) => comparison.axis === "reviewer_load"),
    ).toMatchObject({ candidateValue: null, verdict: "unavailable" });
  });

  test("requires an operator release gate and rolls back without rewriting activation history", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluation = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    const activation = promoteForensicPrompt({
      candidate,
      currentState: genesisState(),
      decidedAt: "2026-08-01T17:00:00.000Z",
      evaluation,
      evaluationRef: "evaluation.forensic.candidate.1",
      operatorDecisionRef: "operator-decision.forensic.1",
      operatorIdentityRef: "identity.operator.release",
      releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
      transitionRef: "transition.forensic.activate.1",
    });
    const rollback = rollbackForensicPrompt({
      currentState: stateAfter(activation),
      decidedAt: "2026-08-01T18:00:00.000Z",
      operatorDecisionRef: "operator-decision.forensic.rollback.1",
      operatorIdentityRef: "identity.operator.release",
      transitionRef: "transition.forensic.rollback.1",
    });
    expect(activation.activePromptDigest).toBe(candidate.candidateDigest);
    expect(activation.priorActivePromptDigest).toBeNull();
    expect(activation.sequence).toBe(1);
    expect(rollback.activePromptDigest).toBeNull();
    expect(rollback.priorActivePromptDigest).toBe(activation.activePromptDigest);
    expect(rollback.sequence).toBe(2);
    expect(activation.transitionType).toBe("activate");
    expect(rollback.transitionType).toBe("rollback");
    expect(forensicPromptTransitionDigestMatches(activation)).toBe(true);
    expect(forensicPromptTransitionDigestMatches(rollback)).toBe(true);
    expect(
      forensicPromptTransitionDigestMatches({ ...activation, activePromptDigest: digest("0") }),
    ).toBe(false);

    expect(() =>
      promoteForensicPrompt({
        candidate,
        currentState: genesisState(),
        decidedAt: "2026-08-01T17:00:00.000Z",
        evaluation,
        evaluationRef: "evaluation.forensic.candidate.1",
        operatorDecisionRef: "operator-decision.forensic.1",
        operatorIdentityRef: "identity.optimizer.generator",
        releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
        transitionRef: "transition.forensic.self-promote",
      }),
    ).toThrow(/cannot evaluate or promote themselves/);
  });

  test("reads the rollback anchor from the durable pointer instead of the caller", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluation = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    const promote = (currentState: ForensicPromptGovernanceState) =>
      promoteForensicPrompt({
        candidate,
        currentState,
        decidedAt: "2026-08-01T17:00:00.000Z",
        evaluation,
        evaluationRef: "evaluation.forensic.candidate.1",
        operatorDecisionRef: "operator-decision.forensic.1",
        operatorIdentityRef: "identity.operator.release",
        releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
        transitionRef: "transition.forensic.activate.1",
      });

    // A promotion taken against a state whose pointer already holds a governed
    // prompt anchors on that observed digest, not on anything the caller says.
    const priorActivation = promote(genesisState());
    const rolledBack = rollbackForensicPrompt({
      currentState: stateAfter(priorActivation),
      decidedAt: "2026-08-01T18:00:00.000Z",
      operatorDecisionRef: "operator-decision.forensic.rollback.1",
      operatorIdentityRef: "identity.operator.release",
      transitionRef: "transition.forensic.rollback.1",
    });
    expect(rolledBack.rollbackAnchorDigest).toBe(priorActivation.rollbackAnchorDigest);

    // The pointer and its history must agree before any decision is taken.
    expect(() =>
      promote({ ...genesisState(), revision: 3 }),
    ).toThrow(/append-only history length/);
    expect(() =>
      promote({ ...stateAfter(priorActivation), activePromptDigest: digest("0") }),
    ).toThrow(/must equal the last recorded transition/);

    // Re-promoting the prompt that is already active is not a transition.
    expect(() => promote(stateAfter(priorActivation))).toThrow(/already the active governed/);
  });

  test("holds rollback to the same authority boundary as promotion", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const evaluation = validateForensicPromptEvaluation(evaluationInputFor(candidate));
    const activation = promoteForensicPrompt({
      candidate,
      currentState: genesisState(),
      decidedAt: "2026-08-01T17:00:00.000Z",
      evaluation,
      evaluationRef: "evaluation.forensic.candidate.1",
      operatorDecisionRef: "operator-decision.forensic.1",
      operatorIdentityRef: "identity.operator.release",
      releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
      transitionRef: "transition.forensic.activate.1",
    });

    // The candidate's own producer cannot mint a rollback, which would
    // otherwise be an unguarded path to setting the active pointer.
    expect(() =>
      rollbackForensicPrompt({
        currentState: stateAfter(activation),
        decidedAt: "2026-08-01T18:00:00.000Z",
        operatorDecisionRef: "operator-decision.forensic.rollback.1",
        operatorIdentityRef: candidate.blueprintModuleVersion.provenance.createdByRef,
        transitionRef: "transition.forensic.rollback.1",
      }),
    ).toThrow(/cannot evaluate or promote themselves/);

    // There must be a recorded activation to revert.
    expect(() =>
      rollbackForensicPrompt({
        currentState: genesisState(),
        decidedAt: "2026-08-01T18:00:00.000Z",
        operatorDecisionRef: "operator-decision.forensic.rollback.1",
        operatorIdentityRef: "identity.operator.release",
        transitionRef: "transition.forensic.rollback.1",
      }),
    ).toThrow(/recorded activation to revert/);

    // A stored activation that has been edited fails its digest check.
    const tampered = { ...activation, activePromptDigest: digest("0") };
    expect(() =>
      rollbackForensicPrompt({
        currentState: {
          ...stateAfter(tampered),
          activePromptDigest: tampered.activePromptDigest,
        },
        decidedAt: "2026-08-01T18:00:00.000Z",
        operatorDecisionRef: "operator-decision.forensic.rollback.1",
        operatorIdentityRef: "identity.operator.release",
        transitionRef: "transition.forensic.rollback.1",
      }),
    ).toThrow(/immutable digest check/);
  });

  test("refuses promotion when the frozen Pareto axes were not all measured", () => {
    const candidate = compileForensicPromptCandidates(compilerInput()).candidates[0]!;
    const { reviewer_load: _omitted, ...withoutReviewerLoad } = DOMINATING_AXIS_VALUES;
    const evaluation = validateForensicPromptEvaluation(
      evaluationInputFor(candidate, withoutReviewerLoad),
    );
    expect(evaluation.paretoStatus).toBe("insufficient_evidence");
    expect(() =>
      promoteForensicPrompt({
        candidate,
        currentState: genesisState(),
        decidedAt: "2026-08-01T17:00:00.000Z",
        evaluation,
        evaluationRef: "evaluation.forensic.candidate.1",
        operatorDecisionRef: "operator-decision.forensic.1",
        operatorIdentityRef: "identity.operator.release",
        releaseGate: releaseGate(candidate.blueprintModuleVersion.id),
        transitionRef: "transition.forensic.activate.1",
      }),
    ).toThrow(/derived Pareto comparison/);
  });
});
