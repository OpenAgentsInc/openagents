import { describe, expect, test } from "bun:test"
import { mkdtemp, rm } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  codexAgentSmokeLease,
  runCodexAgentTaskCiSmoke,
  scanRetainedProjection,
} from "../src/codex-agent-task-smoke"
import { CODEX_AGENT_CAPABILITY_REF } from "../src/codex-agent"
import { computeAssignmentAdmission } from "../src/assignment"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { ensurePylonLocalState, assertPublicProjectionSafe } from "../src/state"

describe("codex agent task smoke (CI-safe leg)", () => {
  test("full worker-loop lifecycle delivers an accepted, redacted closeout", async () => {
    const result = await runCodexAgentTaskCiSmoke()
    expect(result.ok).toBe(true)
    expect(result.mode).toBe("ci_safe")
    expect(result.closeoutStatus).toBe("accepted")
    expect(result.closeoutRef).toContain("assignment.closeout.")
    expect(result.resultRefs).toContain("result.public.pylon.codex_agent_task.fixture_repair_passed")
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
    const lease = codexAgentSmokeLease()
    assertPublicProjectionSafe(lease)
    expect(lease.capabilityRefs).toContain(CODEX_AGENT_CAPABILITY_REF)
    const coding = lease.codingAssignment as { requiredCapabilityRefs?: string[] }
    expect(coding.requiredCapabilityRefs).toContain(CODEX_AGENT_CAPABILITY_REF)
    expect(JSON.stringify(lease)).not.toContain("bounded fixture workspace")
  })

  test("admission refuses a codex_agent_task lease on a Pylon without the capability", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-admission-test-"))
    try {
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const state = await ensurePylonLocalState(summary)
      // Fresh state declares no capability.pylon.local_codex.
      expect(state.runtime.capabilityRefs).not.toContain(CODEX_AGENT_CAPABILITY_REF)
      const admission = await computeAssignmentAdmission(state, codexAgentSmokeLease())
      expect(admission.admissible).toBe(false)
      expect(admission.blockerRefs).toContain("blocker.assignment.wrong_capability")
    } finally {
      await rm(home, { recursive: true, force: true })
    }
  })

  test("the redaction scanner catches the material it exists to catch", () => {
    expect(scanRetainedProjection("clean refs only")).toEqual([])
    expect(scanRetainedProjection("/Users/someone/leak")).toContain("redaction.local_user_path")
    expect(scanRetainedProjection("CODEX_API_KEY=oops")).toContain("redaction.codex_env_name")
    expect(scanRetainedProjection("OPENAI_API_KEY=oops")).toContain("redaction.openai_env_name")
    expect(scanRetainedProjection("sk-proj-abcdefghijklmnop")).toContain("redaction.openai_key_shape")
    expect(scanRetainedProjection("a path like .codex/auth.json")).toContain("redaction.codex_home_path")
    expect(scanRetainedProjection("the bounded fixture workspace instructions")).toContain(
      "redaction.instruction_text",
    )
  })
})
