import {
  FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
  FORENSIC_INFRASTRUCTURE_COST_RECEIPT_VERSION,
  FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
  FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION,
  FORENSIC_RUN_EVENT_VERSION,
  ForensicEvaluatorAdjudicationSchema,
  ForensicInfrastructureCostReceiptSchema,
  ForensicProviderUsageReceiptSchema,
  ForensicReviewerBurdenReceiptSchema,
  ForensicRunEventSchema,
  evaluateForensicEventSequence,
  forensicSha256Digest,
  strictDecode,
  type ForensicEvaluatorAdjudication,
  type ForensicInfrastructureCostReceipt,
  type ForensicProviderUsageReceipt,
  type ForensicReviewerBurdenReceipt,
  type ForensicRunEvent,
} from "@openagentsinc/forensic-contract";
import { Data, Effect } from "effect";

export const FORENSIC_METRIC_EVIDENCE_LEDGER_VERSION =
  "openagents.forensic_metric_evidence_ledger.v1" as const;

export type ForensicMetricEvidence =
  | ForensicRunEvent
  | ForensicProviderUsageReceipt
  | ForensicEvaluatorAdjudication
  | ForensicReviewerBurdenReceipt
  | ForensicInfrastructureCostReceipt;

export type ForensicMetricEvidenceKind =
  | "run_event"
  | "provider_usage"
  | "adjudication"
  | "reviewer_burden"
  | "infrastructure_cost";

export type ForensicMetricEvidenceAppend = Readonly<{
  schema: typeof FORENSIC_METRIC_EVIDENCE_LEDGER_VERSION;
  ownerRef: string;
  runRef: string;
  recordRef: string;
  recordKind: ForensicMetricEvidenceKind;
  canonicalDigest: string;
  duplicate: boolean;
  persisted: true;
}>;

export type ForensicMetricRunEvidence = Readonly<{
  schema: typeof FORENSIC_METRIC_EVIDENCE_LEDGER_VERSION;
  ownerRef: string;
  runRef: string;
  events: ReadonlyArray<ForensicRunEvent>;
  usageReceipts: ReadonlyArray<ForensicProviderUsageReceipt>;
  adjudications: ReadonlyArray<ForensicEvaluatorAdjudication>;
  reviewerBurdenReceipts: ReadonlyArray<ForensicReviewerBurdenReceipt>;
  infrastructureCostReceipts: ReadonlyArray<ForensicInfrastructureCostReceipt>;
  eventDigest: string;
  receiptDigest: string;
}>;

export class ForensicMetricEvidenceError extends Data.TaggedError("ForensicMetricEvidenceError")<{
  readonly code:
    | "invalid_evidence"
    | "record_conflict"
    | "event_sequence_gap"
    | "storage_unavailable";
  readonly message: string;
  readonly retryable: boolean;
}> {}

export type ForensicMetricEvidenceStore = Readonly<{
  append: (
    ownerRef: string,
    evidence: unknown,
  ) => Effect.Effect<ForensicMetricEvidenceAppend, ForensicMetricEvidenceError>;
  readRun: (
    ownerRef: string,
    runRef: string,
  ) => Effect.Effect<ForensicMetricRunEvidence, ForensicMetricEvidenceError>;
}>;

export type ForensicMetricEvidenceSql = <T = Record<string, unknown>>(
  strings: TemplateStringsArray,
  ...values: ReadonlyArray<unknown>
) => Promise<Array<T>>;

type DecodedEvidence = Readonly<{
  evidence: ForensicMetricEvidence;
  runRef: string;
  recordRef: string;
  recordKind: ForensicMetricEvidenceKind;
  sequence: number | null;
  observedAt: string;
  canonicalDigest: string;
}>;

type EvidenceRow = Readonly<{
  owner_ref: string;
  run_ref: string;
  record_ref: string;
  record_kind: ForensicMetricEvidenceKind;
  event_sequence: number | string | null;
  canonical_digest: string;
  payload_json: unknown;
  observed_at: string;
}>;

const invalidEvidence = (message: string) =>
  new ForensicMetricEvidenceError({
    code: "invalid_evidence",
    message,
    retryable: false,
  });

