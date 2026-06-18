import { describe, expect, test } from "bun:test"
import { mkdtempSync } from "node:fs"
import { mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  createControlSessionActions,
  type ControlSessionExecutor,
  type ControlSessionExecutorResult,
} from "../src/node/control-sessions"
import {
  parseSessionsBatchTasks,
  runSessionsBatch,
} from "../src/node/sessions-batch"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { PYLON_DEV_CHECK_SCHEMA, type PylonDevCheckProjection } from "../src/dev-loop"
import type { SessionsExecControl } from "../src/node/sessions-exec"

async function withFixture<T>(fn: (fixture: {
  proofDir: string
  pylonHome: string
  summary: ReturnType<typeof createBootstrapSummary>
  worktree: string
}) => Promise<T>) {
  const root = mkdtempSync(join(tmpdir(), "pylon-sessions-batch-"))
  try {
    const pylonHome = join(root, "pylon-home")
    const accountHome = join(root, "codex-home")
    const worktree = join(root, "worktree")
    const proofDir = join(root, "proofs")
    await mkdir(pylonHome, { recursive: true })
    await mkdir(accountHome, { recursive: true })
    await mkdir(worktree, { recursive: true })
    await writeFile(
      join(pylonHome, "config.json"),
      `${JSON.stringify({ dev: { accounts: [{ ref: "codex-a", provider: "codex", home: accountHome }] } })}\n`,
    )
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
    return await fn({ proofDir, pylonHome, summary, worktree })
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

function devCheck(state: "passed" | "failed"): PylonDevCheckProjection {
  return {
    schema: PYLON_DEV_CHECK_SCHEMA,
    observedAt: new Date().toISOString(),
    action: "check",
    state,
    changeSummary: {
      repo: { state: "ready", rootRef: "root.x", branch: "branch.x", commit: "commit.x" },
      dirty: { state: "clean", changedCount: 0, stagedCount: 0, unstagedCount: 0, untrackedCount: 0 },
      changedFileRefs: [],
      areaRefs: [],
      blockerRefs: [],
    },
    checkPlan: { state: "ready", commandRefs: ["command.verify"], blockerRefs: [] },
    commandResults: [
      {
        commandRef: "command.verify",
        reasonRef: "check.verify",
        cwdRef: "command.cwd.x",
        argvRef: "command.argv.verify",
        exitCode: state === "passed" ? 0 : 1,
        status: state === "passed" ? "passed" : "failed",
        durationMs: 1,
        stdoutBytes: 0,
        stderrBytes: 0,
        stdoutDigestRef: null,
        stderrDigestRef: null,
      },
    ],
    latestRecordRef: null,
    branchUntouched: true,
    commitUntouched: true,
    pushPerformed: false,
    blockerRefs: [],
  }
}

function executorResult(state: "passed" | "failed"): ControlSessionExecutorResult {
  return {
    commandCount: 1,
    devCheck: devCheck(state),
    editedFileCount: 0,
    eventCount: 1,
    executionMode: "local_bounded",
    externalSessionRef: null,
    responseDigestRef: null,
    totalTokens: 0,
  }
}

function controlFrom(actions: ReturnType<typeof createControlSessionActions>): SessionsExecControl {
  return {
    spawn: (cmd) => actions.spawn(cmd as never),
    list: () => actions.list() as never,
    events: (ref) => actions.events(ref) as never,
    artifact: (ref) => actions.artifact(ref) as never,
  }
}

describe("pylon sessions batch", () => {
  test("parses string and object task lists", () => {
    expect(parseSessionsBatchTasks(["one", { id: "two", objective: "two objective", verify: ["bun", "--version"] }])).toEqual([
      { id: "task-1", objective: "one" },
      { id: "two", objective: "two objective", verify: ["bun", "--version"] },
    ])
    expect(() => parseSessionsBatchTasks([])).toThrow("must not be empty")
    expect(() => parseSessionsBatchTasks([{ id: "../bad", objective: "x" }])).toThrow("id is invalid")
  })

  test("runs fan-out with a concurrency cap and surfaces per-task failures", async () => {
    await withFixture(async ({ proofDir, summary, worktree }) => {
      let active = 0
      let maxActive = 0
      const executor: ControlSessionExecutor = async (input) => {
        active += 1
        maxActive = Math.max(maxActive, active)
        await Bun.sleep(30)
        active -= 1
        return executorResult(input.objective.includes("fail") ? "failed" : "passed")
      }
      const actions = createControlSessionActions({ executor, proofsDir: proofDir, summary })
      const result = await runSessionsBatch(controlFrom(actions), {
        adapter: "codex",
        concurrency: 2,
        lane: "cloud-gcp",
        tasks: parseSessionsBatchTasks([
          { id: "first", objective: "first task" },
          { id: "second", objective: "second task should fail" },
          { id: "third", objective: "third task" },
        ]),
        verify: ["bun", "--version"],
        worktreePath: worktree,
        pollIntervalMs: 10,
      })

      expect(maxActive).toBeLessThanOrEqual(2)
      expect(result.schema).toBe("openagents.pylon.sessions_batch_result.v0.1")
      expect(result.ok).toBe(false)
      expect(result.taskCount).toBe(3)
      expect(result.concurrency).toBe(2)
      expect(result.results.map((entry) => entry.id)).toEqual(["first", "second", "third"])
      expect(result.results.map((entry) => entry.ok)).toEqual([true, false, true])
      const list = await actions.list()
      for (const entry of result.results) {
        expect(list.find((session) => session.sessionRef === entry.result.sessionRef)?.lane).toBe("cloud-gcp")
      }
      expect(result.failures).toEqual([
        expect.objectContaining({
          id: "second",
          outcome: "failed",
          errorClass: "verification_failed",
        }),
      ])
    })
  })
})
