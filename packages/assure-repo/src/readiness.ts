import type { SweepReceipt } from "./sweep.ts";

/**
 * AR-3 (issue #9059): the consuming readiness surface.
 *
 * Renders repo-verification readiness ONLY from a decoded, fresh sweep receipt.
 * The no-receipt-no-light rule is load-bearing: an absent or stale receipt
 * renders `unknown`, never `green`. This is the repo-scale form of the essay's
 * "no receipt means no light."
 */

export const DEFAULT_RECEIPT_MAX_AGE_MS = 24 * 60 * 60 * 1000;

export type ReadinessState = "green" | "red" | "unknown";

export type Readiness = {
  readonly state: ReadinessState;
  readonly reason: string;
  /** Echoed for display; never a substitute for the receipt itself. */
  readonly evidenceClass?: string;
  readonly commit?: string;
  readonly ageMs?: number;
};

/**
 * @param receipt the latest decoded sweep receipt, or undefined if none.
 * @param nowMs   current time in ms.
 * @param maxAgeMs freshness window; older receipts render unknown.
 */
export const renderReadiness = (
  receipt: SweepReceipt | undefined,
  nowMs: number,
  maxAgeMs: number = DEFAULT_RECEIPT_MAX_AGE_MS,
): Readiness => {
  if (receipt === undefined) {
    return { state: "unknown", reason: "no sweep receipt (no receipt means no light)" };
  }
  const generatedMs = Date.parse(receipt.generatedAt);
  if (Number.isNaN(generatedMs)) {
    return { state: "unknown", reason: "receipt has an unparseable generatedAt" };
  }
  const ageMs = nowMs - generatedMs;
  if (ageMs > maxAgeMs) {
    return {
      state: "unknown",
      reason: `sweep receipt is stale (${Math.round(ageMs / 3_600_000)}h > ${Math.round(maxAgeMs / 3_600_000)}h window)`,
      evidenceClass: receipt.evidenceClass,
      commit: receipt.commit,
      ageMs,
    };
  }
  if (receipt.overall === "green") {
    return {
      state: "green",
      reason: "latest sweep is green",
      evidenceClass: receipt.evidenceClass,
      commit: receipt.commit,
      ageMs,
    };
  }
  if (receipt.overall === "red") {
    return {
      state: "red",
      reason: "latest sweep is red",
      evidenceClass: receipt.evidenceClass,
      commit: receipt.commit,
      ageMs,
    };
  }
  return {
    state: "unknown",
    reason: "latest sweep is inconclusive",
    evidenceClass: receipt.evidenceClass,
    commit: receipt.commit,
    ageMs,
  };
};
