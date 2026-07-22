import { readFileSync } from "node:fs";
import path from "node:path";

import { Schema } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS,
  IDE_PORTABLE_PHASES,
  IDE_PORTABLE_REQUIRED_FAULT_CASES,
  IDE_PORTABLE_TARGET_CLASSES,
  IdePortableEvidenceReceiptSchema,
  type IdePortableEvidenceReceipt,
  validateIdePortableEvidenceReceipt,
} from "./portable-evidence-contract.ts";

const candidate = "a".repeat(40);
const base = "b".repeat(40);
const main = "d".repeat(40);
const digest = "c".repeat(64);
const decode = Schema.decodeUnknownSync(IdePortableEvidenceReceiptSchema);

const evidenceClassByTarget = {
  owner_local: "real_local",
  owner_managed: "real_owner_managed",
  openagents_managed: "real_openagents_managed",
  managed_provider: "real_managed_provider",
} as const;

type TargetClass = keyof typeof evidenceClassByTarget;

const makeMetrics = (cohortRef: string) => [
  ...IDE_PORTABLE_PHASES.map((phase) => ({
    metricRef: `metric:${cohortRef}:phase_latency:${phase}`,
    metric: "phase_latency" as const,
    phase,
    unit: "milliseconds" as const,
    repetitions: 30,
    p50: 1,
    p95: 2,
    p99: 3,
    thresholdP95: 20,
    thresholdP99: 30,
    passed: true,
    receiptRef: `receipt:metric:${cohortRef}:phase_latency:${phase}`,
  })),
  ...IDE_PORTABLE_NON_PHASE_ACCEPTANCE_METRICS.map((metric) => ({
    metricRef: `metric:${cohortRef}:${metric}`,
    metric,
    phase: null,
    unit:
      metric === "cpu"
        ? ("percent" as const)
        : metric === "checkpoint_size" || metric === "memory"
          ? ("bytes" as const)
          : metric === "network"
            ? ("bytes_per_second" as const)
            : metric === "queue" || metric === "lease" || metric === "resource_cleanup"
              ? ("count" as const)
              : ("milliseconds" as const),
    repetitions: 30,
    p50: 1,
    p95: 2,
    p99: 3,
    thresholdP95: 20,
    thresholdP99: 30,
    passed: true,
    receiptRef: `receipt:metric:${cohortRef}:${metric}`,
  })),
];

const makeCohort = (targetClass: TargetClass, suffix = "primary") => {
  const evidenceClass = evidenceClassByTarget[targetClass];
  const cohortRef = `cohort:${targetClass}:${suffix}`;
  return {
    cohortRef,
    targetClass,
    evidenceClass,
    journeyScope: "full_move" as const,
    journeys: {
      mainJourneyReceiptRef: `receipt:journey:${cohortRef}:main`,
      failbackJourneyReceiptRef: `receipt:journey:${cohortRef}:failback`,
      faultMatrixReceiptRef: `receipt:journey:${cohortRef}:fault-matrix`,
    },
    operatingSystem: targetClass === "owner_local" ? ("darwin" as const) : ("linux" as const),
    architecture: "arm64" as const,
    adapter: {
      kind: "production" as const,
      ref: `adapter:${cohortRef}`,
      name: `${targetClass} adapter`,
      version: "1.0.0",
    },
    targetRef: `target:${cohortRef}`,
    artifact: { ref: `artifact:${cohortRef}`, sha256: digest, bytes: 4_096 },
    candidateCommitSha: candidate,
    baseCommitSha: base,
    capabilityState: "ready" as const,
    custody:
      targetClass === "owner_local"
        ? ("owner_device" as const)
        : targetClass === "owner_managed"
          ? ("owner_managed" as const)
          : targetClass === "openagents_managed"
            ? ("openagents_managed" as const)
            : ("provider_managed" as const),
    networkDestinations: [],
    dataDestinations: [targetClass],
    retentionSeconds: 3_600,
    costFact: "measured",
    phaseReceipts: IDE_PORTABLE_PHASES.map((phase) => ({
      phase,
      evidenceClass,
      receiptRef: `receipt:${cohortRef}:${phase}`,
      operationRef: `operation:${cohortRef}:${phase}`,
      attachmentGeneration: 2,
      result: "passed" as const,
    })),
    metrics: makeMetrics(cohortRef),
    result: "The complete real cohort passed.",
  };
};

