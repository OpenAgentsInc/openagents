import { describe, expect, test } from "bun:test"

import type { KhalaAutoExecutionTargetResolution, KhalaModelPreference } from "../src/sync/khala-mobile-model-preference-api"
import {
  autoResolutionNoticeMessage,
  buildExecutionTargetOptions,
  executionTargetDisplayLabel,
  modelDisplayLabel,
  modelPreferenceFallbackMessage,
} from "../src/sync/khala-mobile-model-preference-format-core"

describe("executionTargetDisplayLabel", () => {
  test("labels execution aliases", () => {
    expect(executionTargetDisplayLabel("auto")).toBe("Auto")
    expect(executionTargetDisplayLabel("gemini")).toBe("Gemini")
    expect(executionTargetDisplayLabel("khala")).toBe("Khala")
  })

  test("labels account-specific targets without exposing account refs", () => {
    expect(executionTargetDisplayLabel("codex:acct_ref_hash")).toBe("Your Codex")
    expect(executionTargetDisplayLabel("claude:acct_ref_hash")).toBe("Claude")
  })
})

describe("modelDisplayLabel", () => {
  test("labels the gemini alias specially", () => {
    expect(modelDisplayLabel("gemini")).toBe("Gemini")
  })

  test("title-cases hyphenated/underscored ids", () => {
    expect(modelDisplayLabel("vertex-anthropic-claude")).toBe("Vertex Anthropic Claude")
  })

  test("uppercases short acronym-like parts", () => {
    expect(modelDisplayLabel("gpt-oss-120b")).toBe("GPT OSS 120b")
  })
})

describe("modelPreferenceFallbackMessage", () => {
  test("is silent for the expected/quiet fallback cases", () => {
    expect(modelPreferenceFallbackMessage("none")).toBeNull()
    expect(modelPreferenceFallbackMessage("no_preference_set")).toBeNull()
  })

  test("explains preference_unavailable", () => {
    expect(modelPreferenceFallbackMessage("preference_unavailable")).toContain("default instead")
  })

  test("explains default_unavailable", () => {
    expect(modelPreferenceFallbackMessage("default_unavailable")).toContain("Try again")
  })
})

// CX-4 (#8548): the per-thread picker's real-account option list.
describe("buildExecutionTargetOptions", () => {
  const basePreference: Pick<
    KhalaModelPreference,
    "autoResolution" | "availableTargetIds" | "claudeAccounts" | "codexAccounts"
  > = {
    autoResolution: null,
    availableTargetIds: ["khala"],
    claudeAccounts: [],
    codexAccounts: [],
  }

  test("includes Khala when available, and nothing else when there are no accounts/auto", () => {
    expect(buildExecutionTargetOptions(basePreference)).toEqual([
      { label: "Khala", target: { executionTargetId: "khala", lane: "hosted_khala" } },
    ])
  })

  test("adds a ready Codex account as its own selectable target", () => {
    const options = buildExecutionTargetOptions({
      ...basePreference,
      codexAccounts: [{ accountRefHash: "acct-a", label: "Your Codex", ready: true }],
    })
    expect(options).toContainEqual({
      label: "Your Codex",
      target: { executionTargetId: "codex:acct-a", lane: "codex_app_server" },
    })
  })

  test("labels a not-ready account with its reason, never silently hiding it", () => {
    const options = buildExecutionTargetOptions({
      ...basePreference,
      codexAccounts: [
        { accountRefHash: "acct-a", label: "Your Codex", reason: "account_exhausted", ready: false },
      ],
    })
    expect(options).toContainEqual({
      label: "Your Codex (exhausted)",
      target: { executionTargetId: "codex:acct-a", lane: "codex_app_server" },
    })
  })

  test("resolves the Auto pill to a CONCRETE target, never the literal 'auto' string", () => {
    const options = buildExecutionTargetOptions({
      ...basePreference,
      autoResolution: { effectiveTargetId: "codex:acct-a", events: [], usedFallback: false },
      availableTargetIds: ["khala", "auto"],
      codexAccounts: [{ accountRefHash: "acct-a", label: "Your Codex", ready: true }],
    })
    expect(options).toContainEqual({
      label: "Auto",
      target: { executionTargetId: "codex:acct-a", lane: "codex_app_server" },
    })
  })

  test("omits Auto when its resolution hasn't landed on anything yet", () => {
    const options = buildExecutionTargetOptions({
      ...basePreference,
      autoResolution: { effectiveTargetId: null, events: [], usedFallback: true },
      availableTargetIds: ["khala", "auto"],
    })
    expect(options.some(option => option.label === "Auto")).toBe(false)
  })

  test("tolerates the pre-CX-4 shape (fields absent) without crashing", () => {
    expect(buildExecutionTargetOptions({ availableTargetIds: ["khala"] } as never)).toEqual([
      { label: "Khala", target: { executionTargetId: "khala", lane: "hosted_khala" } },
    ])
  })
})

// CX-4 (#8548): the typed, never-silent "what did auto do" line.
describe("autoResolutionNoticeMessage", () => {
  test("is null when there's nothing to report", () => {
    expect(autoResolutionNoticeMessage(null)).toBeNull()
    expect(autoResolutionNoticeMessage(undefined)).toBeNull()
    expect(
      autoResolutionNoticeMessage({ effectiveTargetId: "codex:acct-a", events: [], usedFallback: false }),
    ).toBeNull()
  })

  test("names what was skipped, why, and what it fell through to", () => {
    const resolution: KhalaAutoExecutionTargetResolution = {
      effectiveTargetId: "codex:acct-b",
      events: [{ nextTargetId: "codex:acct-b", targetId: "codex:acct-a", type: "account_exhausted" }],
      usedFallback: true,
    }
    const message = autoResolutionNoticeMessage(resolution)
    expect(message).toContain("Your Codex")
    expect(message).toContain("exhausted")
    expect(message).toContain("using Your Codex")
  })

  test("reports the fallback-to-nothing case honestly", () => {
    const resolution: KhalaAutoExecutionTargetResolution = {
      effectiveTargetId: null,
      events: [{ nextTargetId: null, targetId: "codex:acct-a", type: "account_exhausted" }],
      usedFallback: true,
    }
    expect(autoResolutionNoticeMessage(resolution)).toContain("nothing available")
  })
})
