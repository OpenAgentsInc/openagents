/**
 * STREAM-01 (#9129): map Effect AI `AiError` reasons onto the shared
 * `HarnessFailureClass` vocabulary from `contract.ts`.
 *
 * Effect v4 (`effect/unstable/ai`) reports every model-call failure as one
 * `AiError` with a typed `reason`. The harness contract reports failures with
 * `HarnessFailureClass`. This module is the single translation point, so
 * model-call lanes on the Effect AI substrate speak the same fleet failure
 * vocabulary as the coding harnesses.
 *
 * The mapping lives in this package because `HarnessFailureClass` is defined
 * here and its consumers (fleet status surfaces, harness classifiers) already
 * import this package. `@openagentsinc/khala-ai-sdk-core` is npm-publishable
 * and must not depend on this private workspace package.
 */
import { AiError } from "effect/unstable/ai"
import type { HarnessFailureClass } from "./contract.ts"

/** The `_tag` of one typed `AiError` reason. */
export type AiErrorReasonTag = AiError.AiErrorReason["_tag"]

/**
 * Total mapping from every `AiError` reason tag to a harness failure class.
 *
 * The `Record` type makes totality a compile-time property: when Effect adds
 * or renames a reason, this table fails to typecheck until it is updated.
 *
 * Semantic matches:
 * - `RateLimitError` -> `account_rate_limited` (mandatory class)
 * - `QuotaExhaustedError` -> `account_exhausted` (mandatory class, per #9129)
 * - `AuthenticationError` -> `auth_required` (the auth-health class)
 *
 * Every other reason has no honest semantic match in the harness vocabulary
 * (`verification_failed`, `workspace_materialization`, `cancelled`, and
 * `timeout` describe harness-run stages, not model-call faults), so those
 * reasons map to `unknown` instead of inventing a false equivalence.
 */
export const aiErrorReasonFailureClasses: Readonly<
  Record<AiErrorReasonTag, HarnessFailureClass>
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
}

/** Classify one typed `AiError` reason tag. */
export function harnessFailureClassForAiErrorReason(
  tag: AiErrorReasonTag,
): HarnessFailureClass {
  return aiErrorReasonFailureClasses[tag]
}

/** Classify a full `AiError` by its `reason`. */
export function harnessFailureClassForAiError(
  error: AiError.AiError,
): HarnessFailureClass {
  return harnessFailureClassForAiErrorReason(error.reason._tag)
}

/**
 * Classify an arbitrary thrown/failed value. Non-`AiError` values classify as
 * `unknown` so callers do not have to narrow before reporting.
 */
export function harnessFailureClassForUnknown(error: unknown): HarnessFailureClass {
  if (AiError.isAiError(error)) return harnessFailureClassForAiError(error)
  if (AiError.isAiErrorReason(error)) {
    return harnessFailureClassForAiErrorReason(error._tag)
  }
  return "unknown"
}
