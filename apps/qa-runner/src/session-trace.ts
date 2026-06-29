// KhalaSessionTrace (spec §C): a deterministic, replayable, public-safe capture
// of a Khala-driven computer-use session — the distiller's input.
//
// The trace is the typed lowering of a session: ordered beats (chat_turn /
// tool_call / browser / terminal / verdict) with raw text / output / secrets
// WITHHELD (refs/hashes/neutral classifiers only), inferred typed inputs/outputs,
// receipts, and a content digest over the ordered beats. It rides the existing
// computer-use timeline discipline (selectors-as-intent, no raw secrets) and the
// result.json public-safety tripwire.
//
// Determinism (spec §C.3): the same ordered beats re-derive the same digest;
// waits are conditions, never sleeps. `assertSessionTracePublicSafe` is the
// tripwire (a dedicated test pins it).

import { createHash } from "node:crypto";
import { Schema as S } from "effect";
import { assertPublicSafeResult, PublicSafetyViolation } from "./result";

export const SESSION_TRACE_SCHEMA_VERSION = "openagents.khala.session_trace.v1";

/** A typed I/O field inferred from the session goal/observations. */
export const TypedField = S.Struct({
  name: S.String,
  /** A concrete type name — never "any" (the distiller acceptance bar). */
  type: S.String,
  /** Optional neutral description (NO raw values/secrets). */
  description: S.optional(S.String),
});
export type TypedField = typeof TypedField.Type;

export const SessionBeat = S.Union([
  S.Struct({
    kind: S.Literal("chat_turn"),
    role: S.Literals(["system", "user", "assistant"]),
    /** Ref/hash of the content — NEVER the raw text (may carry prompts). */
    contentRef: S.String,
  }),
  S.Struct({
    kind: S.Literal("tool_call"),
    tool: S.String,
    /** Hash of the args — NEVER raw args (may carry typed text). */
    argsHash: S.String,
    effect: S.Literals(["read", "mutate", "spend"]),
  }),
  S.Struct({
    kind: S.Literal("browser"),
    action: S.Literals(["navigate", "click", "type", "wait", "screenshot", "assert", "readText"]),
    /** A neutral target hint (a path/role/selector-as-intent), never a secret. */
    targetHint: S.String,
    status: S.Literals(["ok", "failed"]),
  }),
  S.Struct({
    kind: S.Literal("terminal"),
    /** Hash of the command — NEVER the raw command (may carry secrets). */
    commandHash: S.String,
    outcome: S.Literals(["ok", "fail"]),
  }),
  S.Struct({
    kind: S.Literal("verdict"),
    verificationClass: S.Literals(["none", "seeded", "test_passed", "exact_trace_replay", "failed"]),
  }),
]);
export type SessionBeat = typeof SessionBeat.Type;

export const KhalaSessionTrace = S.Struct({
  schemaVersion: S.Literal(SESSION_TRACE_SCHEMA_VERSION),
  /** The user-stated goal of the session. */
  goal: S.String,
  /** The target the session ran against (name + baseUrl; no secrets). */
  target: S.Struct({ name: S.String, baseUrl: S.String }),
  /** The model that drove the session (one model: openagents/khala). */
  model: S.String,
  beats: S.Array(SessionBeat),
  /** Inferred typed inputs of the task (no `any`). */
  inputs: S.Array(TypedField),
  /** Inferred typed outputs / acceptance of the task. */
  outputs: S.Array(TypedField),
  /** Replay/acceptance receipt refs. */
  receipts: S.Array(S.String),
  /** Content digest of the ordered beats (sha256, hex). */
  digest: S.String,
});
export type KhalaSessionTrace = typeof KhalaSessionTrace.Type;

export const decodeSessionTrace = S.decodeUnknownSync(KhalaSessionTrace);

/** Short stable sha256 hex of a string (for refs/hashes; never reversible). */
export function shortHash(text: string): string {
  return createHash("sha256").update(text).digest("hex").slice(0, 16);
}