const completeReceipt = (): IdePortableEvidenceReceipt => {
  const placementCohorts = IDE_PORTABLE_TARGET_CLASSES.map((targetClass) =>
    makeCohort(targetClass),
  );
  return decode({
    schemaVersion: "openagents.desktop.ide-portable-evidence.v3",
    issue: "IDE-13",
    candidateCommitSha: candidate,
    baseCommitSha: base,
    generatedAt: "2026-07-20T12:00:00.000Z",
    producerRef: "producer:ide-13",
    acceptanceRefs: {
      candidateRef: "git:candidate",
      mainCommitSha: main,
      mainRef: "git:main",
      artifactReceiptRef: "receipt:artifact-set",
      rollbackReceiptRef: "receipt:rollback",
      verificationCommandRef: "verification:commands",
      verificationResultRef: "verification:results",
    },
    environment: { platform: "darwin", architecture: "arm64", node: "v24.13.1" },
    model: {
      maximumDepth: 12,
      exploredStates: 14,
      exploredTransitions: 252,
      staleWriteAttempts: 80,
      counterexamples: 0,
      passed: true,
    },
    implementedChecks: [
      {
        checkRef: "check:model",
        evidenceClass: "model",
        result: "passed",
        receiptRef: "packages/portable-session-contract/src/ide13-model.test.ts",
      },
    ],
    placementCohorts,
    omissions: [
      {
        omissionRef: "omission:none",
        targetClass: "owner_local",
        fact: "No optional capability was added to the checkpoint.",
        disposition: "accepted_limit",
        evidenceRef: "receipt:omission:none",
      },
    ],
    recoveryFacts: placementCohorts.map((cohort) => ({
      recoveryRef: `recovery:${cohort.cohortRef}:older-point`,
      cohortRef: cohort.cohortRef,
      targetClass: cohort.targetClass,
      scenario: "The target failed after source revocation.",
      evidenceClass: cohort.evidenceClass,
      outcome: "passed",
      recoveryPointRef: `checkpoint:${cohort.cohortRef}:older`,
      receiptRef: `receipt:recovery:${cohort.cohortRef}:older-point`,
    })),
    faultFacts: placementCohorts.flatMap((cohort, cohortIndex) =>
      IDE_PORTABLE_REQUIRED_FAULT_CASES.filter((_, faultIndex) =>
        faultIndex % placementCohorts.length === cohortIndex,
      ).map((fault) => ({
        faultRef: `fault:${cohort.cohortRef}:${fault.scenario}:${fault.phase ?? "all"}`,
        cohortRef: cohort.cohortRef,
        targetClass: cohort.targetClass,
        scenario: fault.scenario,
        phase: fault.phase,
        evidenceClass: cohort.evidenceClass,
        outcome: "passed",
        recoveryPointRef: `checkpoint:${cohort.cohortRef}:${fault.scenario}:${fault.phase ?? "all"}`,
        receiptRef: `receipt:fault:${cohort.cohortRef}:${fault.scenario}:${fault.phase ?? "all"}`,
      })),
    ),
    security: {
      forbiddenMaterialProjected: false,
      optimisticAuthorityProjected: false,
      staleGenerationAccepted: false,
      rawCredentialProjected: false,
    },
    review: {
      independentReviewerRef: "reviewer:independent",
      independentDisposition: "accepted",
      independentDispositionRef: "review:independent:accepted",
      ownerRef: "owner:openagents",
      ownerDisposition: "accepted",
      ownerDispositionRef: "review:owner:accepted",
    },
    implementationChecksPassed: true,
    acceptancePassed: true,
    remainingGaps: [],
  });
};

const validate = (receipt: IdePortableEvidenceReceipt): void =>
  validateIdePortableEvidenceReceipt(receipt, {
    candidateCommitSha: candidate,
    baseCommitSha: base,
  });

const expectSemanticRejection = (receipt: IdePortableEvidenceReceipt, pattern: RegExp): void => {
  expect(() => decode(receipt)).toThrow(pattern);
  expect(() => validate(receipt)).toThrow(pattern);
};

