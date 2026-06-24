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
