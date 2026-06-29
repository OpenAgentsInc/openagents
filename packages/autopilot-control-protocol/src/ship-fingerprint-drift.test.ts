import { describe, expect, test } from "bun:test"

import { detectFingerprintDrift } from "./ship-fingerprint-drift.js"

describe("ship fingerprint drift", () => {
  test("returns a stable empty drift report for no history", () => {
    expect(detectFingerprintDrift([])).toEqual({
      currentFingerprint: null,
      lastNativeChangeAt: null,
      otaSafeSince: null,
      changes: 0,
    })
  })

  test("uses the first observation as the current fingerprint without counting a change", () => {
    expect(detectFingerprintDrift([
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:00:00.000Z" },
    ])).toEqual({
      currentFingerprint: "expo.fp.0001",
      lastNativeChangeAt: null,
      otaSafeSince: "2026-06-13T10:00:00.000Z",
      changes: 0,
    })
  })

  test("keeps OTA safe since the first observation when the fingerprint never changes", () => {
    expect(detectFingerprintDrift([
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:00:00.000Z" },
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:05:00.000Z" },
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:10:00.000Z" },
    ])).toEqual({
      currentFingerprint: "expo.fp.0001",
      lastNativeChangeAt: null,
      otaSafeSince: "2026-06-13T10:00:00.000Z",
      changes: 0,
    })
  })

  test("counts a single native fingerprint change and marks OTA safe from that change", () => {
    expect(detectFingerprintDrift([
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:00:00.000Z" },
      { fingerprint: "expo.fp.0002", at: "2026-06-13T10:05:00.000Z" },
      { fingerprint: "expo.fp.0002", at: "2026-06-13T10:10:00.000Z" },
    ])).toEqual({
      currentFingerprint: "expo.fp.0002",
      lastNativeChangeAt: "2026-06-13T10:05:00.000Z",
      otaSafeSince: "2026-06-13T10:05:00.000Z",
      changes: 1,
    })
  })

  test("counts every fingerprint transition and reports the last change timestamp", () => {
    expect(detectFingerprintDrift([
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:00:00.000Z" },
      { fingerprint: "expo.fp.0002", at: "2026-06-13T10:05:00.000Z" },
      { fingerprint: "expo.fp.0002", at: "2026-06-13T10:10:00.000Z" },
      { fingerprint: "expo.fp.0003", at: "2026-06-13T10:15:00.000Z" },
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:20:00.000Z" },
    ])).toEqual({
      currentFingerprint: "expo.fp.0001",
      lastNativeChangeAt: "2026-06-13T10:20:00.000Z",
      otaSafeSince: "2026-06-13T10:20:00.000Z",
      changes: 3,
    })
  })

  test("compares fingerprints exactly through the shared classifier", () => {
    expect(detectFingerprintDrift([
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:00:00.000Z" },
      { fingerprint: " expo.fp.0001 ", at: "2026-06-13T10:05:00.000Z" },
    ])).toEqual({
      currentFingerprint: " expo.fp.0001 ",
      lastNativeChangeAt: "2026-06-13T10:05:00.000Z",
      otaSafeSince: "2026-06-13T10:05:00.000Z",
      changes: 1,
    })
  })

  test("does not mutate the source history", () => {
    const history = [
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:00:00.000Z" },
      { fingerprint: "expo.fp.0002", at: "2026-06-13T10:05:00.000Z" },
    ]

    detectFingerprintDrift(history)

    expect(history).toEqual([
      { fingerprint: "expo.fp.0001", at: "2026-06-13T10:00:00.000Z" },
      { fingerprint: "expo.fp.0002", at: "2026-06-13T10:05:00.000Z" },
    ])
  })
})
