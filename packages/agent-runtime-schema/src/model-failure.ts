import { Schema as S } from "effect";

/**
 * AISDK-05 (#9151): the public, provider-neutral model-call failure
 * vocabulary.
 *
 * STREAM-01 (#9129) landed the total `AiError` reason to failure-class
 * mapping in the private `@openagentsinc/harness-conformance` package
 * (`src/ai-error-failure-class.ts`), because that package owns
 * `HarnessFailureClass`. STREAM-05 (#9133) then had to compose that mapping
 * at the caller, because npm-published packages such as
 * `@openagentsinc/ai-model` must not depend on the private
 * workspace package. This module resolves that seam: the neutral classes a
 * MODEL CALL can honestly produce, and the total reason-tag mapping, live
 * here in the public schema package. `harness-conformance` now consumes
 * this vocabulary and keeps only its typed `AiError`-object wrappers and
 * the wider harness-run-stage classes (`verification_failed`,
 * `workspace_materialization`, `cancelled`, `timeout`), which describe
 * harness stages, not model-call faults, and therefore stay private.
 *
 * Dependency rule: this module is STRING-TAG KEYED on purpose. It never
 * imports `effect/unstable/ai`, so the schema package gains no AI (or any
 * other) dependency and stays the schema-only contract its README
 * promises. The typed `AiError` compile-time totality check remains in
 * `harness-conformance`, where the `AiError` type is already available.
 *
 * Compatibility: `ModelFailureClass` is a frozen vocabulary. Adding,
 * removing, or renaming a class is a breaking change for decoders and
 * needs a deliberate contract revision, not a drive-by edit.
 */

/**
 * The neutral failure classes a model call can honestly report. These are
 * exactly the classes the landed STREAM-01 mapping produces:
 *
 * - `account_rate_limited` — the provider rate-limited the account.
 * - `account_exhausted` — the account's quota/credits are exhausted.
 * - `auth_required` — the credentials are missing, invalid, or expired.
 * - `unknown` — every reason without an honest semantic match.
 */
export const ModelFailureClass = S.Literals([
  "account_rate_limited",
  "account_exhausted",
  "auth_required",
  "unknown",
]);
export type ModelFailureClass = typeof ModelFailureClass.Type;

export const modelFailureClasses: ReadonlyArray<ModelFailureClass> = [
  "account_rate_limited",
  "account_exhausted",
  "auth_required",
  "unknown",
];

/**
 * The 18 `AiError` reason `_tag` values of the pinned Effect v4 runtime
 * (`effect/unstable/ai`, catalog pin 4.0.0-beta.94), carried here as plain
 * string literals so this package needs no AI import.
 */
export type KnownAiErrorReasonTag =
  | "AuthenticationError"
  | "ContentPolicyError"
  | "InternalProviderError"
  | "InvalidOutputError"
  | "InvalidRequestError"
  | "InvalidToolResultError"
  | "InvalidUserInputError"
  | "NetworkError"
  | "QuotaExhaustedError"
  | "RateLimitError"
  | "StructuredOutputError"
  | "ToolConfigurationError"
  | "ToolNotFoundError"
  | "ToolParameterValidationError"
  | "ToolResultEncodingError"
  | "ToolkitRequiredError"
  | "UnknownError"
  | "UnsupportedSchemaError";

/**
 * Total mapping from every known `AiError` reason tag to a neutral model
 * failure class. The semantics are copied verbatim from the landed
 * STREAM-01 (#9129) mapping in
 * `packages/harness-conformance/src/ai-error-failure-class.ts`:
 *
 * - `RateLimitError` -> `account_rate_limited`
 * - `QuotaExhaustedError` -> `account_exhausted`
 * - `AuthenticationError` -> `auth_required`
 *
 * Every other reason has no honest semantic match, so it maps to `unknown`
 * instead of inventing a false equivalence.
 */
export const aiErrorReasonTagModelFailureClasses: Readonly<
  Record<KnownAiErrorReasonTag, ModelFailureClass>
> = {
  AuthenticationError: "auth_required",
  ContentPolicyError: "unknown",
  InternalProviderError: "unknown",
  InvalidOutputError: "unknown",
  InvalidRequestError: "unknown",
  InvalidToolResultError: "unknown",
  InvalidUserInputError: "unknown",
  NetworkError: "unknown",
  QuotaExhaustedError: "account_exhausted",
  RateLimitError: "account_rate_limited",
  StructuredOutputError: "unknown",
  ToolConfigurationError: "unknown",
  ToolNotFoundError: "unknown",
  ToolParameterValidationError: "unknown",
  ToolResultEncodingError: "unknown",
  ToolkitRequiredError: "unknown",
  UnknownError: "unknown",
  UnsupportedSchemaError: "unknown",
};

function isKnownAiErrorReasonTag(tag: string): tag is KnownAiErrorReasonTag {
  return Object.prototype.hasOwnProperty.call(aiErrorReasonTagModelFailureClasses, tag);
}

/**
 * Classify one `AiError` reason `_tag` string. Total over ALL strings: the
 * 18 known tags classify per `aiErrorReasonTagModelFailureClasses`, and any
 * unrecognized tag (for example from a future Effect version) classifies as
 * `unknown` rather than throwing, so a vocabulary skew can never turn a real
 * provider failure into a crash.
 */
export function modelFailureClassForAiErrorReasonTag(tag: string): ModelFailureClass {
  return isKnownAiErrorReasonTag(tag) ? aiErrorReasonTagModelFailureClasses[tag] : "unknown";
}
