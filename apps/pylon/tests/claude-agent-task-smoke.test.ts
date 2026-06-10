import { describe, expect, test } from "bun:test"
import {
  claudeAgentSmokeLease,
  runClaudeAgentTaskCiSmoke,
  scanRetainedProjection,
} from "../src/claude-agent-task-smoke"
import { CLAUDE_AGENT_CAPABILITY_REF } from "../src/claude-agent"
import { assertPublicProjectionSafe } from "../src/state"

describe("claude agent task smoke (CI-safe leg)", () => {
  test("full worker-loop lifecycle delivers an accepted, redacted closeout", async () => {
    const result = await runClaudeAgentTaskCiSmoke()
    expect(result.ok).toBe(true)
    expect(result.mode).toBe("ci_safe")
    expect(result.closeoutStatus).toBe("accepted")
    expect(result.closeoutRef).toContain("assignment.closeout.")
    expect(result.resultRefs).toContain("result.public.pylon.claude_agent_task.fixture_repair_passed")
    expect(result.blockerRefs).toEqual([])
    expect(result.boundaryChecks).toEqual({
      paymentMode: "no-spend",
      settlementState: "not_applicable",
      payoutClaimAllowed: false,
      redacted: true,
    })
    expect(result.redactionScan.scannedRequestCount).toBeGreaterThan(3)
    expect(result.redactionScan.violations).toEqual([])
    assertPublicProjectionSafe(result)
  })

  test("the smoke lease payload is public-safe and capability-gated", () => {
    const lease = claudeAgentSmokeLease()
    assertPublicProjectionSafe(lease)
    expect(lease.capabilityRefs).toContain(CLAUDE_AGENT_CAPABILITY_REF)
    const coding = lease.codingAssignment as { requiredCapabilityRefs?: string[] }
    expect(coding.requiredCapabilityRefs).toContain(CLAUDE_AGENT_CAPABILITY_REF)
    expect(JSON.stringify(lease)).not.toContain("bounded fixture workspace")
  })

  test("the redaction scanner catches the material it exists to catch", () => {
    expect(scanRetainedProjection("clean refs only")).toEqual([])
    expect(scanRetainedProjection("/Users/someone/leak")).toContain("redaction.local_user_path")
    expect(scanRetainedProjection("ANTHROPIC_API_KEY=oops")).toContain("redaction.anthropic_env_name")
    expect(scanRetainedProjection("sk-ant-abc123")).toContain("redaction.anthropic_key_shape")
    expect(scanRetainedProjection("the bounded fixture workspace instructions")).toContain(
      "redaction.instruction_text",
    )
  })
})
