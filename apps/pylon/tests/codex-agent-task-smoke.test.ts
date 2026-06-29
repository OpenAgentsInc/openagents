import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  codexAgentSmokeLease,
  ciHarness,
  runCodexAgentTaskCiSmoke,
  scanRetainedProjection,
} from "../src/codex-agent-task-smoke"
import { CODEX_AGENT_CAPABILITY_REF, CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import { computeAssignmentAdmission, runNoSpendAssignment } from "../src/assignment"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { sendHeartbeat } from "../src/presence"
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

  test("run-no-spend auto-routes codex assignments to a ready connected account", async () => {
    const home = await mkdtemp(join(tmpdir(), "pylon-codex-account-route-test-"))
    const defaultCodexHome = join(home, "default-codex")
    const accountAHome = join(home, "accounts", "codex", "codex-a")
    const accountBHome = join(home, "accounts", "codex", "codex-b")
    const lease = codexAgentSmokeLease({
      assignmentRef: "assignment.public.codex_agent_task.account_route",
      leaseRef: "lease.public.codex_agent_task.account_route",
    })
    const harness = ciHarness(lease)
    const seenAccountRefs: string[] = []

    try {
      await mkdir(defaultCodexHome, { recursive: true })
      await mkdir(accountAHome, { recursive: true })
      await mkdir(accountBHome, { recursive: true })
      await writeFile(join(accountAHome, "auth.json"), "{}\n")
      await writeFile(join(accountBHome, "auth.json"), "{}\n")

      const summary = createBootstrapSummary(
        parseBootstrapArgs(["--json"]),
        { CODEX_HOME: defaultCodexHome, PYLON_HOME: home },
        "darwin",
      )
      const state = await ensurePylonLocalState(summary)
      await writeFile(
        state.paths.runtimeState,
        `${JSON.stringify({
          lifecycle: "assignment-ready",
          displayName: "Codex Account Route Test",
          resourceMode: "background_20",
          capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
          blockerRefs: [],
          updatedAt: new Date().toISOString(),
        })}\n`,
      )
      await writeFile(
        state.paths.config,
        `${JSON.stringify({
          dev: {
            accounts: [
              { provider: "codex", ref: "codex-a", home: accountAHome },
              { provider: "codex", ref: "codex-b", home: accountBHome },
            ],
          },
        }, null, 2)}\n`,
      )
      await sendHeartbeat(summary, { baseUrl: harness.baseUrl })

      const run = await runNoSpendAssignment(summary, {
        baseUrl: harness.baseUrl,
        codexAgentRunner: async (input) => {
          seenAccountRefs.push(input.account?.accountRef ?? "default")
          await writeFile(
            join(input.cwd, "sum.ts"),
            "export const sum = (left: number, right: number) => left + right\n",
          )
          return {
            outcome: "completed",
            turnCount: 1,
            editedFileCount: 1,
            commandCount: 1,
            sessionRef: null,
          }
        },
        codexAgentProbe: {
          env: { CODEX_HOME: defaultCodexHome, PYLON_HOME: home },
          importer: async (specifier: string) => {
            if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
            return {}
          },
          platform: "darwin",
        },
      })

      expect(run.ok).toBe(true)
      expect(seenAccountRefs).toHaveLength(1)
      expect(["codex-a", "codex-b"]).toContain(seenAccountRefs[0])
    } finally {
      harness.stop()
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
