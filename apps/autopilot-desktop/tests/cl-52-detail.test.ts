// CL-52 unit tests for pure helpers in session-detail.ts.
// No DOM required — these test only verifyLineText and artifactLineText.

import { describe, expect, test } from "bun:test"
import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { SessionArtifactStats } from "../src/shared/rpc"
import { artifactLineText, verifyLineText } from "../src/ui/panes/session-detail"

// Minimal SessionSummary fixture builder (fields not needed by helpers are
// omitted via cast — the helpers only read state and optional extension fields).
function session(
  state: SessionSummary["state"],
  ext: { artifactRef?: string; errorClass?: string } = {},
): SessionSummary {
  return {
    sessionRef: "sess-test-001",
    adapter: "codex",
    state,
    accountRefHash: null,
    updatedAt: "2026-06-13T00:00:00.000Z",
    ...ext,
  } as unknown as SessionSummary
}

describe("verifyLineText", () => {
  test("completed without artifactRef shows plain verify passed", () => {
    const { text, toneClass } = verifyLineText(session("completed"))
    expect(text).toBe("✓ verify passed")
    expect(toneClass).toBe("verify-completed")
  })

  test("completed with artifactRef appends last-12 slice", () => {
    const artifactRef = "artifact.public.codex.patch.0001"
    const { text, toneClass } = verifyLineText(session("completed", { artifactRef }))
    expect(text).toBe(`✓ verify passed · artifact ${artifactRef.slice(-12)}`)
    expect(toneClass).toBe("verify-completed")
  })

  test("failed without errorClass shows plain verify failed", () => {
    const { text, toneClass } = verifyLineText(session("failed"))
    expect(text).toBe("✗ verify failed")
    expect(toneClass).toBe("verify-failed")
  })

  test("failed with errorClass appends it", () => {
    const { text, toneClass } = verifyLineText(session("failed", { errorClass: "TIMEOUT" }))
    expect(text).toBe("✗ verify failed · TIMEOUT")
    expect(toneClass).toBe("verify-failed")
  })

  test("cancelled shows cancelled with muted tone", () => {
    const { text, toneClass } = verifyLineText(session("cancelled"))
    expect(text).toBe("cancelled")
    expect(toneClass).toBe("verify-cancelled")
  })

  test("running shows state with ellipsis", () => {
    const { text, toneClass } = verifyLineText(session("running"))
    expect(text).toBe("running…")
    expect(toneClass).toBe("verify-cancelled")
  })

  test("queued shows state with ellipsis", () => {
    const { text } = verifyLineText(session("queued"))
    expect(text).toBe("queued…")
  })
})

describe("artifactLineText", () => {
  test("returns empty string when stats is undefined", () => {
    expect(artifactLineText(undefined)).toBe("")
  })

  test("returns empty string when stats is null", () => {
    expect(artifactLineText(null)).toBe("")
  })

  test("renders outcome when present, ignoring kind", () => {
    const stats: SessionArtifactStats = {
      kind: "proof",
      outcome: "passed",
      editedFileCount: null,
      commandCount: null,
      totalTokens: null,
    }
    expect(artifactLineText(stats)).toBe("artifact: passed")
  })

  test("falls back to kind when outcome is null", () => {
    const stats: SessionArtifactStats = {
      kind: "failure",
      outcome: null,
      editedFileCount: null,
      commandCount: null,
      totalTokens: null,
    }
    expect(artifactLineText(stats)).toBe("artifact: failure")
  })

  test("appends editedFileCount when not null", () => {
    const stats: SessionArtifactStats = {
      kind: "proof",
      outcome: "passed",
      editedFileCount: 3,
      commandCount: null,
      totalTokens: null,
    }
    expect(artifactLineText(stats)).toBe("artifact: passed · 3 files")
  })

  test("appends commandCount when not null", () => {
    const stats: SessionArtifactStats = {
      kind: "proof",
      outcome: "passed",
      editedFileCount: null,
      commandCount: 7,
      totalTokens: null,
    }
    expect(artifactLineText(stats)).toBe("artifact: passed · 7 cmds")
  })

  test("appends totalTokens when not null", () => {
    const stats: SessionArtifactStats = {
      kind: "proof",
      outcome: "passed",
      editedFileCount: null,
      commandCount: null,
      totalTokens: 1024,
    }
    expect(artifactLineText(stats)).toBe("artifact: passed · 1024 tok")
  })

  test("renders all counts together", () => {
    const stats: SessionArtifactStats = {
      kind: "proof",
      outcome: "passed",
      editedFileCount: 5,
      commandCount: 12,
      totalTokens: 2048,
    }
    expect(artifactLineText(stats)).toBe("artifact: passed · 5 files · 12 cmds · 2048 tok")
  })

  test("renders with zero counts (falsy but not null)", () => {
    const stats: SessionArtifactStats = {
      kind: "proof",
      outcome: "passed",
      editedFileCount: 0,
      commandCount: 0,
      totalTokens: 0,
    }
    expect(artifactLineText(stats)).toBe("artifact: passed · 0 files · 0 cmds · 0 tok")
  })
})
