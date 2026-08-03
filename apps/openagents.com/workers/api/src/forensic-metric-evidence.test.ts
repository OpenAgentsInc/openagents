import {
  CORRECTION_REJECTION_BURDEN_METRIC_REF,
  FROZEN_FORENSIC_METRIC_REGISTRY,
  FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
  FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
  FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION,
  FORENSIC_RUN_EVENT_VERSION,
  REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF,
  ForensicEvaluatorAdjudicationSchema,
  ForensicProviderUsageReceiptSchema,
  ForensicReviewerBurdenReceiptSchema,
  ForensicRunEventSchema,
  forensicSha256Digest,
  rebuildForensicScorecard,
  strictDecode,
  type ForensicMetricEventContext,
  type ForensicRunEvent,
} from "@openagentsinc/forensic-contract";
import { Effect } from "effect";
import { describe, expect, test } from "vite-plus/test";

import {
  makePostgresForensicMetricEvidenceStore,
  type ForensicMetricEvidenceSql,
} from "./forensic-metric-evidence";

const digest = (character: string) => `sha256:${character.repeat(64)}`;
const evaluatorRevisionDigest = digest("9");

const context = (overrides: Partial<ForensicMetricEventContext> = {}) => ({
  benchmarkRevisionDigest: digest("1"),
  datasetSplit: "development" as const,
  armRef: "arm.coldcard.complete-vulnerable",
  repetition: 1,
  targetDigest: digest("2"),
  sourceBundleDigest: digest("3"),
  promptDigest: digest("4"),
  modelDigest: digest("5"),
  modelParametersDigest: digest("6"),
  workerImageDigest: digest("7"),
  workerProfileDigest: digest("8"),
  sandboxRef: "sandbox.forensic.native-metrics",
  resourceGeneration: 1,
  evaluatorRevisionDigest,
  ...overrides,
});

const event = (
  runRef: string,
  metricContext: ForensicMetricEventContext,
  sequence: number,
  kind: ForensicRunEvent["kind"],
  observedAt: string,
  relatedRefs: ReadonlyArray<string> = [],
) =>
  strictDecode(ForensicRunEventSchema, {
    schema: FORENSIC_RUN_EVENT_VERSION,
    eventRef: `event.${runRef}.${sequence}`,
    runRef,
    sequence,
    kind,
    actorRef: "actor.forensic.native-driver",
    metricContext,
    relatedRefs,
    detailRefs: [],
    clock: "control_plane_server",
    observedAt,
  });

const makeFakeSqlStore = () => {
  const rows: Array<Record<string, unknown>> = [];
  const sql = (async (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
    const statement = strings.join("?");
    if (statement.includes("INSERT INTO forensic_metric_evidence")) {
      const eventInsert = statement.includes("WITH locked AS");
      const offset = eventInsert ? 3 : 0;
      const ownerRef = String(values[offset]);
      const runRef = String(values[offset + 1]);
      const recordRef = String(values[offset + 2]);
      const recordKind = String(values[offset + 3]);
      const eventSequence = eventInsert ? Number(values[offset + 4]) : null;
      const canonicalDigest = String(values[offset + (eventInsert ? 5 : 4)]);
      const payload = JSON.parse(String(values[offset + (eventInsert ? 6 : 5)])) as unknown;
      const observedAt = String(values[offset + (eventInsert ? 7 : 6)]);
      const existing = rows.find(
        (row) => row.owner_ref === ownerRef && row.record_ref === recordRef,
      );
      if (existing !== undefined) return [];
      if (eventInsert) {
        const expected =
          rows.filter(
            (row) =>
              row.owner_ref === ownerRef &&
              row.run_ref === runRef &&
              row.record_kind === "run_event",
          ).length + 1;
        if (eventSequence !== expected) return [];
      }
      const row = {
        owner_ref: ownerRef,
        run_ref: runRef,
        record_ref: recordRef,
        record_kind: recordKind,
        event_sequence: eventSequence,
        canonical_digest: canonicalDigest,
        payload_json: payload,
        observed_at: observedAt,
      };
      rows.push(row);
      return [row];
    }
    if (statement.includes("record_ref =")) {
      return rows.filter((row) => row.owner_ref === values[0] && row.record_ref === values[1]);
    }
    return rows.filter((row) => row.owner_ref === values[0] && row.run_ref === values[1]);
  }) as ForensicMetricEvidenceSql;
  return { rows, store: makePostgresForensicMetricEvidenceStore(sql) };
};

