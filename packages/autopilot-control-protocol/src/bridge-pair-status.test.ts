import { describe, expect, test } from "bun:test"

import { projectPairStatus } from "./bridge-pair-status.js"

const nowIso = "2026-06-13T12:00:00.000Z"

describe("bridge pair status projection", () => {
  test("projects null claims to unpaired", () => {
    expect(projectPairStatus(null, nowIso)).toEqual({
      state: "unpaired",
      verbCount: 0,
      reason: "no_pairing_claims",
    })
  })

  test("projects unexpired observe claims to active with readable bridge verbs", () => {
    expect(projectPairStatus({
      expiresAt: "2026-06-13T12:00:01.000Z",
      capabilities: ["observe_public"],
    }, nowIso)).toEqual({
      state: "active",
      verbCount: 8,
      reason: "pairing_active",
    })
  })

  test("projects revoked claims to revoked before considering expiry", () => {
    expect(projectPairStatus({
      revoked: true,
      expiresAt: "2026-06-13T11:59:59.000Z",
      capabilities: ["observe_public", "send_instruction"],
    }, nowIso)).toEqual({
      state: "revoked",
      verbCount: 0,
      reason: "pairing_revoked",
    })
  })

  test("projects claims with past expiry to expired", () => {
    expect(projectPairStatus({
      expiresAt: "2026-06-13T11:59:59.000Z",
      capabilities: ["observe_public", "read_artifact"],
    }, nowIso)).toEqual({
      state: "expired",
      verbCount: 0,
      reason: "pairing_expired",
    })
  })

  test("keeps claims active when expiry equals now", () => {
    expect(projectPairStatus({
      expiresAt: nowIso,
      capabilities: ["observe_public", "read_artifact"],
    }, nowIso)).toEqual({
      state: "active",
      verbCount: 9,
      reason: "pairing_active",
    })
  })

  test("ignores unknown capability strings defensively", () => {
    expect(projectPairStatus({
      expiresAt: "2026-06-13T12:00:01.000Z",
      capabilities: ["root", "observe_public", "admin"],
    }, nowIso)).toEqual({
      state: "active",
      verbCount: 8,
      reason: "pairing_active",
    })
  })

  test("handles missing and malformed capability lists without throwing", () => {
    expect(projectPairStatus({
      expiresAt: "2026-06-13T12:00:01.000Z",
    }, nowIso)).toEqual({
      state: "active",
      verbCount: 0,
      reason: "pairing_active",
    })

    expect(projectPairStatus({
      expiresAt: "2026-06-13T12:00:01.000Z",
      capabilities: "observe_public",
    } as never, nowIso)).toEqual({
      state: "active",
      verbCount: 0,
      reason: "pairing_active",
    })
  })

  test("treats invalid expiry strings as non-expiring rather than throwing", () => {
    expect(projectPairStatus({
      expiresAt: "not-an-iso-date",
      capabilities: ["observe_public"],
    }, nowIso)).toEqual({
      state: "active",
      verbCount: 8,
      reason: "pairing_active",
    })
  })
})
