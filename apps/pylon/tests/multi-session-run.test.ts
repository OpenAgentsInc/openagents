import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  classifyError,
  MULTI_SESSION_SUMMARY_SCHEMA,
  parsePlanJson,
  runMultiSessionPlan,
  type ProofChildInput,
} from "../scripts/multi-session-run"
import { assertPublicProjectionSafe } from "../src/state"

async function withTempRoot<T>(fn: (root: string) => Promise<T>) {
  const root = await mkdtemp(join(tmpdir(), "pylon-multi-session-"))
  try {
    return await fn(root)
  } finally {
    await rm(root, { recursive: true, force: true })
  }
}

const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))

describe("multi-session plan parsing", () => {
  test("accepts array and object plan shapes while enforcing one workspace selector", () => {
    const entry = {
      adapter: "codex",
      objective: "run a retained local coding proof",
      verify: ["bun", "--version"],
      worktreePath: ".",
    }

    expect(parsePlanJson([entry])[0]?.adapter).toBe("codex")
    expect(parsePlanJson({ sessions: [entry] })[0]?.verify).toEqual(["bun", "--version"])
    expect(() =>
      parsePlanJson([
        {
          ...entry,
          repoRef: {
            provider: "github",
            visibility: "public",
            fullName: "OpenAgentsInc/openagents",
            branch: "main",
            commitSha: "3333333333333333333333333333333333333333",
          },
        },
      ]),
    ).toThrow("must use only one workspace selector")
  })
})

