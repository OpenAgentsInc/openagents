/**
 * The append-only `bench-results` store, and the receipts that make it one.
 *
 * Issue #34's contract says results append into `bench-results` with receipts.
 * A file you can append to is not yet append-only — nothing stops a later hand
 * from editing a row that made a release look bad, and a trend line whose
 * history can be quietly rewritten is worth less than no trend line. So every
 * row carries a receipt: a digest over that row's own facts AND the receipt of
 * the row before it. Change one figure in row 3 and rows 3..n stop verifying.
 * Delete row 3 and row 4's `previousReceipt` names a receipt that is no longer
 * in the file. Both are named breaks, not silent ones.
 *
 * This is deliberately a hash chain and not a signature. A signature would
 * answer "who wrote this", which needs a key this package has no business
 * holding; the chain answers "has this history been rewritten since it was
 * written", which is the question a benchmark trend actually asks. When a
 * signing seam exists, it signs the head receipt and the chain underneath it
 * still holds.
 *
 * THE UNKNOWNS SURVIVE THE ROUND TRIP. A row's cost is `null` when the run
 * could not be priced, exactly as the report says it. Writing 0 into the store
 * would launder an unpriced lane into a free one at the moment the figure stops
 * being read next to its reason, which is the whole failure this suite exists
 * to prevent. The disposition and the coverage travel with every row so a later
 * reader can tell "we did not measure this" from "this cost nothing".
 */

import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname } from "node:path";

import type {
  CostCoverage,
  CostPerAcceptedOutcomeDisposition,
  EffectivenessReport,
} from "./effectiveness.ts";
import type { RateBasis } from "./pricing.ts";
import type { CriterionVerdict, ThresholdGate } from "./thresholds.ts";

export const BENCH_RESULT_SCHEMA = "openagents.bench_result.v1";

/**
 * One graded run, flattened to the columns a trend or a lane comparison reads.
 *
 * The per-trial detail deliberately stays out. A results store is a row per
 * run, and a row that carries its whole job directory invites reading the store
 * as the archive of record, which it is not — the Harbor job directory is. What
 * is here is what two runs can be compared on.
 */
export interface BenchResultRow {
  readonly schema: typeof BENCH_RESULT_SCHEMA;
  readonly recordedAt: string;

  readonly suite: string;
  readonly lane: string;
  /** The report's pin over suite, lane, tasks, CLI version, model, and rates. */
  readonly runDigest: string;
  /**
   * The narrower pin two rows must share to be comparable at all: the suite,
   * the sorted task list, and the rate catalog. Lane, model, and CLI version
   * are excluded on purpose — those are the axes a comparison varies.
   */
  readonly suiteKey: string;
  readonly jobId: string | null;

  readonly models: ReadonlyArray<string>;
  readonly agentVersions: ReadonlyArray<string>;
  readonly rateCatalogVersion: string;
  readonly tasks: ReadonlyArray<string>;

  readonly trialsTotal: number;
  readonly accepted: number;
  readonly rejected: number;
  readonly ungraded: number;
  readonly graded: number;
  readonly successRate: number | null;
  readonly ungradedRatio: number;

  /** `null` when the run could not be priced. Never 0 for an unpriced lane. */
  readonly costPerAcceptedOutcomeUsd: number | null;
  readonly costDisposition: CostPerAcceptedOutcomeDisposition;
  readonly totalCostUsd: number | null;
  readonly costCoverage: CostCoverage;
  readonly rateBasis: RateBasis | null;

  readonly promptTokens: number | null;
  readonly completionTokens: number | null;
  readonly cachedInputTokens: number;
  readonly toolCalls: number | null;
  readonly wallClockSeconds: number | null;

  /** `null` when the run was scored without a thresholds file. */
  readonly gateStatus: CriterionVerdict | null;
  readonly thresholdsId: string | null;

  /** The receipt of the row before this one, or `null` for the first row. */
  readonly previousReceipt: string | null;
  /** `receipt:<sha256>` over every field above, this row's chain link. */
  readonly receipt: string;
}

