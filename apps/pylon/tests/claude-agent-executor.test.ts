import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
  CLAUDE_AGENT_TASK_SCHEMA,
  claudeAgentTaskFrom,
  executeClaudeAgentAssignment,
  toolInputEscapesWorkspace,
  type ClaudeAgentRunner,
} from "../src/claude-agent-executor"
import { CLAUDE_AGENT_SDK_PACKAGE } from "../src/claude-agent"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { ensurePylonLocalState, assertPublicProjectionSafe } from "../src/state"

const now = new Date("2026-06-10T22:00:00.000Z")

const readyProbe = {
  env: { ANTHROPIC_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CLAUDE_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
}

const notReadyProbe = {
  env: {},
  platform: "darwin",
  importer: async () => {
    throw new Error("Cannot find module")
  },
}

const claudeAgentCodingAssignment = {
  claudeAgent: {
    schema: CLAUDE_AGENT_TASK_SCHEMA,
    agentKind: "claude_agent_sdk",
    fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF,
    maxTurns: 8,
    timeoutSeconds: 120,
  },
}

const lease = {
  assignmentRef: "assignment.public.claude_agent.test",
  leaseRef: "lease.public.claude_agent.test",
  codingAssignment: claudeAgentCodingAssignment,
}

const fixingRunner: ClaudeAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return { outcome: "completed", turnCount: 3, editedFileCount: 1, commandCount: 1, sessionRef: null }
}

const idleRunner: ClaudeAgentRunner = async () => ({
  outcome: "completed",
  turnCount: 1,
  editedFileCount: 0,
  commandCount: 0,
  sessionRef: null,
})