const unclaimedProviderCohort = (cohortRef: string, template: ReturnType<typeof makeCohort>) => ({
  ...template,
  cohortRef,
  evidenceClass: "not_run" as const,
  journeyScope: "not_run" as const,
  journeys: {
    mainJourneyReceiptRef: null,
    failbackJourneyReceiptRef: null,
    faultMatrixReceiptRef: null,
  },
  operatingSystem: "unknown" as const,
  architecture: "unknown" as const,
  adapter: { kind: "not_run" as const, ref: null, name: null, version: null },
  targetRef: null,
  artifact: { ref: null, sha256: null, bytes: null },
  capabilityState: "unsupported" as const,
  custody: "unverified" as const,
  networkDestinations: [],
  dataDestinations: [],
  retentionSeconds: 0,
  phaseReceipts: IDE_PORTABLE_PHASES.map((phase) => ({
    phase,
    evidenceClass: "not_run" as const,
    receiptRef: null,
    operationRef: null,
    attachmentGeneration: null,
    result: "not_run" as const,
  })),
  metrics: [],
  result: "No managed provider is admitted or claimed.",
});

describe("IDE-13 portability evidence contract", () => {
  test("decodes the accepted real cohort set without promoting the provider lane", () => {
    const current = decode(
      JSON.parse(
        readFileSync(
          path.resolve(
            import.meta.dirname,
            "../../benchmarks/ide/2026-07-20-ide-13-portability.json",
          ),
          "utf8",
        ),
      ),
    );
    expect(() =>
      validateIdePortableEvidenceReceipt(current, {
        candidateCommitSha: current.candidateCommitSha,
        baseCommitSha: current.baseCommitSha,
      }),
    ).not.toThrow();
    expect(current.acceptancePassed).toBe(true);
    expect(
      current.placementCohorts.filter((cohort) => cohort.evidenceClass === "real_local"),
    ).toHaveLength(1);
    expect(
      current.placementCohorts.filter((cohort) =>
        ["real_local", "real_owner_managed", "real_openagents_managed"].includes(
          cohort.evidenceClass,
        ),
      ),
    ).toHaveLength(3);
    expect(
      current.placementCohorts.find((cohort) => cohort.targetClass === "managed_provider"),
    ).toMatchObject({ evidenceClass: "not_run", capabilityState: "unsupported" });
  });

  test("accepts a complete real target, fault, recovery, metric, and review matrix", () => {
    expect(() => validate(completeReceipt())).not.toThrow();
  });

  test("allows bounded additional claimed-provider cohorts", () => {
    const receipt = completeReceipt();
    const extra = makeCohort("managed_provider", "second-provider");
    const expanded = decode({
      ...receipt,
      placementCohorts: [...receipt.placementCohorts, extra],
      recoveryFacts: [
        ...receipt.recoveryFacts,
        {
          recoveryRef: `recovery:${extra.cohortRef}:older-point`,
          cohortRef: extra.cohortRef,
          targetClass: extra.targetClass,
          scenario: "The provider evicted the target.",
          evidenceClass: extra.evidenceClass,
          outcome: "passed",
          recoveryPointRef: `checkpoint:${extra.cohortRef}:older`,
          receiptRef: `receipt:recovery:${extra.cohortRef}:older-point`,
        },
      ],
      faultFacts: [
        ...receipt.faultFacts,
        ...IDE_PORTABLE_REQUIRED_FAULT_CASES.map((fault) => ({
          faultRef: `fault:${extra.cohortRef}:${fault.scenario}:${fault.phase ?? "all"}`,
          cohortRef: extra.cohortRef,
          targetClass: extra.targetClass,
          scenario: fault.scenario,
          phase: fault.phase,
          evidenceClass: extra.evidenceClass,
          outcome: "passed",
          recoveryPointRef: `checkpoint:${extra.cohortRef}:${fault.scenario}:${fault.phase ?? "all"}`,
          receiptRef: `receipt:fault:${extra.cohortRef}:${fault.scenario}:${fault.phase ?? "all"}`,
        })),
      ],
    });
    expect(() => validate(expanded)).not.toThrow();
  });

  test("does not require a real managed-provider cohort when no provider is claimed", () => {
    const receipt = completeReceipt();
    const provider = receipt.placementCohorts.find(
      (cohort) => cohort.targetClass === "managed_provider",
    );
    if (provider === undefined) throw new Error("missing provider fixture");
    const unclaimed = decode({
      ...receipt,
      placementCohorts: receipt.placementCohorts.map((cohort) =>
        cohort.cohortRef === provider.cohortRef
          ? unclaimedProviderCohort(provider.cohortRef, makeCohort("managed_provider"))
          : cohort,
      ),
      recoveryFacts: receipt.recoveryFacts.filter((fact) => fact.cohortRef !== provider.cohortRef),
      faultFacts: receipt.faultFacts.map((fact) =>
        fact.cohortRef === provider.cohortRef
          ? {
              ...fact,
              cohortRef: receipt.placementCohorts[0]!.cohortRef,
              targetClass: "owner_local" as const,
              evidenceClass: "real_local" as const,
            }
          : fact,
      ),
    });
    expect(() => validate(unclaimed)).not.toThrow();
  });

  test("rejects acceptance-gap omissions", () => {
    const receipt = completeReceipt();
    expectSemanticRejection(
      {
        ...receipt,
        omissions: receipt.omissions.map((omission) => ({
          ...omission,
          disposition: "acceptance_gap" as const,
        })),
      },
      /acceptance-gap omission/u,
    );
  });

  test("rejects missing exact acceptance refs", () => {
    const receipt = completeReceipt();
    expectSemanticRejection(
      {
        ...receipt,
        acceptanceRefs: { ...receipt.acceptanceRefs, rollbackReceiptRef: null },
      },
      /exact candidate, main, artifact, rollback, and verification refs/u,
    );
  });

  test("rejects a missing per-cohort main, failback, or fault-matrix journey ref", () => {
    const receipt = completeReceipt();
    const cohort = receipt.placementCohorts[0];
    if (cohort === undefined) throw new Error("missing cohort fixture");
    expectSemanticRejection(
      {
        ...receipt,
        placementCohorts: receipt.placementCohorts.map((current) =>
          current.cohortRef === cohort.cohortRef
            ? {
                ...current,
                journeys: { ...current.journeys, failbackJourneyReceiptRef: null },
              }
            : current,
        ),
      },
      /lacks exact .* journey refs/u,
    );
  });

  test("rejects a missing phase-latency observation", () => {
    const receipt = completeReceipt();
    const cohort = receipt.placementCohorts[0];
    if (cohort === undefined) throw new Error("missing cohort fixture");
    expectSemanticRejection(
      {
        ...receipt,
        placementCohorts: receipt.placementCohorts.map((current) =>
          current.cohortRef === cohort.cohortRef
            ? { ...current, metrics: current.metrics.slice(1) }
            : current,
        ),
      },
      /acceptance metric and phase matrix is incomplete or duplicated/u,
    );
  });

  test("rejects a duplicate metric and phase identity", () => {
    const receipt = completeReceipt();
    const cohort = receipt.placementCohorts[0];
    if (cohort === undefined) throw new Error("missing cohort fixture");
    const first = cohort.metrics[0];
    if (first === undefined) throw new Error("missing metric fixture");
    expectSemanticRejection(
      {
        ...receipt,
        placementCohorts: receipt.placementCohorts.map((current) =>
          current.cohortRef === cohort.cohortRef
            ? { ...current, metrics: [...current.metrics.slice(0, -1), first] }
            : current,
        ),
      },
      /acceptance metric and phase matrix is incomplete or duplicated/u,
    );
  });

  test("rejects a failed acceptance metric", () => {
    const receipt = completeReceipt();
    const cohort = receipt.placementCohorts[1];
    if (cohort === undefined) throw new Error("missing cohort fixture");
    expectSemanticRejection(
      {
        ...receipt,
        placementCohorts: receipt.placementCohorts.map((current) =>
          current.cohortRef === cohort.cohortRef
            ? {
                ...current,
                metrics: current.metrics.map((metric, index) =>
                  index === 8 ? { ...metric, passed: false } : metric,
                ),
              }
            : current,
        ),
      },
      /contains a failed .* metric/u,
    );
  });

  test("rejects failed, not-run, null, and non-real recovery facts", () => {
    const mutations = [
      { outcome: "failed" as const },
      { outcome: "not_run" as const },
      { recoveryPointRef: null },
      { receiptRef: null },
      { evidenceClass: "simulator" as const },
    ];
    for (const mutation of mutations) {
      const receipt = completeReceipt();
      const fact = receipt.recoveryFacts[0];
      if (fact === undefined) throw new Error("missing recovery fixture");
      expectSemanticRejection(
        {
          ...receipt,
          recoveryFacts: [{ ...fact, ...mutation }, ...receipt.recoveryFacts.slice(1)],
        },
        /incomplete or non-real recovery evidence|failed, not-run, incomplete, or non-real recovery/u,
      );
    }
  });

  test("rejects missing target-cohort recovery coverage", () => {
    const receipt = completeReceipt();
    const cohortRef = receipt.placementCohorts[2]?.cohortRef;
    if (cohortRef === undefined) throw new Error("missing cohort fixture");
    expectSemanticRejection(
      {
        ...receipt,
        recoveryFacts: receipt.recoveryFacts.filter((fact) => fact.cohortRef !== cohortRef),
      },
      /lacks recovery coverage/u,
    );
  });

  test("rejects an incomplete or failed required fault matrix", () => {
    const missing = completeReceipt();
    expectSemanticRejection(
      {
        ...missing,
        faultFacts: missing.faultFacts.slice(1),
      },
      /fault matrix is incomplete across the real placement cohorts/u,
    );

    const failed = completeReceipt();
    const fact = failed.faultFacts[0];
    if (fact === undefined) throw new Error("missing fault fixture");
    expectSemanticRejection(
      {
        ...failed,
        faultFacts: [{ ...fact, outcome: "failed" as const }, ...failed.faultFacts.slice(1)],
      },
      /incomplete or non-real fault evidence|failed, not-run, incomplete, or non-real fault/u,
    );
  });

  test("rejects simulated evidence classified as real", () => {
    const receipt = completeReceipt();
    const cohort = receipt.placementCohorts[1];
    if (cohort === undefined) throw new Error("missing cohort fixture");
    expectSemanticRejection(
      {
        ...receipt,
        placementCohorts: receipt.placementCohorts.map((current) =>
          current.cohortRef === cohort.cohortRef
            ? {
                ...current,
                adapter: { ...current.adapter, kind: "deterministic_simulator" as const },
              }
            : current,
        ),
      },
      /simulated evidence cannot be classified as real/u,
    );
  });

  test("rejects a stale candidate", () => {
    const receipt = completeReceipt();
    expect(() =>
      validateIdePortableEvidenceReceipt(receipt, {
        candidateCommitSha: "e".repeat(40),
        baseCommitSha: base,
      }),
    ).toThrow(/candidate is stale/u);
  });

  test("rejects reviewer, owner, and producer identity overlap", () => {
    const producerReview = completeReceipt();
    expectSemanticRejection(
      {
        ...producerReview,
        review: {
          ...producerReview.review,
          independentReviewerRef: producerReview.producerRef,
        },
      },
      /reviewer independent from producer and owner/u,
    );

    const ownerReview = completeReceipt();
    expectSemanticRejection(
      {
        ...ownerReview,
        review: {
          ...ownerReview.review,
          independentReviewerRef: ownerReview.review.ownerRef,
        },
      },
      /reviewer independent from producer and owner/u,
    );

    const producerOwner = completeReceipt();
    expectSemanticRejection(
      {
        ...producerOwner,
        review: { ...producerOwner.review, ownerRef: producerOwner.producerRef },
      },
      /owner distinct from the producer/u,
    );
  });

  test("keeps a non-acceptance receipt valid while real cohorts remain absent", () => {
    const receipt = completeReceipt();
    const absent = receipt.placementCohorts.map((cohort) => ({
      ...cohort,
      evidenceClass: "not_run" as const,
      journeyScope: "not_run" as const,
      journeys: {
        mainJourneyReceiptRef: null,
        failbackJourneyReceiptRef: null,
        faultMatrixReceiptRef: null,
      },
      adapter: { kind: "not_run" as const, ref: null, name: null, version: null },
      targetRef: null,
      artifact: { ref: null, sha256: null, bytes: null },
      phaseReceipts: cohort.phaseReceipts.map((phase) => ({
        ...phase,
        evidenceClass: "not_run" as const,
        receiptRef: null,
        operationRef: null,
        attachmentGeneration: null,
        result: "not_run" as const,
      })),
      metrics: [],
    }));
    const incomplete = decode({
      ...receipt,
      acceptanceRefs: {
        candidateRef: null,
        mainCommitSha: null,
        mainRef: null,
        artifactReceiptRef: null,
        rollbackReceiptRef: null,
        verificationCommandRef: null,
        verificationResultRef: null,
      },
      placementCohorts: absent,
      recoveryFacts: receipt.recoveryFacts.map((fact) => ({
        ...fact,
        evidenceClass: "not_run" as const,
        outcome: "not_run" as const,
        recoveryPointRef: null,
        receiptRef: null,
      })),
      faultFacts: [],
      review: {
        independentReviewerRef: null,
        independentDisposition: "not_run",
        independentDispositionRef: null,
        ownerRef: null,
        ownerDisposition: "not_run",
        ownerDispositionRef: null,
      },
      acceptancePassed: false,
      remainingGaps: ["The real placement cohorts did not run."],
    });
    expect(() => validate(incomplete)).not.toThrow();
  });
});