/** The row minus its own receipt: what the receipt is computed over. */
type UnreceiptedRow = Omit<BenchResultRow, "receipt">;

export interface AppendOptions {
  /** The clock is injected so a test never reads one. */
  readonly recordedAt: string;
}

/**
 * The suite key: the run shape two rows must share before comparing them is
 * meaningful at all.
 *
 * Lane, model, and CLI version are left out because those are exactly what a
 * comparison varies. The rate catalog version is in, because a cost figure
 * computed from one catalog and a cost figure computed from another are not the
 * same measurement even when the tasks match.
 */
export const suiteKeyOf = (report: EffectivenessReport): string => {
  const source = JSON.stringify({
    suite: report.suite,
    tasks: report.perTrial.map((trial) => trial.task).toSorted(),
    rateCatalogVersion: report.rateCatalogVersion,
  });
  return `suite:${createHash("sha256").update(source).digest("hex")}`;
};

/** Build the row a report and its gate would append. Pure. */
export const buildResultRow = (
  report: EffectivenessReport,
  gate: ThresholdGate | null,
  previousReceipt: string | null,
  options: AppendOptions,
): BenchResultRow => {
  const unreceipted: UnreceiptedRow = {
    schema: BENCH_RESULT_SCHEMA,
    recordedAt: options.recordedAt,

    suite: report.suite,
    lane: report.lane,
    runDigest: report.runDigest,
    suiteKey: suiteKeyOf(report),
    jobId: report.jobId,

    models: report.models,
    agentVersions: report.agentVersions,
    rateCatalogVersion: report.rateCatalogVersion,
    tasks: report.perTrial.map((trial) => trial.task).toSorted(),

    trialsTotal: report.trialsTotal,
    accepted: report.accepted,
    rejected: report.rejected,
    ungraded: report.ungraded,
    graded: report.graded,
    successRate: report.successRate,
    ungradedRatio: report.ungradedRatio,

    costPerAcceptedOutcomeUsd: report.costPerAcceptedOutcome.usd,
    costDisposition: report.costPerAcceptedOutcome.disposition,
    totalCostUsd: report.cost.totalUsd,
    costCoverage: report.cost.coverage,
    rateBasis: report.cost.rateBasis,

    promptTokens: report.promptTokens,
    completionTokens: report.completionTokens,
    cachedInputTokens: report.cachedInputTokens,
    toolCalls: report.toolCalls,
    wallClockSeconds: report.wallClockSeconds,

    gateStatus: gate === null ? null : gate.status,
    thresholdsId: gate === null ? null : gate.thresholdsId,

    previousReceipt,
  };
  return { ...unreceipted, receipt: receiptOf(unreceipted) };
};

/**
 * The receipt.
 *
 * Serialised through an explicit key order rather than `JSON.stringify` on the
 * object, so a future field reordering does not silently invalidate every
 * receipt already written. Adding a field does invalidate them, which is
 * correct: a new column changes what the row asserts, and the schema string
 * carries the version that says so.
 */
export const receiptOf = (row: UnreceiptedRow): string => {
  const ordered = Object.keys(row)
    .toSorted()
    .map((key) => [key, (row as unknown as Record<string, unknown>)[key]] as const);
  const source = JSON.stringify(ordered);
  return `receipt:${createHash("sha256").update(source).digest("hex")}`;
};

export type ChainBreak =
  | { readonly kind: "receipt_mismatch"; readonly index: number; readonly detail: string }
  | { readonly kind: "chain_broken"; readonly index: number; readonly detail: string };

export type ChainVerdict =
  | { readonly ok: true; readonly rows: number; readonly head: string | null }
  | { readonly ok: false; readonly break: ChainBreak };

/**
 * Verify that a store has not been rewritten since it was written.
 *
 * Two distinct findings, because they mean different things. A
 * `receipt_mismatch` says a row's own contents no longer match its receipt —
 * somebody edited a figure in place. A `chain_broken` says a row's
 * `previousReceipt` does not name the row before it — somebody inserted,
 * removed, or reordered rows.
 */
