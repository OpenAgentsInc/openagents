/**
 * STREAM-01 (#9129), refactored under AISDK-05 (#9151): map Effect AI
 * `AiError` reasons onto the shared `HarnessFailureClass` vocabulary from
 * `contract.ts`.
 *
 * Effect v4 (`effect/unstable/ai`) reports every model-call failure as one
 * `AiError` with a typed `reason`. The harness contract reports failures with
 * `HarnessFailureClass`. This module is the single translation point, so
 * model-call lanes on the Effect AI substrate speak the same fleet failure
 * vocabulary as the coding harnesses.
 *
 * AISDK-05 resolved the public/private seam: the neutral model-call classes
 * (`account_rate_limited`, `account_exhausted`, `auth_required`, `unknown`)
 * and the string-tag-keyed total mapping now live in the PUBLIC
 * `@openagentsinc/agent-runtime-schema` package (`model-failure.ts`), so
 * npm-published packages such as `@openagentsinc/ai-model` can use
 * them directly. This private module keeps the typed `AiError`-object
 * helpers as thin wrappers over that public function, plus the compile-time
 * totality check against the real `AiError` reason union — that check stays
 * here because the schema package deliberately imports no AI types. The
 * wider harness-run-stage classes (`verification_failed`,
 * `workspace_materialization`, `cancelled`, `timeout`) describe harness
 * stages, not model-call faults, and remain private to this package.
 */
import {
  aiErrorReasonTagModelFailureClasses,
  modelFailureClassForAiErrorReasonTag,
} from "@openagentsinc/agent-runtime-schema"
import { AiError } from "effect/unstable/ai"
import type { HarnessFailureClass } from "./contract.ts"

/** The `_tag` of one typed `AiError` reason. */
export type AiErrorReasonTag = AiError.AiErrorReason["_tag"]

/**
 * Total mapping from every `AiError` reason tag to a harness failure class.
 * The values are the public neutral mapping from
 * `@openagentsinc/agent-runtime-schema` (semantics recorded there):
 *
 * - `RateLimitError` -> `account_rate_limited` (mandatory class)
 * - `QuotaExhaustedError` -> `account_exhausted` (mandatory class, per #9129)
 * - `AuthenticationError` -> `auth_required` (the auth-health class)
 * - every other reason -> `unknown` (no honest semantic match; the
 *   harness-run-stage classes describe harness stages, not model faults)
 *
 * The `Record<AiErrorReasonTag, ...>` annotation keeps totality a
 * compile-time property against the REAL typed reason union: when Effect
 * adds or renames a reason, this assignment fails to typecheck until the
 * public schema-package mapping is updated. Every `ModelFailureClass` value
 * is one of the `HarnessFailureClass` literals, which makes the assignment
 * well-typed; the alignment guard test asserts that inclusion at runtime.
 */
export const aiErrorReasonFailureClasses: Readonly<
  Record<AiErrorReasonTag, HarnessFailureClass>
> = aiErrorReasonTagModelFailureClasses

/** Classify one typed `AiError` reason tag. */
export function harnessFailureClassForAiErrorReason(
  tag: AiErrorReasonTag,
): HarnessFailureClass {
  return modelFailureClassForAiErrorReasonTag(tag)
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
