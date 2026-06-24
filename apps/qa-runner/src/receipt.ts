// Run = dereferenceable verified receipt (issue #6188, spec "run = verified
// receipt").
//
// A passing run already produces a public-safe, dereferenceable `result.json` +
// artifacts (the runner). This module adds, ADDITIVELY, a `receipt` to that
// result: a small, public-safe, dereferenceable receipt ref tied to the run's
// VERIFICATION CLASS — so a run "carries a receipt, not just files."
//
// Why a post-run helper instead of editing the runner: the runner's control
// flow is owned by a concurrent lane (which is additively adding a `verify`
// field). This receipt lives in its OWN namespace (`receipt`) on the result so
// the two additive fields merge trivially and neither lane edits the other's
// surface.
//
// Honesty / public-safety:
//   - The receipt's `verificationClass` is derived ONLY from what the run
//     actually proved (status + assertion outcomes). A failing run gets `none`;
//     a deterministic black-box pass with >=1 outcome assertion gets
//     `exact_trace_replay`; a pass with no assertions gets `seeded`. No
//     exactness inflation.
//   - The receipt is a ref + digest over the public-safe result, never a secret.
//     `assertPublicSafeResult` is re-run over the augmented result before write.

import { createHash } from "node:crypto";
import { join } from "node:path";
import { readFileSync, writeFileSync } from "node:fs";
import { Schema as S } from "effect";
import { assertPublicSafeResult, decodeQaRunResult, QaRunResult, type QaRunResult as QaRunResultType } from "./result";

export const QA_RUN_RECEIPT_SCHEMA_VERSION = "openagents.qa_runner.receipt.v1";

/** The verification class a run's receipt honestly carries. */
export const QaRunReceiptVerificationClass = S.Literals([
  "none",
  "seeded",
  "test_passed",
  "exact_trace_replay",
]);
export type QaRunReceiptVerificationClass = typeof QaRunReceiptVerificationClass.Type;

/**
 * The dereferenceable receipt attached to a run. Public-safe by construction:
 * refs + a digest over the public-safe result, never secrets.
 */
export const QaRunReceipt = S.Struct({
  schemaVersion: S.Literal(QA_RUN_RECEIPT_SCHEMA_VERSION),
  /** A url-safe, dereferenceable receipt ref (resolves to this run's result). */
  receiptRef: S.String,
  /** The honest verification class this run proved (tied to the receipt). */
  verificationClass: QaRunReceiptVerificationClass,
  /** sha256 (hex) over the public-safe result the receipt attests. */
  resultDigest: S.String,
  /** The number of outcome assertions that held (>=1 for a verified class). */
  assertionCount: S.Number,
  /** Where the result the receipt dereferences lives, relative to artifacts. */
  resultPath: S.Literal("result.json"),
});
export type QaRunReceipt = typeof QaRunReceipt.Type;

/**
 * A `QaRunResult` augmented with the additive `receipt` field. This is the
 * merge-trivial extension: it adds exactly one namespaced key and touches no
 * existing field. (A peer lane adds `verify` the same way.)
 */
export const QaRunResultWithReceipt = S.Struct({
  ...QaRunResult.fields,
  receipt: QaRunReceipt,
});
export type QaRunResultWithReceipt = typeof QaRunResultWithReceipt.Type;

const decodeWithReceipt = S.decodeUnknownSync(QaRunResultWithReceipt);

/** sha256 hex digest over the canonical JSON of the (public-safe) result. */
function digestResult(result: QaRunResultType): string {
  return createHash("sha256").update(JSON.stringify(result)).digest("hex");
}

/**
 * Honestly grade the run from what it actually proved. A failing run never
 * carries a verified class; a passing run with outcome assertions is an
 * exact (deterministic black-box) replay; a passing run with no assertions is
 * only `seeded` (it ran, but asserted nothing a user cares about).
 */
function honestVerificationClass(
  result: QaRunResultType,
  assertionCount: number,
): QaRunReceiptVerificationClass {
  if (result.status === "fail") return "none";
  return assertionCount >= 1 ? "exact_trace_replay" : "seeded";
}

const slugifyRef = (text: string): string =>
  text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 48) || "run";

/**
 * Build the dereferenceable receipt for a run result. Pure; deterministic for a
 * fixed result. The `receiptRef` is a stable, public-safe handle derived from
 * the target name + the result digest, so the same run always dereferences to
 * the same receipt.
 */
export function buildQaRunReceipt(result: QaRunResultType): QaRunReceipt {
  const assertionCount = result.steps.filter(
    step => step.kind === "assert" && step.status === "ok",
  ).length;
  const resultDigest = digestResult(result);
  const verificationClass = honestVerificationClass(result, assertionCount);
  const receiptRef = `receipt:qa_runner:${slugifyRef(result.target.name)}:${resultDigest.slice(0, 16)}`;
  return {
    schemaVersion: QA_RUN_RECEIPT_SCHEMA_VERSION,
    receiptRef,
    verificationClass,
    resultDigest,
    assertionCount,
    resultPath: "result.json",
  };
}

/** Attach the additive `receipt` field to a result (pure; no I/O). */
export function attachReceipt(result: QaRunResultType): QaRunResultWithReceipt {
  const receipt = buildQaRunReceipt(result);
  const augmented = { ...result, receipt } satisfies QaRunResultWithReceipt;
  // Tripwire: the augmented result must still be public-safe (the receipt adds
  // only refs/digests, never a secret).
  assertPublicSafeResult(augmented);
  return augmented;
}

/**
 * Post-run path (the part of the pipeline this lane owns): read the result.json
 * the runner already wrote, attach the additive `receipt`, and write it back.
 * Idempotent: re-running produces the same receipt for the same result (the
 * digest is computed over the result WITHOUT a prior receipt).
 *
 * Returns the receipt so a caller (CLI / distiller) can surface the ref.
 */
export function writeReceiptForRun(artifactDir: string): QaRunReceipt {
  const resultPath = join(artifactDir, "result.json");
  const raw = JSON.parse(readFileSync(resultPath, "utf8")) as Record<string, unknown>;
  // Compute the receipt over the result WITHOUT any pre-existing receipt so the
  // digest is stable across re-runs (idempotent) and never self-referential.
  const { receipt: _existing, ...withoutReceipt } = raw;
  const baseResult = decodeQaRunResult(withoutReceipt);
  const augmented = attachReceipt(baseResult);
  writeFileSync(resultPath, `${JSON.stringify(augmented, null, 2)}\n`);
  return augmented.receipt;
}

/** Decode + validate a result.json that carries the additive receipt. */
export const decodeQaRunResultWithReceipt = (value: unknown): QaRunResultWithReceipt =>
  decodeWithReceipt(value);
