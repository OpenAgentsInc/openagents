import { describe, expect, test } from "bun:test"
import { parseExplicitSkillInvocation } from "./skill-invocation.ts"

const plugin = {
  ref: "plugin.local.0123456789abcdef01234567" as const,
  name: "review-tools", provider: "claude_agent" as const, provenance: "user_local" as const,
  scope: "app" as const, readiness: "ready" as const, enabled: true,
  restartRequired: false as const, perSessionUse: "next_turn" as const,
  capabilities: ["skills" as const], skills: ["review"],
}

describe("explicit skill slash grammar", () => {
  test("selects one exact catalog skill and leaves only the prompt", () => {
    expect(parseExplicitSkillInvocation("/skill review-tools/review inspect this diff", [plugin])).toEqual({
      kind: "skill", message: "inspect this diff",
      skill: { pluginRef: plugin.ref, name: "review" },
    })
  })
  test("ordinary text is untouched; malformed, absent, and disabled skills fail closed", () => {
    expect(parseExplicitSkillInvocation("please use a skill", [plugin])).toEqual({ kind: "none", message: "please use a skill" })
    expect(parseExplicitSkillInvocation("/skill review-tools/review", [plugin])).toEqual({ kind: "invalid" })
    expect(parseExplicitSkillInvocation("/skill other/review go", [plugin])).toEqual({ kind: "invalid" })
    expect(parseExplicitSkillInvocation("/skill review-tools/review go", [{ ...plugin, enabled: false }])).toEqual({ kind: "invalid" })
  })
})
