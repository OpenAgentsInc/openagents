import { describe, expect, test } from "vite-plus/test"

import { decodeClaudeLocalStartRequest } from "./claude-local-contract.ts"

const request = (target: unknown) => ({
  turnRef: "turn-target",
  threadRef: "thread-target",
  message: "run this",
  target,
})

describe("local provider target contract", () => {
  test("accepts each exact bundled provider/model target", () => {
    expect(decodeClaudeLocalStartRequest(request({
      provider: "codex",
      accountRef: "codex-2",
      model: "gpt-5.6-sol",
    }))?.target).toEqual({
      provider: "codex",
      accountRef: "codex-2",
      model: "gpt-5.6-sol",
    })
    expect(decodeClaudeLocalStartRequest(request({
      provider: "claude_agent",
      accountRef: "claude-pylon-2",
      model: "claude-fable-5",
    }))?.target).toEqual({
      provider: "claude_agent",
      accountRef: "claude-pylon-2",
      model: "claude-fable-5",
    })
  })

  test("rejects unknown providers plus empty and unbounded target fields", () => {
    expect(decodeClaudeLocalStartRequest(request({
      provider: "other",
      accountRef: "codex",
      model: "gpt-5.6-sol",
    }))).toBeNull()
    expect(decodeClaudeLocalStartRequest(request({
      provider: "codex",
      accountRef: "",
      model: "gpt-5.6-sol",
    }))).toBeNull()
    expect(decodeClaudeLocalStartRequest(request({
      provider: "codex",
      accountRef: "x".repeat(81),
      model: "gpt-5.6-sol",
    }))).toBeNull()
    expect(decodeClaudeLocalStartRequest(request({
      provider: "codex",
      accountRef: "codex",
      model: "gpt-unknown",
    }))?.target?.model).toBe("gpt-unknown")
    expect(decodeClaudeLocalStartRequest(request({
      provider: "codex",
      accountRef: "codex",
      model: "unknown-provider-model",
    }))).toBeNull()
  })

  test("strips excess target fields at the boundary", () => {
    expect(decodeClaudeLocalStartRequest(request({
      provider: "codex",
      accountRef: "codex",
      model: "gpt-5.6-sol",
      token: "must-not-cross",
    }))?.target).toEqual({
      provider: "codex",
      accountRef: "codex",
      model: "gpt-5.6-sol",
    })
  })

  test("accepts only bounded explicit local skill selections", () => {
    const value = {
      ...request({ provider: "claude_agent", accountRef: "claude-pylon-2", model: "claude-fable-5" }),
      skill: { pluginRef: "plugin.local.0123456789abcdef01234567", name: "review" },
    }
    expect(decodeClaudeLocalStartRequest(value)?.skill).toEqual(value.skill)
    expect(decodeClaudeLocalStartRequest({ ...value, skill: { ...value.skill, name: "../escape" } })).toBeNull()
    expect(decodeClaudeLocalStartRequest({ ...value, skill: { ...value.skill, pluginRef: "raw-path" } })).toBeNull()
  })

  test("accepts only the closed local permission modes", () => {
    const value = { ...request(undefined), permissionMode: "plan_only" }
    expect(decodeClaudeLocalStartRequest(value)?.permissionMode).toBe("plan_only")
    expect(decodeClaudeLocalStartRequest({ ...value, permissionMode: "bypassPermissions" })).toBeNull()
  })
})
