import { describe, expect, test } from "vite-plus/test";
import { Schema as S } from "effect";
import {
  ModelFailureClass,
  aiErrorReasonTagModelFailureClasses,
  modelFailureClassForAiErrorReasonTag,
  modelFailureClasses,
  type KnownAiErrorReasonTag,
} from "./index.js";

const allKnownReasonTags: ReadonlyArray<KnownAiErrorReasonTag> = [
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
];

describe("ModelFailureClass", () => {
  test("the schema and the runtime list carry the same four classes", () => {
    expect(modelFailureClasses).toEqual([
      "account_rate_limited",
      "account_exhausted",
      "auth_required",
      "unknown",
    ]);
    const decode = S.decodeUnknownSync(ModelFailureClass);
    for (const value of modelFailureClasses) {
      expect(decode(value)).toBe(value);
    }
    expect(() => decode("verification_failed")).toThrow();
    expect(() => decode("")).toThrow();
  });
});

describe("aiErrorReasonTagModelFailureClasses", () => {
  test("maps every one of the 18 known AiError reason tags (total)", () => {
    expect(allKnownReasonTags).toHaveLength(18);
    expect(Object.keys(aiErrorReasonTagModelFailureClasses).sort()).toEqual(
      [...allKnownReasonTags].sort(),
    );
    for (const tag of allKnownReasonTags) {
      expect(modelFailureClasses).toContain(aiErrorReasonTagModelFailureClasses[tag]);
    }
  });

  test("keeps the STREAM-01 semantic matches verbatim", () => {
    expect(modelFailureClassForAiErrorReasonTag("RateLimitError")).toBe("account_rate_limited");
    expect(modelFailureClassForAiErrorReasonTag("QuotaExhaustedError")).toBe("account_exhausted");
    expect(modelFailureClassForAiErrorReasonTag("AuthenticationError")).toBe("auth_required");
  });

  test("reasons without a semantic match classify as unknown", () => {
    expect(modelFailureClassForAiErrorReasonTag("NetworkError")).toBe("unknown");
    expect(modelFailureClassForAiErrorReasonTag("ContentPolicyError")).toBe("unknown");
    expect(modelFailureClassForAiErrorReasonTag("ToolNotFoundError")).toBe("unknown");
  });
});

describe("modelFailureClassForAiErrorReasonTag", () => {
  test("is total over arbitrary strings: unrecognized tags classify as unknown", () => {
    expect(modelFailureClassForAiErrorReasonTag("SomeFutureError")).toBe("unknown");
    expect(modelFailureClassForAiErrorReasonTag("")).toBe("unknown");
    // Prototype-chain names must not leak through the record lookup.
    expect(modelFailureClassForAiErrorReasonTag("toString")).toBe("unknown");
    expect(modelFailureClassForAiErrorReasonTag("hasOwnProperty")).toBe("unknown");
  });
});
