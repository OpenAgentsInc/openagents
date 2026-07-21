import { describe, expect, test } from "vite-plus/test"
import {
  aiErrorReasonTagModelFailureClasses,
  modelFailureClassForAiErrorReasonTag,
  modelFailureClasses,
  type ModelFailureClass,
} from "@openagentsinc/agent-runtime-schema"
import { AiError } from "effect/unstable/ai"
import {
  aiErrorReasonFailureClasses,
  harnessFailureClassForAiError,
  harnessFailureClassForAiErrorReason,
  harnessFailureClassForUnknown,
  type AiErrorReasonTag,
} from "./ai-error-failure-class.ts"
import { requiredFailureClasses, type HarnessFailureClass } from "./contract.ts"

const allReasonTags: ReadonlyArray<AiErrorReasonTag> = [
  "AuthenticationError",
  "ContentPolicyError",
  "InternalProviderError",
  "InvalidOutputError",
  "InvalidRequestError",
  "InvalidToolResultError",
  "InvalidUserInputError",
  "NetworkError",
  "QuotaExhaustedError",
  "RateLimitError",
  "StructuredOutputError",
  "ToolConfigurationError",
  "ToolNotFoundError",
  "ToolParameterValidationError",
  "ToolResultEncodingError",
  "ToolkitRequiredError",
  "UnknownError",
  "UnsupportedSchemaError",
]

describe("aiErrorReasonFailureClasses", () => {
  test("maps every one of the 18 AiError reasons (total)", () => {
    expect(allReasonTags).toHaveLength(18)
    expect(Object.keys(aiErrorReasonFailureClasses).sort()).toEqual(
      [...allReasonTags].sort(),
    )
    for (const tag of allReasonTags) {
      expect(typeof harnessFailureClassForAiErrorReason(tag)).toBe("string")
    }
  })

  test("maps the mandatory account-capacity and auth-health classes", () => {
    expect(harnessFailureClassForAiErrorReason("RateLimitError")).toBe(
      "account_rate_limited",
    )
    expect(harnessFailureClassForAiErrorReason("QuotaExhaustedError")).toBe(
      "account_exhausted",
    )
    expect(harnessFailureClassForAiErrorReason("AuthenticationError")).toBe(
      "auth_required",
    )
    expect(requiredFailureClasses).toContain("account_rate_limited")
    expect(requiredFailureClasses).toContain("account_exhausted")
  })

  test("reasons without a semantic harness match classify as unknown", () => {
    expect(harnessFailureClassForAiErrorReason("NetworkError")).toBe("unknown")
    expect(harnessFailureClassForAiErrorReason("ContentPolicyError")).toBe("unknown")
    expect(harnessFailureClassForAiErrorReason("ToolNotFoundError")).toBe("unknown")
  })
})

describe("harnessFailureClassForAiError", () => {
  test("classifies a constructed AiError by its reason", () => {
    const rateLimited = AiError.make({
      method: "streamText",
      module: "Test",
      reason: new AiError.RateLimitError({}),
    })
    expect(harnessFailureClassForAiError(rateLimited)).toBe("account_rate_limited")

    const exhausted = AiError.make({
      method: "streamText",
      module: "Test",
      reason: new AiError.QuotaExhaustedError({}),
    })
    expect(harnessFailureClassForAiError(exhausted)).toBe("account_exhausted")

    const auth = AiError.make({
      method: "generateText",
      module: "Test",
      reason: new AiError.AuthenticationError({ kind: "InvalidKey" }),
    })
    expect(harnessFailureClassForAiError(auth)).toBe("auth_required")
  })
})

describe("public/private failure-class alignment (AISDK-05 #9151)", () => {
  // Compile-time guard: every public ModelFailureClass literal must be one of
  // the private HarnessFailureClass literals. If the public vocabulary gains
  // a class the harness contract does not carry, this line stops compiling.
  const _publicClassesAreHarnessClasses =
    modelFailureClasses satisfies ReadonlyArray<HarnessFailureClass>
  void _publicClassesAreHarnessClasses

  // Compile-time guard in the other direction for the SHARED model-call
  // classes: each one must still exist in the public vocabulary.
  const sharedClasses = [
    "account_rate_limited",
    "account_exhausted",
    "auth_required",
    "unknown",
  ] as const satisfies ReadonlyArray<ModelFailureClass & HarnessFailureClass>

  test("every public ModelFailureClass literal is a HarnessFailureClass literal", () => {
    const harnessClasses: ReadonlySet<HarnessFailureClass> = new Set([
      "account_exhausted",
      "account_rate_limited",
      "account_quota_exhausted",
      "auth_required",
      "verification_failed",
      "workspace_materialization",
      "cancelled",
      "timeout",
      "unknown",
    ])
    for (const publicClass of modelFailureClasses) {
      expect(harnessClasses.has(publicClass)).toBe(true)
    }
    for (const shared of sharedClasses) {
      expect(modelFailureClasses).toContain(shared)
      expect(harnessClasses.has(shared)).toBe(true)
    }
  })

  test("the private mapping is the public mapping, tag for tag", () => {
    expect(aiErrorReasonFailureClasses).toBe(aiErrorReasonTagModelFailureClasses)
    for (const tag of allReasonTags) {
      expect(harnessFailureClassForAiErrorReason(tag)).toBe(
        modelFailureClassForAiErrorReasonTag(tag),
      )
    }
  })
})

describe("harnessFailureClassForUnknown", () => {
  test("classifies AiError values, bare reasons, and foreign errors", () => {
    const error = AiError.make({
      method: "streamText",
      module: "Test",
      reason: new AiError.RateLimitError({}),
    })
    expect(harnessFailureClassForUnknown(error)).toBe("account_rate_limited")
    expect(
      harnessFailureClassForUnknown(new AiError.QuotaExhaustedError({})),
    ).toBe("account_exhausted")
    expect(harnessFailureClassForUnknown(new Error("boom"))).toBe("unknown")
    expect(harnessFailureClassForUnknown("boom")).toBe("unknown")
  })
})