const decodeEvidence = (value: unknown): DecodedEvidence => {
  if (typeof value !== "object" || value === null || !("schema" in value)) {
    throw invalidEvidence("forensic metric evidence requires a typed schema");
  }
  const schema = (value as { schema?: unknown }).schema;
  let evidence: ForensicMetricEvidence;
  let recordRef: string;
  let recordKind: ForensicMetricEvidenceKind;
  let sequence: number | null;
  let observedAt: string;
  try {
    switch (schema) {
      case FORENSIC_RUN_EVENT_VERSION: {
        const event = strictDecode(ForensicRunEventSchema, value);
        evidence = event;
        recordRef = event.eventRef;
        recordKind = "run_event";
        sequence = event.sequence;
        observedAt = event.observedAt;
        break;
      }
      case FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION: {
        const receipt = strictDecode(ForensicProviderUsageReceiptSchema, value);
        evidence = receipt;
        recordRef = receipt.receiptRef;
        recordKind = "provider_usage";
        sequence = null;
        observedAt = receipt.recordedAt;
        break;
      }
      case FORENSIC_EVALUATOR_ADJUDICATION_VERSION: {
        const adjudication = strictDecode(ForensicEvaluatorAdjudicationSchema, value);
        evidence = adjudication;
        recordRef = adjudication.adjudicationRef;
        recordKind = "adjudication";
        sequence = null;
        observedAt = adjudication.evaluatedAt;
        break;
      }
      case FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION: {
        const receipt = strictDecode(ForensicReviewerBurdenReceiptSchema, value);
        evidence = receipt;
        recordRef = receipt.receiptRef;
        recordKind = "reviewer_burden";
        sequence = null;
        observedAt = receipt.recordedAt;
        break;
      }
      case FORENSIC_INFRASTRUCTURE_COST_RECEIPT_VERSION: {
        const receipt = strictDecode(ForensicInfrastructureCostReceiptSchema, value);
        evidence = receipt;
        recordRef = receipt.receiptRef;
        recordKind = "infrastructure_cost";
        sequence = null;
        observedAt = receipt.recordedAt;
        break;
      }
      default:
        throw invalidEvidence("unsupported forensic metric evidence schema");
    }
  } catch (error) {
    if (error instanceof ForensicMetricEvidenceError) throw error;
    throw invalidEvidence("forensic metric evidence failed strict validation");
  }
  return {
    evidence,
    runRef: evidence.runRef,
    recordRef,
    recordKind,
    sequence,
    observedAt,
    canonicalDigest: forensicSha256Digest(evidence),
  };
};

const decodePayload = (payload: unknown): unknown => {
  if (typeof payload === "string") {
    try {
      return JSON.parse(payload) as unknown;
    } catch {
      throw invalidEvidence("stored forensic metric evidence is not valid JSON");
    }
  }
  return payload;
};

const projectRun = (
  ownerRef: string,
  runRef: string,
  records: ReadonlyArray<ForensicMetricEvidence>,
): ForensicMetricRunEvidence => {
  if (records.some((record) => record.runRef !== runRef)) {
    throw invalidEvidence("stored forensic metric evidence crossed its run boundary");
  }
  const events = records
    .filter((record): record is ForensicRunEvent => record.schema === FORENSIC_RUN_EVENT_VERSION)
    .sort((left, right) => left.sequence - right.sequence);
  const sequence = events.length === 0 ? undefined : evaluateForensicEventSequence(events);
  if (sequence?._tag === "Invalid") {
    throw new ForensicMetricEvidenceError({
      code: "event_sequence_gap",
      message: `stored forensic metric events are not dense: ${sequence.blockerRef}`,
      retryable: false,
    });
  }
  const usageReceipts = records.filter(
    (record): record is ForensicProviderUsageReceipt =>
      record.schema === FORENSIC_PROVIDER_USAGE_RECEIPT_VERSION,
  );
  const adjudications = records.filter(
    (record): record is ForensicEvaluatorAdjudication =>
      record.schema === FORENSIC_EVALUATOR_ADJUDICATION_VERSION,
  );
  const reviewerBurdenReceipts = records.filter(
    (record): record is ForensicReviewerBurdenReceipt =>
      record.schema === FORENSIC_REVIEWER_BURDEN_RECEIPT_VERSION,
  );
  const infrastructureCostReceipts = records.filter(
    (record): record is ForensicInfrastructureCostReceipt =>
      record.schema === FORENSIC_INFRASTRUCTURE_COST_RECEIPT_VERSION,
  );
  return {
    schema: FORENSIC_METRIC_EVIDENCE_LEDGER_VERSION,
    ownerRef,
    runRef,
    events,
    usageReceipts,
    adjudications,
    reviewerBurdenReceipts,
    infrastructureCostReceipts,
    eventDigest: forensicSha256Digest([...events, ...adjudications]),
    receiptDigest: forensicSha256Digest([
      ...usageReceipts,
      ...reviewerBurdenReceipts,
      ...infrastructureCostReceipts,
    ]),
  };
};

const appendProjection = (
  ownerRef: string,
  decoded: DecodedEvidence,
  duplicate: boolean,
): ForensicMetricEvidenceAppend => ({
  schema: FORENSIC_METRIC_EVIDENCE_LEDGER_VERSION,
  ownerRef,
  runRef: decoded.runRef,
  recordRef: decoded.recordRef,
  recordKind: decoded.recordKind,
  canonicalDigest: decoded.canonicalDigest,
  duplicate,
  persisted: true,
});

