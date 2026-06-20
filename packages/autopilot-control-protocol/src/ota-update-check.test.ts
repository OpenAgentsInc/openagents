import { describe, expect, test } from "bun:test"

import { decideUpdateApply } from "./ota-update-check.js"

describe("OTA update apply decision", () => {
  test("applies when runtime matches and manifest id differs from current update", () => {
    expect(decideUpdateApply({
      currentUpdateId: "update-001",
      manifestId: "update-002",
      runtimeMatches: true,
    })).toEqual({
      apply: true,
      reason: "manifest is new and runtime matches",
    })
  })

  test("applies when there is no current update and a matching manifest is present", () => {
    expect(decideUpdateApply({
      currentUpdateId: null,
      manifestId: "update-001",
      runtimeMatches: true,
    })).toEqual({
      apply: true,
      reason: "manifest is new and runtime matches",
    })
  })

  test("does not apply when runtime does not match", () => {
    expect(decideUpdateApply({
      currentUpdateId: "update-001",
      manifestId: "update-002",
      runtimeMatches: false,
    })).toEqual({
      apply: false,
      reason: "runtime does not match",
    })
  })

  test("runtime mismatch takes precedence over a missing manifest id", () => {
    expect(decideUpdateApply({
      currentUpdateId: "update-001",
      manifestId: null,
      runtimeMatches: false,
    })).toEqual({
      apply: false,
      reason: "runtime does not match",
    })
  })

  test("does not apply when manifest id is missing", () => {
    expect(decideUpdateApply({
      currentUpdateId: "update-001",
      manifestId: null,
      runtimeMatches: true,
    })).toEqual({
      apply: false,
      reason: "manifest id is missing",
    })
  })

  test("does not apply when manifest id is already current", () => {
    expect(decideUpdateApply({
      currentUpdateId: "update-001",
      manifestId: "update-001",
      runtimeMatches: true,
    })).toEqual({
      apply: false,
      reason: "manifest is already current",
    })
  })
})
