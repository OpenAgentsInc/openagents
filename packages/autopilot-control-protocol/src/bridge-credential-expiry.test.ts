import { describe, expect, test } from "bun:test"

import { credentialExpiry } from "./bridge-credential-expiry.js"

const nowIso = "2026-06-13T12:00:00.000Z"

describe("bridge credential expiry projection", () => {
  test("projects null expiry to none", () => {
    expect(credentialExpiry({ expiresAt: null, nowIso })).toEqual({
      state: "none",
      msRemaining: null,
      shouldRefresh: false,
    })
  })

  test("projects malformed expiry to none without throwing", () => {
    expect(credentialExpiry({ expiresAt: "not-an-iso-date", nowIso })).toEqual({
      state: "none",
      msRemaining: null,
      shouldRefresh: false,
    })
  })

  test("projects malformed now to none without throwing", () => {
    expect(credentialExpiry({
      expiresAt: "2026-06-13T12:10:00.000Z",
      nowIso: "not-an-iso-date",
    })).toEqual({
      state: "none",
      msRemaining: null,
      shouldRefresh: false,
    })
  })

  test("keeps credentials valid outside the default five minute refresh window", () => {
    expect(credentialExpiry({
      expiresAt: "2026-06-13T12:05:00.001Z",
      nowIso,
    })).toEqual({
      state: "valid",
      msRemaining: 300_001,
      shouldRefresh: false,
    })
  })

  test("marks credentials expiring inside the default five minute refresh window", () => {
    expect(credentialExpiry({
      expiresAt: "2026-06-13T12:05:00.000Z",
      nowIso,
    })).toEqual({
      state: "expiring",
      msRemaining: 300_000,
      shouldRefresh: true,
    })
  })

  test("marks credentials expiring when expiry equals now", () => {
    expect(credentialExpiry({
      expiresAt: nowIso,
      nowIso,
    })).toEqual({
      state: "expiring",
      msRemaining: 0,
      shouldRefresh: true,
    })
  })

  test("marks credentials expired after expiry passes", () => {
    expect(credentialExpiry({
      expiresAt: "2026-06-13T11:59:59.999Z",
      nowIso,
    })).toEqual({
      state: "expired",
      msRemaining: -1,
      shouldRefresh: true,
    })
  })

  test("honors a custom refresh window", () => {
    expect(credentialExpiry({
      expiresAt: "2026-06-13T12:02:00.000Z",
      nowIso,
      refreshWindowMs: 119_999,
    })).toEqual({
      state: "valid",
      msRemaining: 120_000,
      shouldRefresh: false,
    })

    expect(credentialExpiry({
      expiresAt: "2026-06-13T12:02:00.000Z",
      nowIso,
      refreshWindowMs: 120_000,
    })).toEqual({
      state: "expiring",
      msRemaining: 120_000,
      shouldRefresh: true,
    })
  })

  test("falls back to the default refresh window for invalid window values", () => {
    expect(credentialExpiry({
      expiresAt: "2026-06-13T12:05:00.000Z",
      nowIso,
      refreshWindowMs: Number.NaN,
    })).toEqual({
      state: "expiring",
      msRemaining: 300_000,
      shouldRefresh: true,
    })
  })
})
