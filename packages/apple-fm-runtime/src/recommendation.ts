import { Schema as S } from "effect";

import {
  MAX_TURN_OUTPUT_CHARS,
  ROUTE_RECOMMENDATION_SCHEMA_LITERAL,
  RouteReasonCode,
  RouteRecommendation,
  TurnProviderCandidate,
  TurnTaskClass,
  type TurnProviderCandidate as TurnProviderCandidateType,
  type TurnRefusalReason,
} from "@openagentsinc/agent-runtime-schema";

/**
 * `@openagentsinc/apple-fm-runtime` Phase-1 JSON recommendation decoder
 * (AFS-02).
 *
 * The current Apple FM bridge speaks plain text. A structured router asks the
 * local model to emit a typed route recommendation as JSON over that plain-text
 * bridge, and this decoder parses and validates it. It is FAIL-CLOSED:
 *
 * - A DECODE FAILURE NEVER DISPATCHES. Only a `Recommendation` result may drive
 *   a provider start; the safe answer fallback is `Answer` (advisory text), and
 *   everything else is a typed `Reject`.
 * - A recommendation whose candidate is not in the owner-bound admitted set is
 *   an unavailable-agent claim and is rejected — never dispatched.
 * - An action-claim (a structured output asserting it performed or authorized a
 *   tool/command/file mutation) is rejected: Apple FM has no action authority.
 * - Empty, oversized, malformed, unavailable-agent, and action-claim outputs
 *   all produce a typed refusal, not a dispatch.
 * - The safe answer fallback is permitted ONLY when the complete raw result is
 *   plain advisory text that passes the normal local answer contract. A broken
 *   structured route attempt is `malformed_output`, not an answer.
 *
 * A later Swift `@Generable` path can replace this text-to-JSON adapter after it
 * has its own bridge and package evidence.
 */

export type AppleFmRouteDecodeResult =
  | { readonly _tag: "Recommendation"; readonly recommendation: RouteRecommendation }
  | { readonly _tag: "Answer"; readonly text: string }
  | { readonly _tag: "Reject"; readonly reason: TurnRefusalReason };

export interface AppleFmRouteDecodeInput {
  readonly raw: string;
  /** The owner-bound admitted candidate vocabulary. A recommendation must name one of these. */
  readonly admittedCandidates: ReadonlyArray<TurnProviderCandidateType>;
  readonly maxOutputChars?: number;
}

/** Keys whose presence in a structured output claims action authority. */
const ACTION_CLAIM_KEYS = [
  "action",
  "actions",
  "tool",
  "tools",
  "toolCall",
  "tool_call",
  "command",
  "commands",
  "apply",
  "exec",
  "execute",
  "mutation",
  "fileChange",
  "file_change",
  "sourceControl",
  "source_control",
  "dispatch",
  "delegateTo",
] as const;

/** The bounded structured route recommendation the local model may emit as JSON. */
const AppleFmRouteRecommendationJson = S.Struct({
  candidate: TurnProviderCandidate,
  taskClass: TurnTaskClass,
  reasonCode: RouteReasonCode,
  confidence: S.Number.check(S.isGreaterThanOrEqualTo(0), S.isLessThanOrEqualTo(1)),
});

const decodeRecommendationJson = S.decodeUnknownExit(AppleFmRouteRecommendationJson);

const asRecord = (value: unknown): Record<string, unknown> | undefined =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;

/** Extract the first balanced JSON object from a plain-text result, if any. */
const extractJsonObject = (raw: string): unknown | undefined => {
  const trimmed = raw.trim();
  const unfenced = trimmed.replace(/^```(?:json)?\s*/iu, "").replace(/\s*```$/u, "");
  const start = unfenced.indexOf("{");
  if (start === -1) return undefined;
  let depth = 0;
  let inString = false;
  let escaped = false;
  for (let index = start; index < unfenced.length; index += 1) {
    const char = unfenced[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === '"') inString = false;
      continue;
    }
    if (char === '"') inString = true;
    else if (char === "{") depth += 1;
    else if (char === "}") {
      depth -= 1;
      if (depth === 0) {
        const candidate = unfenced.slice(start, index + 1);
        try {
          const parsed: unknown = JSON.parse(candidate);
          return parsed;
        } catch {
          return undefined;
        }
      }
    }
  }
  return undefined;
};

/** True when a parsed object claims to perform or authorize an action. */
const claimsAction = (record: Record<string, unknown>): boolean =>
  ACTION_CLAIM_KEYS.some((key) => Object.prototype.hasOwnProperty.call(record, key) && record[key] != null);

/** Does the raw text look like an ATTEMPTED structured route (JSON object present)? */
const looksStructured = (raw: string): boolean => extractJsonObject(raw) !== undefined;

/**
 * Decode Apple FM plain-text output into a route recommendation, a safe advisory
 * answer, or a typed rejection. FAIL-CLOSED: only a `Recommendation` may drive a
 * dispatch.
 */
export const decodeAppleFmRouteOutput = (input: AppleFmRouteDecodeInput): AppleFmRouteDecodeResult => {
  const maxOutputChars = input.maxOutputChars ?? MAX_TURN_OUTPUT_CHARS;
  const raw = input.raw;
  const trimmed = raw.trim();

  if (trimmed.length === 0) return { _tag: "Reject", reason: "empty_output" };
  if (raw.length > maxOutputChars) return { _tag: "Reject", reason: "oversized_output" };

  const json = extractJsonObject(raw);
  const record = asRecord(json);

  if (record !== undefined) {
    // An action-claim in structured output is never dispatched.
    if (claimsAction(record)) return { _tag: "Reject", reason: "action_claim_rejected" };

    // Support a `route` alias for `candidate`.
    const normalized: Record<string, unknown> = { ...record };
    if (normalized.candidate === undefined && typeof normalized.route === "string") {
      normalized.candidate = normalized.route;
    }

    const decoded = decodeRecommendationJson(normalized);
    if (decoded._tag === "Failure") {
      // A broken structured route attempt is malformed output — never an answer.
      return { _tag: "Reject", reason: "malformed_output" };
    }
    const value = decoded.value;
    if (!input.admittedCandidates.includes(value.candidate)) {
      // Unavailable-agent: the recommended lane is not in the owner-bound set.
      return { _tag: "Reject", reason: "provider_unadmitted" };
    }
    const recommendation: RouteRecommendation = {
      schema: ROUTE_RECOMMENDATION_SCHEMA_LITERAL,
      candidate: value.candidate,
      taskClass: value.taskClass,
      reasonCode: value.reasonCode,
      confidence: value.confidence,
    };
    return { _tag: "Recommendation", recommendation };
  }

  // No structured object. Decode failure must never dispatch: fall back to a
  // safe advisory answer only when the complete raw result is plain text that
  // passes the normal local answer contract.
  if (looksStructured(raw)) return { _tag: "Reject", reason: "malformed_output" };
  if (trimmed.length > MAX_TURN_OUTPUT_CHARS) return { _tag: "Reject", reason: "oversized_output" };
  return { _tag: "Answer", text: trimmed.slice(0, MAX_TURN_OUTPUT_CHARS) };
};
