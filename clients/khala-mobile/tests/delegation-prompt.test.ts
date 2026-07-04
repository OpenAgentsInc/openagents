import { describe, expect, test } from "bun:test"

import {
  assertDelegationPrompt,
  validateDelegationPrompt
} from "../src/security/delegation-prompt"

describe("Khala mobile delegation prompt validation", () => {
  test("accepts bounded public-safe requests", () => {
    expect(
      assertDelegationPrompt("Summarize the latest fleet run and list blocker refs."),
    ).toBe("Summarize the latest fleet run and list blocker refs.")
  })

  test("rejects local paths and secrets", () => {
    expect(validateDelegationPrompt("open ~/.codex/auth.json").blockerRefs).toContain(
      "blocker.khala_mobile.prompt.codex_auth_path",
    )
    expect(validateDelegationPrompt("Bearer abcdefghijklmnop").blockerRefs).toContain(
      "blocker.khala_mobile.prompt.bearer_token",
    )
    expect(validateDelegationPrompt("oa_agent_secret_value").blockerRefs).toContain(
      "blocker.khala_mobile.prompt.openagents_api_key",
    )
    expect(validateDelegationPrompt("/Users/alice/.secrets/key").blockerRefs).toContain(
      "blocker.khala_mobile.prompt.local_path",
    )
  })

  test("normalizes whitespace and enforces length bounds", () => {
    expect(assertDelegationPrompt("  Check   fleet   ")).toBe("Check fleet")
    expect(validateDelegationPrompt("  ").blockerRefs).toContain(
      "blocker.khala_mobile.prompt_too_short",
    )
    expect(validateDelegationPrompt("a".repeat(2_001)).blockerRefs).toContain(
      "blocker.khala_mobile.prompt_too_long",
    )
  })
})
