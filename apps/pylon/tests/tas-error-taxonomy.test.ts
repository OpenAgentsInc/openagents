import { describe, expect, test } from "bun:test"

import {
  classifyError,
  ErrorCategory,
  toBlocker,
} from "../src/tas/error-taxonomy"

describe("tas error taxonomy core", () => {
  test("classifies quota errors as retryable and recoverable", () => {
    expect(
      classifyError({
        code: "RATE_LIMITED",
        message: "Provider rate limit exceeded",
      }),
    ).toEqual({
      category: ErrorCategory.Quota,
      recoverable: true,
      retryable: true,
      blockerRef: "blocker.tas_error.quota",
    })
  })

  test("classifies auth errors as recoverable but not retryable", () => {
    expect(
      classifyError({
        code: "401",
        message: "Invalid API key",
      }),
    ).toEqual({
      category: ErrorCategory.Auth,
      recoverable: true,
      retryable: false,
      blockerRef: "blocker.tas_error.auth",
    })
  })

  test("classifies transient network messages as retryable", () => {
    expect(
      classifyError({
        message: "Connection reset while streaming model output",
      }),
    ).toEqual({
      category: ErrorCategory.Network,
      recoverable: true,
      retryable: true,
      blockerRef: "blocker.tas_error.network",
    })
  })

  test("classifies validation and sandbox failures as non-retryable", () => {
    expect(
      classifyError({
        code: "VALIDATION_ERROR",
        message: "Required field is missing",
      }),
    ).toEqual({
      category: ErrorCategory.Validation,
      recoverable: true,
      retryable: false,
      blockerRef: "blocker.tas_error.validation",
    })

    expect(
      classifyError({
        message: "Write denied outside workspace boundary",
      }),
    ).toEqual({
      category: ErrorCategory.Sandbox,
      recoverable: false,
      retryable: false,
      blockerRef: "blocker.tas_error.sandbox",
    })
  })

  test("classifies unknown failures as internal and not retryable", () => {
    expect(
      classifyError({
        code: "WEIRD_PROVIDER_PAYLOAD",
        message: "Unexpected provider envelope",
      }),
    ).toEqual({
      category: ErrorCategory.Internal,
      recoverable: false,
      retryable: false,
      blockerRef: "blocker.tas_error.internal",
    })
  })

  test("builds refs-only blockers without raw diagnostic text", () => {
    const classified = classifyError({
      code: "ETIMEDOUT",
      message: "Timeout at /private/repo with stack: Error: secret token abc",
    })

    expect(toBlocker(classified)).toEqual({
      kind: "tas_error_blocker",
      blockerRef: "blocker.tas_error.network",
      category: ErrorCategory.Network,
      recoverable: true,
      retryable: true,
    })
    expect(JSON.stringify(toBlocker(classified))).not.toContain(
      "/private/repo",
    )
    expect(JSON.stringify(toBlocker(classified))).not.toContain("secret token")
    expect(JSON.stringify(toBlocker(classified))).not.toContain("stack")
  })
})
