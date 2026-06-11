import { describe, expect, test } from "bun:test"
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
  CODEX_AGENT_TASK_SCHEMA,
  codexAgentTaskFrom,
  effectiveSandboxMode,
  executeCodexAgentAssignment,
  fileChangeEscapesWorkspace,
  type CodexAgentRunner,
} from "../src/codex-agent-executor"
import { CODEX_AGENT_SDK_PACKAGE } from "../src/codex-agent"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { ensurePylonLocalState, assertPublicProjectionSafe } from "../src/state"

const now = new Date("2026-06-11T22:00:00.000Z")

const readyProbe = {
  env: { CODEX_API_KEY: "test-key-shape" },
  platform: "darwin",
  importer: async (specifier: string) => {
    if (specifier !== CODEX_AGENT_SDK_PACKAGE) throw new Error("unexpected import")
    return {}
  },
  codexCliLoginPresent: false,
}

const notReadyProbe = {
  env: {},
  platform: "darwin",
  importer: async () => {
    throw new Error("Cannot find module")
  },
  codexCliLoginPresent: false,
}

const codexAgentCodingAssignment = {
  codex: {
    schema: CODEX_AGENT_TASK_SCHEMA,
    agentKind: "codex_sdk",
    fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF,
    timeoutSeconds: 120,
  },
}

const lease = {
  assignmentRef: "assignment.public.codex_agent.test",
  leaseRef: "lease.public.codex_agent.test",
  codingAssignment: codexAgentCodingAssignment,
}

const fixingRunner: CodexAgentRunner = async (input) => {
  await writeFile(
    join(input.cwd, "sum.ts"),
    "export const sum = (left: number, right: number) => left + right\n",
  )
  return { outcome: "completed", turnCount: 1, editedFileCount: 1, commandCount: 1, sessionRef: null }
}

const idleRunner: CodexAgentRunner = async () => ({
  outcome: "completed",
  turnCount: 1,
  editedFileCount: 0,
  commandCount: 0,
  sessionRef: null,
})

