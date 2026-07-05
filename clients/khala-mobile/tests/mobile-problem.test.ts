import { describe, expect, test } from "bun:test"

import {
  MobileProblemError,
  classifyMobileStatusProblem,
  classifyMobileThrownProblem,
  mobileProblemMessageSafe,
  readOkMobileJsonResponse,
} from "../src/network/mobile-problem"

describe("Khala mobile problem classification", () => {
  test("classifies representative HTTP status codes with public-safe messages", () => {
    expect(classifyMobileStatusProblem(401, null, "bootstrap").kind).toBe("unauthorized")
    expect(classifyMobileStatusProblem(403, null, "bootstrap").kind).toBe("forbidden")
    expect(classifyMobileStatusProblem(404, null, "bootstrap").kind).toBe("not_found")
    expect(classifyMobileStatusProblem(429, null, "bootstrap").kind).toBe("rate_limited")
    expect(classifyMobileStatusProblem(503, null, "bootstrap").kind).toBe("server_error")
    expect(classifyMobileStatusProblem(418, null, "bootstrap")).toEqual({
      kind: "unknown",
      messageSafe: "bootstrap: request failed (418)",
      status: 418,
    })
  })

  test("uses server-provided messageSafe without exposing raw payload fields", () => {
    const problem = classifyMobileStatusProblem(
      403,
      {
        messageSafe: "scope is forbidden",
        rawProviderPayload: "do not leak",
        token: "do not leak",
      },
      "bootstrap",
    )

    expect(problem).toEqual({
      kind: "forbidden",
      messageSafe: "scope is forbidden",
      status: 403,
    })
    expect(problem.messageSafe).not.toContain("rawProviderPayload")
    expect(problem.messageSafe).not.toContain("token")
  })

  test("classifies timeout and network thrown errors", () => {
    const abort = new Error("aborted")
    abort.name = "AbortError"

    expect(classifyMobileThrownProblem(abort, "sign-in check")).toEqual({
      kind: "timeout",
      messageSafe: "sign-in check: request timed out",
      status: null,
    })
    expect(classifyMobileThrownProblem(new TypeError("Failed to fetch"), "push").kind).toBe(
      "network_unavailable",
    )
  })

  test("throws a typed malformed response problem when JSON parsing fails", async () => {
    await expect(
      readOkMobileJsonResponse(
        {
          json: async () => {
            throw new Error("raw parse failure")
          },
          ok: true,
          status: 200,
        },
        "bootstrap",
      ),
    ).rejects.toThrow(MobileProblemError)

    try {
      await readOkMobileJsonResponse(
        {
          json: async () => {
            throw new Error("raw parse failure")
          },
          ok: true,
          status: 200,
        },
        "bootstrap",
      )
    } catch (error) {
      expect(error).toBeInstanceOf(MobileProblemError)
      expect((error as MobileProblemError).problem).toEqual({
        kind: "malformed_response",
        messageSafe: "bootstrap: server returned an unreadable response",
        status: null,
      })
    }
  })

  test("mobileProblemMessageSafe preserves typed messages and bounds unknown errors", () => {
    const typed = new MobileProblemError({
      kind: "rate_limited",
      messageSafe: "try again later",
      status: 429,
    })

    expect(mobileProblemMessageSafe(typed, "push")).toBe("try again later")
    expect(mobileProblemMessageSafe(new Error("private token abc"), "push")).toBe(
      "push: request failed",
    )
  })
})
