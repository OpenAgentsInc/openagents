// The run result model — a dereferenceable, public-safe receipt.
//
// result.json + artifacts let a reviewer confirm a run by reading the result and
// watching the video, with NO local run. The schema is deliberately small and
// PUBLIC-SAFE: it MUST NOT carry secrets, tokens, prompts, cookie values, or raw
// credentials. `assertPublicSafeResult` is the tripwire enforced by the runner
// and by a dedicated test.

import { Schema as S } from "effect";

export const QaRunStepStatus = S.Literals(["ok", "failed"]);
export type QaRunStepStatus = typeof QaRunStepStatus.Type;

export const QaRunStep = S.Struct({
  index: S.Number,
  kind: S.String,
  label: S.String,
  status: QaRunStepStatus,
  detail: S.optional(S.Record(S.String, S.Union([S.String, S.Number, S.Boolean]))),
});
export type QaRunStep = typeof QaRunStep.Type;

// ---------------------------------------------------------------------------
// Verify stage (#6192): the investigator verdict, an ADDITIVE field.
// ---------------------------------------------------------------------------
//
// `verify` is an ADDITIVE, namespaced field on the result. It does NOT rename or
// remove any existing field. A separate lane is adding a distinct additive
// `receipt` field; keeping `verify` self-contained makes that merge trivial.
//
// The verdict mirrors the Tassadar verification-class vocabulary
// (CONFIRMED/REFUTED/INCONCLUSIVE). Per-commitment findings carry the OBSERVED
// evidence summary so a reviewer sees WHY a verdict landed with no local run.
// Public-safe: only labels/claims/evidence summaries (no prompts/tokens) — the
// tripwire below also re-checks it on write.

export const QaVerifyVerdict = S.Literals(["CONFIRMED", "REFUTED", "INCONCLUSIVE"]);
export type QaVerifyVerdict = typeof QaVerifyVerdict.Type;

export const QaVerifyFinding = S.Struct({
  id: S.String,
  claim: S.String,
  verdict: QaVerifyVerdict,
  evidenceSummary: S.String,
});
export type QaVerifyFinding = typeof QaVerifyFinding.Type;

export const QaVerify = S.Struct({
  verdict: QaVerifyVerdict,
  findings: S.Array(QaVerifyFinding),
  /** True only when every finding rests on observed evidence (anti-fabrication). */
  observed: S.Boolean,
});
export type QaVerify = typeof QaVerify.Type;

export const QaRunArtifacts = S.Struct({
  /** Relative path to the playable video (mp4 or webm). */
  video: S.optional(S.String),
  videoFormat: S.optional(S.Literals(["mp4", "webm"])),
  /** Relative path to the Playwright trace zip. */
  trace: S.optional(S.String),
  /** Relative paths to per-step screenshots. */
  screenshots: S.Array(S.String),
});
export type QaRunArtifacts = typeof QaRunArtifacts.Type;

export const QaRunResult = S.Struct({
  schemaVersion: S.Literal("openagents.qa_runner.result.v1"),
  status: S.Literals(["pass", "fail"]),
  target: S.Struct({ name: S.String, baseUrl: S.String }),
  brain: S.String,
  backend: S.String,
  startedAt: S.String,
  endedAt: S.String,
  durationMs: S.Number,
  steps: S.Array(QaRunStep),
  artifacts: QaRunArtifacts,
  /** Honest failure summary when status is "fail". */
  failure: S.optional(S.String),
  /** ADDITIVE (#6192): the verify-stage investigator verdict, when a run
   *  declared commitments. Namespaced + self-contained so it does not collide
   *  with the separate additive `receipt` field another lane is adding. */
  verify: S.optional(QaVerify),
});
export type QaRunResult = typeof QaRunResult.Type;

export const decodeQaRunResult = S.decodeUnknownSync(QaRunResult);

/**
 * Forbidden substrings/keys that must never appear in a public-safe result.
 * This is a tripwire, not a security boundary — surfaces already withhold
 * secrets at the source; this catches regressions.
 */
const FORBIDDEN_KEY_PATTERNS = [
  /token/i,
  /secret/i,
  /password/i,
  /passwd/i,
  /cookie/i,
  /authorization/i,
  /bearer/i,
  /api[-_]?key/i,
  /prompt/i,
  /credential/i,
];

export class PublicSafetyViolation extends Error {
  constructor(reason: string) {
    super(`public_safety_violation: ${reason}`);
    this.name = "PublicSafetyViolation";
  }
}

const FORBIDDEN_TEXT_PATTERNS = [
  /\/Users\//i,
  /\/home\//i,
  /~\//,
  /auth\.json/i,
  /bearer\s+[a-z0-9._=-]+/i,
  /sk-[a-z0-9]/i,
  /raw[_-]?(prompt|trace|log|provider)/i,
  /provider[_-]?payload/i,
  /api[_-]?key/i,
  /credential/i,
  /secret/i,
];

export function assertPublicSafeText(text: string, path = "$"): void {
  for (const pattern of FORBIDDEN_TEXT_PATTERNS) {
    if (pattern.test(text)) {
      throw new PublicSafetyViolation(`forbidden text at ${path}`);
    }
  }
}

export function assertPublicSafeTextValues(value: unknown, path = "$"): void {
  if (typeof value === "string") {
    assertPublicSafeText(value, path);
    return;
  }
  if (value === null || typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((v, i) => assertPublicSafeTextValues(v, `${path}[${i}]`));
    return;
  }
  for (const [key, v] of Object.entries(value as Record<string, unknown>)) {
    assertPublicSafeText(key, `${path}.${key}:key`);
    assertPublicSafeTextValues(v, `${path}.${key}`);
  }
}

/**
 * Assert a result (any JSON-like value) carries no forbidden fields. Walks the
 * object graph checking KEYS against the forbidden patterns. Throws
 * `PublicSafetyViolation` on the first hit.
 */
export function assertPublicSafeResult(value: unknown, path = "$"): void {
  if (value === null || typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((v, i) => assertPublicSafeResult(v, `${path}[${i}]`));
    return;
  }
  for (const [key, v] of Object.entries(value as Record<string, unknown>)) {
    for (const pattern of FORBIDDEN_KEY_PATTERNS) {
      if (pattern.test(key)) {
        throw new PublicSafetyViolation(`forbidden field "${key}" at ${path}`);
      }
    }
    assertPublicSafeResult(v, `${path}.${key}`);
  }
}