async function withState<T>(fn: (state: Awaited<ReturnType<typeof ensurePylonLocalState>>) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-codex-exec-test-"))
  try {
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    const state = await ensurePylonLocalState(summary)
    return await fn(state)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

describe("codex agent task recognition", () => {
  test("recognizes the typed work class and passes everything else through", async () => {
    expect(codexAgentTaskFrom(codexAgentCodingAssignment)).not.toBeNull()
    expect(codexAgentTaskFrom({ kind: "job.public.tassadar_executor_trace", tassadar: {} })).toBeNull()
    expect(codexAgentTaskFrom({ claudeAgent: { schema: "openagents.pylon.claude_agent_task.v0.3" } })).toBeNull()
    expect(codexAgentTaskFrom({ codex: { schema: "wrong", agentKind: "codex_sdk", fixtureRef: CODEX_AGENT_SUM_REPAIR_FIXTURE_REF } })).toBeNull()
    expect(codexAgentTaskFrom({ codex: { schema: CODEX_AGENT_TASK_SCHEMA, agentKind: "codex_sdk", fixtureRef: "fixture.unknown" } })).toBeNull()
    expect(codexAgentTaskFrom(undefined)).toBeNull()

    await withState(async (state) => {
      const passthrough = await executeCodexAgentAssignment(
        state,
        { ...lease, codingAssignment: { kind: "something_else" } },
        now,
        { codexAgentRunner: fixingRunner, codexAgentProbe: readyProbe },
      )
      expect(passthrough).toBeNull()
    })
  })

  test("a claude_agent_task lease passes through this gate untouched", async () => {
    await withState(async (state) => {
      const passthrough = await executeCodexAgentAssignment(
        state,
        {
          ...lease,
          codingAssignment: {
            kind: "claude_agent_task",
            claudeAgent: {
              schema: "openagents.pylon.claude_agent_task.v0.3",
              agentKind: "claude_agent_sdk",
              fixtureRef: "fixture.public.pylon.claude_agent.sum_repair.v1",
            },
          },
        },
        now,
        { codexAgentRunner: fixingRunner, codexAgentProbe: readyProbe },
      )
      expect(passthrough).toBeNull()
    })
  })

  test("executes the fixture task, verifies with the real test command, accepts", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: fixingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record).not.toBeNull()
      expect(record?.status).toBe("accepted")
      expect(record?.blockerRefs).toEqual([])
      expect(record?.resultRefs).toContain("result.public.pylon.codex_agent_task.fixture_repair_passed")
      expect(record?.artifactRefs[0]).toStartWith("artifact.pylon.codex_agent_task.patch.")
      expect(record?.testRefs[0]).toStartWith("command.pylon.codex_agent_task.verification.")
      assertPublicProjectionSafe(record)
      const projected = JSON.stringify(record)
      expect(projected).not.toContain(state.paths.cache)
      expect(projected).not.toContain("bounded fixture workspace")
    })
  })

  test("rejects with codex_agent_test_failed when the agent does not fix the fixture", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: idleRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual(["blocker.assignment.codex_agent_test_failed"])
      assertPublicProjectionSafe(record)
    })
  })

  test("refuses with typed blockers when the lane is not ready", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: fixingRunner,
        codexAgentProbe: notReadyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toContain("blocker.assignment.codex_agent_unavailable")
      expect(record?.blockerRefs).toContain("blocker.codex_agent.sdk_missing")
      assertPublicProjectionSafe(record)
    })
  })

  test("maps escape, budget, refusal, and thrown-runner outcomes to typed blockers", async () => {
    await withState(async (state) => {
      const outcomes = [
        { outcome: "workspace_escape_blocked", blocker: "blocker.assignment.codex_agent_workspace_escape_blocked" },
        { outcome: "budget_exceeded", blocker: "blocker.assignment.codex_agent_budget_exceeded" },
        { outcome: "refused", blocker: "blocker.assignment.codex_agent_execution_refused" },
      ] as const
      for (const { outcome, blocker } of outcomes) {
        const runner: CodexAgentRunner = async () => ({
          outcome,
          turnCount: 1,
          editedFileCount: 0,
          commandCount: 0,
          sessionRef: null,
        })
        const record = await executeCodexAgentAssignment(state, lease, now, {
          codexAgentRunner: runner,
          codexAgentProbe: readyProbe,
        })
        expect(record?.status).toBe("rejected")
        expect(record?.blockerRefs).toEqual([blocker])
        assertPublicProjectionSafe(record)
      }

      const throwingRunner: CodexAgentRunner = async () => {
        throw new Error("sdk exploded")
      }
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: throwingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("rejected")
      expect(record?.blockerRefs).toEqual(["blocker.assignment.codex_agent_execution_refused"])
    })
  })

  test("redaction: unsafe-shaped digests are rejected before any POST", async () => {
    await withState(async (state) => {
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: fixingRunner,
        codexAgentProbe: readyProbe,
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
      const observingRunner: CodexAgentRunner = async (input) => {
        workspaceDir = input.cwd
        return fixingRunner(input)
      }
      const record = await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: observingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(record?.status).toBe("accepted")
      expect(workspaceDir).not.toBeNull()
      const fixed = await readFile(join(workspaceDir as unknown as string, "sum.ts"), "utf8")
      expect(fixed).toContain("left + right")
    })
  })

  test("the runner receives the bounded sandbox mode, never full access", async () => {
    await withState(async (state) => {
      let seenMode: string | null = null
      const observingRunner: CodexAgentRunner = async (input) => {
        seenMode = input.sandboxMode
        return fixingRunner(input)
      }
      await executeCodexAgentAssignment(state, lease, now, {
        codexAgentRunner: observingRunner,
        codexAgentProbe: readyProbe,
      })
      expect(seenMode).toBe("workspace-write")
    })
  })
})

describe("sandbox mode resolution", () => {
  test("read-only requested anywhere wins; default is workspace-write", () => {
    expect(effectiveSandboxMode(undefined, undefined)).toBe("workspace-write")
    expect(effectiveSandboxMode("workspace-write", undefined)).toBe("workspace-write")
    expect(effectiveSandboxMode("read-only", "workspace-write")).toBe("read-only")
    expect(effectiveSandboxMode("workspace-write", "read-only")).toBe("read-only")
  })
})

describe("post-hoc workspace boundary checks", () => {
  const workspace = "/var/pylon-test/workspace"

  test("file changes outside the workspace escape", () => {
    expect(fileChangeEscapesWorkspace("/etc/passwd", workspace)).toBe(true)
    expect(fileChangeEscapesWorkspace(`${workspace}/sum.ts`, workspace)).toBe(false)
    expect(fileChangeEscapesWorkspace("../outside.ts", workspace)).toBe(true)
    expect(fileChangeEscapesWorkspace("sum.ts", workspace)).toBe(false)
    expect(fileChangeEscapesWorkspace(`${workspace}-sibling/sum.ts`, workspace)).toBe(true)
  })
})
