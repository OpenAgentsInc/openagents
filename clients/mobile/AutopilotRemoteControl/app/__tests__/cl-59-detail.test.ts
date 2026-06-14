/// <reference types="bun-types" />
import { describe, expect, test } from "bun:test"

import type { ControlSessionRow } from "../../src/control/control-client"
// Import the pure helper from its RN-free home; session-detail.tsx re-exports
// the same `verifyText`, but importing the screen would drag in react-native
// and bun can't parse RN's flow-typed entrypoint.
import { verifyText } from "../session-detail-view-model"

// Build a ControlSessionRow with sensible defaults; override the bits each case
// cares about (state / artifactRef / errorClass).
function row(over: Partial<ControlSessionRow>): ControlSessionRow {
  return {
    sessionRef: "ref-abc",
    adapter: "codex",
    state: "running",
    lastProgressRef: "none",
    artifactRef: null,
    resultRef: null,
    errorClass: null,
    latestActivity: "",
    parentRef: null,
    agentKind: null,
    ...over,
  }
}

describe("verifyText", () => {
  test("completed → ok, with artifact slice when present", () => {
    const v = verifyText(row({ state: "completed", artifactRef: "sha256:0123456789abcdef" }))
    expect(v.tone).toBe("ok")
    expect(v.text).toContain("✓ verify passed")
    expect(v.text).toContain("3456789abcdef".slice(-12))
  })

  test("completed without artifactRef → ok, no artifact suffix", () => {
    const v = verifyText(row({ state: "completed", artifactRef: null }))
    expect(v.tone).toBe("ok")
    expect(v.text).toBe("✓ verify passed")
  })

  test("failed → bad, with errorClass when present", () => {
    const v = verifyText(row({ state: "failed", errorClass: "verify_failed" }))
    expect(v.tone).toBe("bad")
    expect(v.text).toBe("✗ verify failed · verify_failed")
  })

  test("failed without errorClass → bad, no suffix", () => {
    const v = verifyText(row({ state: "failed", errorClass: null }))
    expect(v.tone).toBe("bad")
    expect(v.text).toBe("✗ verify failed")
  })

  test("cancelled → muted", () => {
    const v = verifyText(row({ state: "cancelled" }))
    expect(v.tone).toBe("muted")
    expect(v.text).toBe("cancelled")
  })

  test("running → muted, ellipsis line", () => {
    const v = verifyText(row({ state: "running" }))
    expect(v.tone).toBe("muted")
    expect(v.text).toBe("running…")
  })
})
