import { describe, expect, test } from "bun:test"
import {
  CLAUDE_SECOND_PASS_REVIEW_SCHEMA,
  buildClaudeSecondPassReviewPrompt,
  parseClaudeSecondPassVerdict,
  runClaudeSecondPassReview,
} from "./claude-second-pass-reviewer.js"
import { CLAUDE_AGENT_SDK_PACKAGE } from "./claude-agent.js"
import type { ResolvedPylonAccountSelection } from "./account-registry.js"

const claudeAccount: ResolvedPylonAccountSelection = {
  provider: "claude_agent",
  selector: "accountRef",
  accountRef: "claude-reviewer",
  accountRefHash: "account.pylon.claude_agent.review",
  home: "/tmp/pylon-claude-reviewer-home",
}

describe("Claude second-pass reviewer", () => {
  test("parses structured json_schema verdict shapes", () => {
    expect(parseClaudeSecondPassVerdict(JSON.stringify({
      schema: CLAUDE_SECOND_PASS_REVIEW_SCHEMA,
      recommendation: "request_changes",
      confidence: "high",
      summary: "Diff likely misses an edge case.",
      riskRefs: ["risk.public.pylon.review.edge_case"],
    }))).toMatchObject({
      recommendation: "request_changes",
      riskRefs: ["risk.public.pylon.review.edge_case"],
    })
    expect(parseClaudeSecondPassVerdict({
      schema: CLAUDE_SECOND_PASS_REVIEW_SCHEMA,
      recommendation: "merge_now",
      confidence: "high",
      summary: "bad",
      riskRefs: [],
    })).toBeNull()
  })

  test("builds a bounded review prompt over the closeout diff", () => {
    const prompt = buildClaudeSecondPassReviewPrompt({
      assignmentRef: "assignment.public.t9_5",
      workspace: "/workspace",
      diffText: "diff --git a/sum.ts b/sum.ts",
      verifyCommandRef: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
      verifyCommand: ["bun", "test"],
    })
    expect(prompt).toContain("verification command already passed")
    expect(prompt).toContain("diff --git a/sum.ts b/sum.ts")
    expect(prompt).toContain("Return exactly one JSON object")
  })

  test("runs a mocked Claude SDK session with an isolated Claude account home", async () => {
    let seenOptions: Record<string, unknown> | null = null
    const verdict = await runClaudeSecondPassReview({
      assignmentRef: "assignment.public.t9_5",
      workspace: "/tmp/workspace",
      diffText: "diff --git a/sum.ts b/sum.ts",
      verifyCommandRef: "command.public.pylon_khala.verify.28484fe0b746db06b92c2eb2",
      verifyCommand: ["bun", "test"],
      account: claudeAccount,
      env: {},
      sdkImporter: async (specifier) => {
        expect(specifier).toBe(CLAUDE_AGENT_SDK_PACKAGE)
        return {
          query: (args: unknown) => {
            seenOptions = (args as { options?: Record<string, unknown> }).options ?? null
            return (async function* () {
              yield {
                type: "result",
                result: JSON.stringify({
                  schema: CLAUDE_SECOND_PASS_REVIEW_SCHEMA,
                  recommendation: "approve",
                  confidence: "medium",
                  summary: "No semantic risk found.",
                  riskRefs: [],
                }),
              }
            })()
          },
        }
      },
    })

    expect(verdict.recommendation).toBe("approve")
    expect(seenOptions?.env).toMatchObject({ CLAUDE_CONFIG_DIR: claudeAccount.home })
    expect(seenOptions?.outputFormat).toMatchObject({ type: "json_schema" })
    expect(seenOptions?.permissionMode).toBe("plan")
  })
})