async function withState<T>(fn: (state: Awaited<ReturnType<typeof ensurePylonLocalState>>) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-claude-exec-test-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const state = await ensurePylonLocalState(summary)
    return await fn(state)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("claude agent task recognition", () => {
  test("recognizes the typed work class and passes everything else through", async () => {
    expect(claudeAgentTaskFrom(claudeAgentCodingAssignment)).not.toBeNull()
    expect(claudeAgentTaskFrom({ kind: "job.public.tassadar_executor_trace", tassadar: {} })).toBeNull()
    expect(claudeAgentTaskFrom({ claudeAgent: { schema: "wrong", agentKind: "claude_agent_sdk", fixtureRef: CLAUDE_AGENT_SUM_REPAIR_FIXTURE_REF } })).toBeNull()
    expect(claudeAgentTaskFrom({ claudeAgent: { schema: CLAUDE_AGENT_TASK_SCHEMA, agentKind: "claude_agent_sdk", fixtureRef: "fixture.unknown" } })).toBeNull()
    expect(claudeAgentTaskFrom(undefined)).toBeNull()

    await withState(async (state) => {
      const passthrough = await executeClaudeAgentAssignment(
        state,
        { ...lease, codingAssignment: { kind: "something_else" } },
        now,
        { claudeAgentRunner: fixingRunner, claudeAgentProbe: readyProbe },
      )
      expect(passthrough).toBeNull()
    })
  })

  test("executes the fixture task, verifies with the real test command, accepts", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: readyProbe,
      })
      expect(record).not.toBeNull()
      expect(record?.status).toBe("accepted")
      expect(record?.blockerRefs).toEqual([])
      expect(record?.resultRefs).toContain("result.public.pylon.claude_agent_task.fixture_repair_passed")
      expect(record?.artifactRefs[0]).toStartWith("artifact.pylon.claude_agent_task.patch.")
      expect(record?.testRefs[0]).toStartWith("command.pylon.claude_agent_task.verification.")
      assertPublicProjectionSafe(record)
      const projected = JSON.stringify(record)
      expect(projected).not.toContain(state.paths.cache)
      expect(projected).not.toContain("bounded fixture workspace")
    })
  })

  test("rejects with claude_agent_test_failed when the agent does not fix the fixture", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: idleRunner,
        claudeAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual(["blocker.assignment.claude_agent_test_failed"])
      assertPublicProjectionSafe(record)
    })
  })

  test("refuses with typed blockers when the lane is not ready", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: notReadyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toContain("blocker.assignment.claude_agent_unavailable")
      expect(record?.blockerRefs).toContain("blocker.claude_agent.sdk_missing")
      assertPublicProjectionSafe(record)
    })
  })

  test("maps escape, budget, refusal, and thrown-runner outcomes to typed blockers", async () => {
    await withState(async (state) => {
      const outcomes = [
        { outcome: "workspace_escape_blocked", blocker: "blocker.assignment.claude_agent_workspace_escape_blocked" },
        { outcome: "budget_exceeded", blocker: "blocker.assignment.claude_agent_budget_exceeded" },
        { outcome: "refused", blocker: "blocker.assignment.claude_agent_execution_refused" },
      ] as const
      for (const { outcome, blocker } of outcomes) {
        const runner: ClaudeAgentRunner = async () => ({
          outcome,
          turnCount: 2,
          editedFileCount: 0,
          commandCount: 0,
          sessionRef: null,
        })
        const record = await executeClaudeAgentAssignment(state, lease, now, {
          claudeAgentRunner: runner,
          claudeAgentProbe: readyProbe,
        })
        expect(record?.status).toBe("rejected")
        expect(record?.blockerRefs).toEqual([blocker])
        assertPublicProjectionSafe(record)
      }

      const throwingRunner: ClaudeAgentRunner = async () => {
        throw new Error("sdk exploded")
      }
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: throwingRunner,
        claudeAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual(["blocker.assignment.claude_agent_execution_refused"])
    })
  })

  test("redaction: unsafe-shaped digests are rejected before any POST", async () => {
    await withState(async (state) => {
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: fixingRunner,
        claudeAgentProbe: readyProbe,
      })
      const tampered = { ...record, message: `${record?.message} raw prompt follows` }
      expect(() => assertPublicProjectionSafe(tampered)).toThrow()
      const keyTampered = { ...record, cachePath: "/leak" }
      expect(() => assertPublicProjectionSafe(keyTampered)).toThrow()
      const credentialTampered = { ...record, summaryRefs: ["bearer abc123"] }
      expect(() => assertPublicProjectionSafe(credentialTampered)).toThrow()
    })
  })

  test("the agent's edit is real: workspace file changed and bun test passes", async () => {
    await withState(async (state) => {
      let workspaceDir: string | null = null
      const observingRunner: ClaudeAgentRunner = async (input) => {
        workspaceDir = input.cwd
        return fixingRunner(input)
      }
      const record = await executeClaudeAgentAssignment(state, lease, now, {
        claudeAgentRunner: observingRunner,
        claudeAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("accepted")
      expect(workspaceDir).not.toBeNull()
      const fixed = await readFile(join(workspaceDir as unknown as string, "sum.ts"), "utf8")
      expect(fixed).toContain("left + right")
    })
  })
})

describe("workspace boundary checks", () => {
  const workspace = "/var/pylon-test/workspace"

  test("path fields outside the workspace escape", () => {
    expect(toolInputEscapesWorkspace("Edit", { file_path: "/etc/passwd" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Edit", { file_path: `${workspace}/sum.ts` }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Read", { path: "../outside.ts" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Read", { path: "sum.ts" }, workspace)).toBe(false)
  })

  test("bash commands with traversal or foreign absolute paths escape", () => {
    expect(toolInputEscapesWorkspace("Bash", { command: "cat ../secrets" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "cat /etc/passwd" }, workspace)).toBe(true)
    expect(toolInputEscapesWorkspace("Bash", { command: "bun test sum.test.ts" }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: `cat ${workspace}/sum.ts` }, workspace)).toBe(false)
    expect(toolInputEscapesWorkspace("Bash", { command: "/usr/bin/env bun test" }, workspace)).toBe(false)
  })
})