/**
 * Compute the content digest over the ordered beats. Deterministic: the same
 * ordered beats always yield the same digest (the replay-determinism property).
 */
export function computeDigest(beats: ReadonlyArray<SessionBeat>): string {
  return createHash("sha256").update(JSON.stringify(beats)).digest("hex");
}

export interface MakeSessionTraceInput {
  readonly goal: string;
  readonly target: { readonly name: string; readonly baseUrl: string };
  readonly model: string;
  readonly beats: ReadonlyArray<SessionBeat>;
  readonly inputs: ReadonlyArray<TypedField>;
  readonly outputs: ReadonlyArray<TypedField>;
  readonly receipts?: ReadonlyArray<string>;
}

/** Build a `KhalaSessionTrace`, computing the digest from the beats. */
export function makeSessionTrace(input: MakeSessionTraceInput): KhalaSessionTrace {
  const beats = [...input.beats];
  return {
    schemaVersion: SESSION_TRACE_SCHEMA_VERSION,
    goal: input.goal,
    target: { name: input.target.name, baseUrl: input.target.baseUrl },
    model: input.model,
    beats,
    inputs: [...input.inputs],
    outputs: [...input.outputs],
    receipts: [...(input.receipts ?? [])],
    digest: computeDigest(beats),
  };
}

/**
 * Re-derive the digest from a trace's beats and confirm it matches the recorded
 * digest. This is the deterministic-replay check for the trace's ordered beats.
 */
export function verifyTraceDigest(trace: KhalaSessionTrace): boolean {
  return computeDigest(trace.beats) === trace.digest;
}

/**
 * Patterns that must NEVER appear as raw values inside a trace. The schema
 * already withholds raw text via *Ref/*Hash fields, but this tripwire catches a
 * regression (e.g. a future field accidentally carrying a path/url with a token).
 */
const FORBIDDEN_VALUE_PATTERNS: ReadonlyArray<RegExp> = [
  /bearer\s+[a-z0-9._-]{12,}/i,
  /sk-[a-z0-9]{16,}/i,
  /\bpassword\b\s*[:=]/i,
  /eyJ[a-zA-Z0-9._-]{20,}/, // JWT-ish
];

export class SessionTracePublicSafetyViolation extends Error {
  constructor(reason: string) {
    super(`session_trace_public_safety_violation: ${reason}`);
    this.name = "SessionTracePublicSafetyViolation";
  }
}

/**
 * Tripwire (spec §C.3): assert the trace carries no secrets. Two checks:
 *   1. forbidden KEYS (token/secret/cookie/...) anywhere — reuses the result
 *      tripwire.
 *   2. forbidden VALUE patterns (bearer/JWT/sk-...) in any string field.
 * Throws on the first hit; never a fabricated pass.
 */
export function assertSessionTracePublicSafe(trace: unknown): void {
  // 1. forbidden keys (reuse the result-level walker).
  try {
    assertPublicSafeResult(trace);
  } catch (error) {
    if (error instanceof PublicSafetyViolation) {
      throw new SessionTracePublicSafetyViolation(error.message);
    }
    throw error;
  }
  // 2. forbidden value patterns in any string.
  walkStrings(trace, (value, path) => {
    for (const pattern of FORBIDDEN_VALUE_PATTERNS) {
      if (pattern.test(value)) {
        throw new SessionTracePublicSafetyViolation(`forbidden value pattern at ${path}`);
      }
    }
  });
}

function walkStrings(value: unknown, visit: (value: string, path: string) => void, path = "$"): void {
  if (typeof value === "string") {
    visit(value, path);
    return;
  }
  if (value === null || typeof value !== "object") return;
  if (Array.isArray(value)) {
    value.forEach((v, i) => walkStrings(v, visit, `${path}[${i}]`));
    return;
  }
  for (const [key, v] of Object.entries(value as Record<string, unknown>)) {
    walkStrings(v, visit, `${path}.${key}`);
  }
}