export const verifyResultChain = (rows: ReadonlyArray<BenchResultRow>): ChainVerdict => {
  let previous: string | null = null;
  for (const [index, row] of rows.entries()) {
    const { receipt, ...unreceipted } = row;
    const expected = receiptOf(unreceipted);
    if (receipt !== expected) {
      return {
        ok: false,
        break: {
          kind: "receipt_mismatch",
          index,
          detail: `row ${String(index)} (${row.suite} on ${row.lane}, recorded ${row.recordedAt}) carries ${receipt} but its contents digest to ${expected}, so it was edited after it was written`,
        },
      };
    }
    if (row.previousReceipt !== previous) {
      return {
        ok: false,
        break: {
          kind: "chain_broken",
          index,
          detail: `row ${String(index)} follows ${String(row.previousReceipt)} but the row before it is ${String(previous)}, so rows were inserted, removed, or reordered`,
        },
      };
    }
    previous = receipt;
  }
  return { ok: true, rows: rows.length, head: previous };
};

/**
 * Read a store.
 *
 * A malformed line throws rather than being skipped. Skipping would turn a
 * corrupted store into a shorter, apparently valid one, and a trend that
 * silently drops the rows it could not parse is the worst of both worlds.
 * A store that does not exist yet reads as no rows, which is not a corruption.
 */
export const readResultRows = (storePath: string): ReadonlyArray<BenchResultRow> => {
  if (!existsSync(storePath)) return [];
  const lines = readFileSync(storePath, "utf8")
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "");
  return lines.map((line, index) => {
    let parsed: unknown;
    try {
      parsed = JSON.parse(line);
    } catch {
      throw new Error(`${storePath} line ${String(index + 1)} is not JSON`);
    }
    const row = parsed as BenchResultRow;
    if (row.schema !== BENCH_RESULT_SCHEMA) {
      throw new Error(
        `${storePath} line ${String(index + 1)} has schema ${String(row.schema)}, expected ${BENCH_RESULT_SCHEMA}`,
      );
    }
    return row;
  });
};

export type AppendRefusal = "duplicate_job" | "chain_broken";

export type AppendResult =
  | { readonly appended: true; readonly row: BenchResultRow }
  | { readonly appended: false; readonly refusal: AppendRefusal; readonly reason: string };

/**
 * Append one graded run to a store.
 *
 * Two refusals, both returned rather than thrown, because both are ordinary
 * operator situations rather than programming errors:
 *
 * - `duplicate_job` — this Harbor job is already in the store. Re-scoring a job
 *   with a different thresholds file is a useful thing to do and a second row
 *   is not what it produces; two rows for one execution would double-count it
 *   in every trend that follows.
 * - `chain_broken` — the existing store does not verify, so appending to it
 *   would extend a history that has already been rewritten and bury the break
 *   one row deeper.
 */
export const appendResultRow = (
  storePath: string,
  report: EffectivenessReport,
  gate: ThresholdGate | null,
  options: AppendOptions,
): AppendResult => {
  const rows = readResultRows(storePath);
  const verdict = verifyResultChain(rows);
  if (!verdict.ok) {
    return {
      appended: false,
      refusal: "chain_broken",
      reason: `${storePath} does not verify, so nothing was appended: ${verdict.break.detail}`,
    };
  }
  if (report.jobId !== null && rows.some((row) => row.jobId === report.jobId)) {
    return {
      appended: false,
      refusal: "duplicate_job",
      reason: `harbor job ${report.jobId} is already recorded in ${storePath}; re-scoring a run does not make it a second run`,
    };
  }

  const row = buildResultRow(report, gate, verdict.head, options);
  mkdirSync(dirname(storePath), { recursive: true });
  appendFileSync(storePath, `${JSON.stringify(row)}\n`, "utf8");
  return { appended: true, row };
};