describe("runMultiSessionPlan", () => {
  test("runs bounded concurrent sessions with isolated account selectors and path-safe summary", async () => {
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const codexHome = join(root, "codex-a")
      const claudeHome = join(root, "claude-direct")
      const worktreeOne = join(root, "worktree-one")
      const worktreeTwo = join(root, "worktree-two")
      await mkdir(pylonHome, { recursive: true })
      await mkdir(codexHome, { recursive: true })
      await mkdir(claudeHome, { recursive: true })
      await mkdir(worktreeOne, { recursive: true })
      await mkdir(worktreeTwo, { recursive: true })
      await writeFile(
        join(pylonHome, "config.json"),
        `${JSON.stringify(
          {
            dev: {
              accounts: [{ ref: "codex-a", provider: "codex", home: codexHome }],
            },
          },
          null,
          2,
        )}\n`,
      )

      const calls: ProofChildInput[] = []
      let active = 0
      let maxActive = 0
      const summary = await runMultiSessionPlan(
        {
          concurrency: 2,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.test",
          plan: parsePlanJson([
            {
              id: "codex-registry",
              adapter: "codex",
              accountRef: "codex-a",
              worktreePath: worktreeOne,
              objective: "registry-account",
              verify: ["bun", "--version"],
            },
            {
              id: "claude-direct",
              adapter: "claude_agent",
              accountHome: claudeHome,
              worktreePath: worktreeTwo,
              objective: "direct-account",
              verify: ["bun", "--version"],
            },
            {
              id: "codex-default",
              adapter: "codex",
              worktreePath: worktreeOne,
              objective: "default-account",
              verify: ["bun", "--version"],
            },
          ]),
        },
        {
          proofRunner: async (child) => {
            active += 1
            maxActive = Math.max(maxActive, active)
            calls.push(child)
            await writeFile(
              child.proofOutput,
              `${JSON.stringify({
                schema: "test.proof",
                objective: child.objective,
                executor: { totalTokens: 100 },
              })}\n`,
            )
            await delay(10)
            active -= 1
            return { exitCode: 0, stdout: "ok", stderr: "" }
          },
        },
      )

      expect(maxActive).toBeLessThanOrEqual(2)
      expect(summary.schema).toBe(MULTI_SESSION_SUMMARY_SCHEMA)
      expect(summary.totalSessions).toBe(3)
      expect(summary.completedCount).toBe(3)
      expect(summary.failedCount).toBe(0)
      expect(summary.deviations).toEqual([])
      expect(summary.artifactRefs).toHaveLength(3)
      expect(typeof summary.totalDurationMs).toBe("number")
      expect(typeof summary.totalTokens).toBe("number")
      expect(summary.totalTokens).toBe(300)
      for (const outcome of summary.outcomes) {
        expect(typeof outcome.durationMs).toBe("number")
      }
      assertPublicProjectionSafe(summary)
      const serialized = JSON.stringify(summary)
      expect(serialized).not.toContain(root)
      expect(serialized).not.toContain(codexHome)
      expect(serialized).not.toContain(claudeHome)

      const byObjective = new Map(calls.map(call => [call.objective, call]))
      expect(byObjective.get("registry-account")?.accountRef).toBe("codex-a")
      expect(byObjective.get("registry-account")?.accountHome).toBeNull()
      expect(byObjective.get("direct-account")?.accountRef).toBeNull()
      expect(byObjective.get("direct-account")?.accountHome).toBe(claudeHome)
      expect(byObjective.get("default-account")?.account).toBeNull()

      const retainedSummary = await readFile(join(proofsDir, "multi-session-summary.json"), "utf8")
      expect(JSON.parse(retainedSummary).schema).toBe(MULTI_SESSION_SUMMARY_SCHEMA)
      const heartbeatLog = await readFile(join(proofsDir, "heartbeats.jsonl"), "utf8")
      expect(heartbeatLog).toContain('"phase":"run_started"')
      expect(heartbeatLog).toContain('"phase":"run_completed"')
    })
  })

  test("keeps going after a child failure without retaining raw child output", async () => {
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const worktree = join(root, "worktree")
      await mkdir(pylonHome, { recursive: true })
      await mkdir(worktree, { recursive: true })

      const summary = await runMultiSessionPlan(
        {
          concurrency: 2,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.failure-test",
          plan: parsePlanJson([
            {
              id: "ok",
              adapter: "codex",
              worktreePath: worktree,
              objective: "successful child",
              verify: ["bun", "--version"],
            },
            {
              id: "bad",
              adapter: "claude_agent",
              worktreePath: worktree,
              objective: "failed child",
              verify: ["bun", "--version"],
            },
          ]),
        },
        {
          proofRunner: async (child) => {
            if (child.objective === "failed child") {
              return { exitCode: 17, stdout: "raw public stdout", stderr: "raw public stderr" }
            }
            await writeFile(child.proofOutput, '{"schema":"test.proof"}\n')
            return { exitCode: 0, stdout: "ok", stderr: "" }
          },
        },
      )

      expect(summary.completedCount).toBe(1)
      expect(summary.failedCount).toBe(1)
      expect(summary.deviations).toEqual(["deviation.pylon.multi_session.some_sessions_failed"])
      const failed = summary.outcomes.find(outcome => outcome.state === "failed")
      expect(failed?.artifactFile).toBe("bad-failure.json")
      expect(failed?.errorClass).toBe("execution_error")
      const retainedFailure = await readFile(join(proofsDir, "bad-failure.json"), "utf8")
      expect(retainedFailure).not.toContain("raw public stdout")
      expect(retainedFailure).not.toContain("raw public stderr")
      expect(JSON.stringify(summary)).not.toContain(root)
      assertPublicProjectionSafe(summary)
    })
  })
})

describe("multi-session error classification", () => {
  test("a blocked/failed dev check is verification_failed, not redaction_gate", () => {
    // The combined child output of a non-passing proof carries the success
    // field name "redactionScan" plus the dev-check failure on stderr.
    const childOutput =
      'dev check did not pass: blocked\n{"adapter":"codex","devCheckState":"blocked","redactionScan":"clean"}'
    expect(classifyError(childOutput).errorClass).toBe("verification_failed")
  })

  test("a genuine redaction-scan failure is still redaction_gate", () => {
    expect(classifyError(new Error("retained proof failed redaction scan: pattern.local_path")).errorClass).toBe(
      "redaction_gate",
    )
  })

  test("workspace and account failures keep their classes", () => {
    expect(classifyError(new Error("worktree_path_missing")).errorClass).toBe("workspace_materialization")
    expect(classifyError(new Error("account home not found")).errorClass).toBe("account_selection")
  })
})
