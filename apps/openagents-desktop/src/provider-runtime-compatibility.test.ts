import { describe, expect, test } from "bun:test"

import {
  classifyProviderRuntimeCompatibility,
  decodeProviderRuntimeCompatibility,
  supportedProviderRuntimeVersions,
} from "./provider-runtime-compatibility.ts"

describe("provider runtime compatibility", () => {
  test("accepts only the exact bundled and receipted Codex CLI version", () => {
    expect(classifyProviderRuntimeCompatibility("codex_cli", "codex-cli 0.144.1")).toMatchObject({ state: "compatible", reason: "verified" })
    expect(classifyProviderRuntimeCompatibility("codex_cli", "codex 0.144.2")).toMatchObject({ state: "incompatible", observedVersion: "0.144.2" })
    expect(supportedProviderRuntimeVersions.codex_cli).toBe("0.144.1")
  })

  test("accepts only the exact bundled and receipted Claude Agent SDK version", () => {
    expect(classifyProviderRuntimeCompatibility("claude_agent_sdk", "0.3.172")).toMatchObject({ state: "compatible", reason: "verified" })
    expect(classifyProviderRuntimeCompatibility("claude_agent_sdk", "@anthropic-ai/claude-agent-sdk 0.4.0")).toMatchObject({ state: "incompatible", observedVersion: "0.4.0" })
    expect(supportedProviderRuntimeVersions.claude_agent_sdk).toBe("0.3.172")
  })

  test("missing and malformed observations are explicit and public-safe", () => {
    expect(classifyProviderRuntimeCompatibility("codex_cli", null)).toMatchObject({ state: "missing", observedVersion: null, reason: "not_found" })
    const malformed = classifyProviderRuntimeCompatibility("codex_cli", "token=sk-abcdefghijklmnop")
    expect(malformed).toMatchObject({ state: "malformed", observedVersion: null, reason: "unreadable_version" })
    expect(JSON.stringify(malformed)).not.toContain("sk-")
  })

  test("the decoder rejects unknown providers, states, and leaked fields", () => {
    const valid = classifyProviderRuntimeCompatibility("claude_agent_sdk", "0.3.172")
    expect(decodeProviderRuntimeCompatibility(valid)).toEqual(valid)
    expect(decodeProviderRuntimeCompatibility({ ...valid, kind: "shell" })).toBeNull()
    expect(decodeProviderRuntimeCompatibility({ ...valid, state: "probably" })).toBeNull()
  })
})