describe("native forensic metric evidence", () => {
  test("persists immutable events, unavailable usage, adjudication, and reviewer burden", async () => {
    const { store } = makeFakeSqlStore();
    const ownerRef = "owner.forensic.native-metrics";
    const runRef = "run.forensic.native-metrics.1";
    const metricContext = context();
    const events = [
      event(runRef, metricContext, 1, "analysis_started", "2026-08-03T16:00:00.000Z"),
      event(runRef, metricContext, 2, "turn_started", "2026-08-03T16:00:01.000Z"),
      event(runRef, metricContext, 3, "finding_submitted", "2026-08-03T16:02:00.000Z", [
        "finding.native.1",
      ]),
      event(runRef, metricContext, 4, "review_recorded", "2026-08-03T16:04:00.000Z"),
      event(runRef, metricContext, 5, "run_settled", "2026-08-03T16:05:00.000Z"),
    ];
    const usage = strictDecode(ForensicProviderUsageReceiptSchema, {
      schema: FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
      receiptRef: "receipt.provider.native.1",
      runRef,
      turnRef: "turn.native.1",
      role: "discovery",
      attempt: 1,
      startEventRef: events[1]!.eventRef,
      startEventSequence: 2,
      settledEventSequence: 5,
      abandoned: false,
      losingParallelArm: false,
      usage: {
        exactness: "unavailable",
        unavailableReasonRef: "reason.provider.usage-unavailable",
      },
      recordedAt: "2026-08-03T16:05:01.000Z",
    });
    const adjudication = strictDecode(ForensicEvaluatorAdjudicationSchema, {
      schema: FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
      adjudicationRef: "adjudication.native.1",
      runRef,
      findingRef: "finding.native.1",
      findingEventRef: events[2]!.eventRef,
      findingEventDigest: forensicSha256Digest(events[2]!),
      evaluatorRevisionDigest,
      outcome: "qualified",
      vulnerabilityIdentityDigest: digest("a"),
      requiredCausalLinks: 6,
      supportedCausalLinks: 6,
      submittedSourceRefs: 2,
      validSourceRefs: 2,
      reasonRefs: ["reason.frozen-evaluator.qualified"],
      evaluatedAt: "2026-08-03T16:04:00.000Z",
    });
    const reviewerBurden = strictDecode(ForensicReviewerBurdenReceiptSchema, {
      schema: FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION,
      receiptRef: "receipt.reviewer-burden.native.1",
      runRef,
      reviewerActorRef: "actor.forensic.independent-reviewer",
      reviewEventRef: events[3]!.eventRef,
      reviewEventSequence: 4,
      duration: { milliseconds: 60_000, exactness: "exact" },
      correctionCount: 1,
      rejectionCount: 0,
      reasonRefs: ["reason.reviewer.corrected-source-ref"],
      recordedAt: "2026-08-03T16:04:00.000Z",
    });

    for (const record of [...events, usage, adjudication, reviewerBurden]) {
      const append = await Effect.runPromise(store.append(ownerRef, record));
      expect(append.persisted).toBe(true);
      expect(append.duplicate).toBe(false);
    }
    await expect(Effect.runPromise(store.append(ownerRef, events[0]))).resolves.toMatchObject({
      duplicate: true,
    });
    await expect(
      Effect.runPromise(
        store.append(ownerRef, { ...events[0], observedAt: "2026-08-03T16:00:00.001Z" }),
      ),
    ).rejects.toMatchObject({ code: "record_conflict" });

    const retained = await Effect.runPromise(store.readRun(ownerRef, runRef));
    expect(retained.events).toHaveLength(5);
    expect(retained.usageReceipts[0]?.usage).toEqual({
      exactness: "unavailable",
      unavailableReasonRef: "reason.provider.usage-unavailable",
    });
    expect(retained.adjudications).toEqual([adjudication]);
    expect(retained.reviewerBurdenReceipts).toEqual([reviewerBurden]);
    expect((await Effect.runPromise(store.readRun("owner.other", runRef))).events).toHaveLength(0);

    const scorecard = rebuildForensicScorecard({
      scorecardRef: "scorecard.native.1",
      datasetRevisionDigest: digest("b"),
      evaluatorRevisionDigest,
      candidateDigest: digest("c"),
      registry: FROZEN_FORENSIC_METRIC_REGISTRY,
      hardGates: [
        {
          gateRef: "gate.native.complete-input",
          passed: true,
          evidenceRefs: ["evidence.coverage.complete"],
          blockerRefs: [],
        },
      ],
      runs: [
        {
          runRef,
          runDigest: digest("d"),
          armRef: metricContext.armRef,
          datasetSplit: metricContext.datasetSplit,
          population: "vulnerable",
          coverageStatus: "complete",
          events: retained.events,
          usageReceipts: retained.usageReceipts,
          adjudications: retained.adjudications,
          reviewerBurdenReceipts: retained.reviewerBurdenReceipts,
          retainedReceiptDigests: [digest("e")],
          failureRefs: [],
        },
      ],
      generatedAt: "2026-08-03T16:06:00.000Z",
    });
    const values = scorecard.runs[0]!.values;
    expect(
      values.find((value) => value.metricRef === REVIEWER_MINUTES_PER_QUALIFIED_FINDING_METRIC_REF),
    ).toMatchObject({ numericValue: 60_000, exactness: "exact" });
    expect(
      values.find((value) => value.metricRef === CORRECTION_REJECTION_BURDEN_METRIC_REF),
    ).toMatchObject({ numericValue: 1, exactness: "exact" });
    expect(scorecard.cost).toMatchObject({ exactness: "unavailable" });
  });

  test("rejects event gaps and keeps repeated runs isolated", async () => {
    const { store } = makeFakeSqlStore();
    const ownerRef = "owner.forensic.repetitions";
    const firstRun = "run.forensic.repetition.1";
    const secondRun = "run.forensic.repetition.2";
    const first = event(
      firstRun,
      context({ repetition: 1 }),
      1,
      "analysis_started",
      "2026-08-03T17:00:00.000Z",
    );
    const second = event(
      secondRun,
      context({ repetition: 2 }),
      1,
      "analysis_started",
      "2026-08-03T17:00:00.000Z",
    );
    await Effect.runPromise(store.append(ownerRef, first));
    await Effect.runPromise(store.append(ownerRef, second));
    await expect(
      Effect.runPromise(
        store.append(
          ownerRef,
          event(firstRun, context({ repetition: 1 }), 3, "run_settled", "2026-08-03T17:01:00.000Z"),
        ),
      ),
    ).rejects.toMatchObject({ code: "event_sequence_gap" });
    expect((await Effect.runPromise(store.readRun(ownerRef, firstRun))).events).toEqual([first]);
    expect((await Effect.runPromise(store.readRun(ownerRef, secondRun))).events).toEqual([second]);
  });

  test("the Cloud SQL adapter keeps inserts idempotent, dense, and digest-checked", async () => {
    const { rows, store } = makeFakeSqlStore();
    const ownerRef = "owner.forensic.postgres";
    const runRef = "run.forensic.postgres.1";
    const first = event(runRef, context(), 1, "request_accepted", "2026-08-03T18:00:00.000Z");
    await expect(Effect.runPromise(store.append(ownerRef, first))).resolves.toMatchObject({
      duplicate: false,
    });
    await expect(Effect.runPromise(store.append(ownerRef, first))).resolves.toMatchObject({
      duplicate: true,
    });
    await expect(
      Effect.runPromise(
        store.append(
          ownerRef,
          event(runRef, context(), 3, "run_settled", "2026-08-03T18:02:00.000Z"),
        ),
      ),
    ).rejects.toMatchObject({ code: "event_sequence_gap" });
    expect((await Effect.runPromise(store.readRun(ownerRef, runRef))).events).toEqual([first]);

    rows[0]!.canonical_digest = digest("0");
    await expect(Effect.runPromise(store.readRun(ownerRef, runRef))).rejects.toMatchObject({
      code: "invalid_evidence",
    });
  });
});