export const makePostgresForensicMetricEvidenceStore = (
  sql: ForensicMetricEvidenceSql,
): ForensicMetricEvidenceStore => ({
  append: (ownerRef, value) =>
    Effect.gen(function* () {
      const decoded = yield* Effect.try({
        try: () => decodeEvidence(value),
        catch: (error) =>
          error instanceof ForensicMetricEvidenceError
            ? error
            : invalidEvidence("forensic metric evidence failed strict validation"),
      });
      const inserted = yield* Effect.tryPromise({
        try: () =>
          decoded.recordKind === "run_event"
            ? sql<EvidenceRow>`
                WITH locked AS (
                  SELECT pg_advisory_xact_lock(
                    hashtextextended(${`${ownerRef}:${decoded.runRef}:run_event`}, 0)
                  )
                ), next_sequence AS (
                  SELECT COALESCE(MAX(event_sequence), 0) + 1 AS value
                    FROM forensic_metric_evidence, locked
                   WHERE owner_ref = ${ownerRef}
                     AND run_ref = ${decoded.runRef}
                     AND record_kind = 'run_event'
                )
                INSERT INTO forensic_metric_evidence
                  (owner_ref, run_ref, record_ref, record_kind, event_sequence,
                   canonical_digest, payload_json, observed_at, created_at)
                SELECT ${ownerRef}, ${decoded.runRef}, ${decoded.recordRef},
                       ${decoded.recordKind}, ${decoded.sequence}, ${decoded.canonicalDigest},
                       ${JSON.stringify(decoded.evidence)}::jsonb, ${decoded.observedAt}, NOW()
                  FROM next_sequence
                 WHERE value = ${decoded.sequence}
                ON CONFLICT (owner_ref, record_ref) DO NOTHING
                RETURNING *
              `
            : sql<EvidenceRow>`
                INSERT INTO forensic_metric_evidence
                  (owner_ref, run_ref, record_ref, record_kind, event_sequence,
                   canonical_digest, payload_json, observed_at, created_at)
                VALUES (${ownerRef}, ${decoded.runRef}, ${decoded.recordRef},
                        ${decoded.recordKind}, NULL, ${decoded.canonicalDigest},
                        ${JSON.stringify(decoded.evidence)}::jsonb, ${decoded.observedAt}, NOW())
                ON CONFLICT (owner_ref, record_ref) DO NOTHING
                RETURNING *
              `,
        catch: () =>
          new ForensicMetricEvidenceError({
            code: "storage_unavailable",
            message: "forensic metric evidence storage is unavailable",
            retryable: true,
          }),
      });
      if (inserted[0] !== undefined) return appendProjection(ownerRef, decoded, false);
      const existing = yield* Effect.tryPromise({
        try: () => sql<EvidenceRow>`
          SELECT * FROM forensic_metric_evidence
           WHERE owner_ref = ${ownerRef}
             AND record_ref = ${decoded.recordRef}
           LIMIT 1
        `,
        catch: () =>
          new ForensicMetricEvidenceError({
            code: "storage_unavailable",
            message: "forensic metric evidence storage is unavailable",
            retryable: true,
          }),
      });
      if (existing[0]?.canonical_digest === decoded.canonicalDigest) {
        return appendProjection(ownerRef, decoded, true);
      }
      if (existing[0] !== undefined) {
        return yield* new ForensicMetricEvidenceError({
          code: "record_conflict",
          message: "forensic metric record ref already binds different immutable bytes",
          retryable: false,
        });
      }
      return yield* new ForensicMetricEvidenceError({
        code: "event_sequence_gap",
        message: "forensic metric event sequence is not the next dense sequence",
        retryable: false,
      });
    }),
  readRun: (ownerRef, runRef) =>
    Effect.gen(function* () {
      const rows = yield* Effect.tryPromise({
        try: () => sql<EvidenceRow>`
          SELECT * FROM forensic_metric_evidence
           WHERE owner_ref = ${ownerRef}
             AND run_ref = ${runRef}
           ORDER BY observed_at ASC, record_ref ASC
        `,
        catch: () =>
          new ForensicMetricEvidenceError({
            code: "storage_unavailable",
            message: "forensic metric evidence storage is unavailable",
            retryable: true,
          }),
      });
      return yield* Effect.try({
        try: () => {
          const records = rows.map((row) => {
            const decoded = decodeEvidence(decodePayload(row.payload_json));
            if (
              row.owner_ref !== ownerRef ||
              row.run_ref !== runRef ||
              row.record_ref !== decoded.recordRef ||
              row.record_kind !== decoded.recordKind ||
              row.canonical_digest !== decoded.canonicalDigest ||
              (decoded.sequence === null
                ? row.event_sequence !== null
                : Number(row.event_sequence) !== decoded.sequence)
            ) {
              throw invalidEvidence("stored forensic metric evidence row drifted");
            }
            return decoded.evidence;
          });
          return projectRun(ownerRef, runRef, records);
        },
        catch: (error) =>
          error instanceof ForensicMetricEvidenceError
            ? error
            : invalidEvidence("stored forensic metric evidence failed validation"),
      });
    }),
});
