import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import {
  MULTI_SESSION_SUMMARY_SCHEMA,
  parsePlanJson,
  runMultiSessionPlan,
  runOneSession,
  type ProofChildInput,
} from "../scripts/multi-session-run"
import { parseProofRunArgs } from "../scripts/dev-proof-run"
import { classifySessionError } from "../src/session-error-class"
import { assertPublicProjectionSafe } from "../src/state"
import { hashPylonAccountRef } from "../src/account-registry"
import { loadQuotaRecord, recordQuotaBlock } from "../src/account-quota-ledger"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"

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

  test("accepts noNetwork at top level and per entry", () => {
    const entry = {
      adapter: "codex",
      objective: "run a retained local coding proof",
      verify: ["bun", "--version"],
      worktreePath: ".",
    }

    expect(parsePlanJson([{ ...entry, noNetwork: true }])[0]?.noNetwork).toBe(true)
    expect(parsePlanJson({ noNetwork: true, sessions: [entry] })[0]?.noNetwork).toBe(true)
    expect(parsePlanJson({ noNetwork: true, sessions: [{ ...entry, noNetwork: false }] })[0]?.noNetwork).toBe(
      false,
    )
  })
})

describe("dev proof args parsing", () => {
  test("enables network by default and lets --no-network opt out", () => {
    const base = ["--adapter", "codex", "--objective", "prove fanout", "--", "bun", "--version"]

    expect(parseProofRunArgs(base).networkAccessEnabled).toBe(true)
    expect(
      parseProofRunArgs([
        "--adapter",
        "codex",
        "--objective",
        "prove fanout",
        "--no-network",
        "--",
        "bun",
        "--version",
      ]).networkAccessEnabled,
    ).toBe(false)
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
              noNetwork: true,
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
      expect(byObjective.get("registry-account")?.noNetwork).toBe(true)
      expect(byObjective.get("direct-account")?.accountRef).toBeNull()
      expect(byObjective.get("direct-account")?.accountHome).toBe(claudeHome)
      expect(byObjective.get("direct-account")?.noNetwork).toBeUndefined()
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

  test("a heartbeat-append failure on the session's first heartbeat produces a failed outcome instead of throwing", async () => {
    // Regression for the Promise.all cron-landmine audit: runOneSession's very
    // first appendHeartbeat call used to run before its own try block, so a
    // disk/permission failure there would reject the session's promise
    // uncaught. Under runBounded's Promise.all-based worker pool, that
    // uncaught rejection would abort the ENTIRE multi-session run and discard
    // every OTHER session's outcome, not just this one's.
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const worktree = join(root, "worktree")
      await mkdir(pylonHome, { recursive: true })
      await mkdir(worktree, { recursive: true })
      await mkdir(proofsDir, { recursive: true })

      // A heartbeat path inside a nonexistent parent directory: appendFile
      // throws ENOENT rather than silently succeeding, forcing runOneSession's
      // very first appendHeartbeat call to reject.
      const brokenHeartbeatPath = join(proofsDir, "does-not-exist", "heartbeats.jsonl")

      const entry = parsePlanJson([
        {
          id: "broken-heartbeat",
          adapter: "codex",
          worktreePath: worktree,
          objective: "should still produce a failed outcome",
          verify: ["bun", "--version"],
        },
      ])[0]!

      const outcome = await runOneSession({
        args: {
          concurrency: 1,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.heartbeat-failure-test",
          plan: [entry],
        },
        entry,
        index: 0,
        heartbeatPath: brokenHeartbeatPath,
        ambientPool: [],
        proofRunner: async () => ({ exitCode: 0, stdout: "ok", stderr: "" }),
      })

      expect(outcome.state).toBe("failed")
      expect(outcome.errorClass).not.toBeNull()
      expect(outcome.sessionIndex).toBe(0)
      // The catch/writeFailure path ran, proving control returned normally
      // through runOneSession's own error handling rather than rejecting.
      const retainedFailure = await readFile(join(proofsDir, "broken-heartbeat-failure.json"), "utf8")
      expect(JSON.parse(retainedFailure).sessionRef).toBe(outcome.sessionRef)
    })
  })
})

describe("multi-session error classification", () => {
  test("a blocked/failed dev check is verification_failed, not redaction_gate", () => {
    // The combined child output of a non-passing proof carries the success
    // field name "redactionScan" plus the dev-check failure on stderr.
    const childOutput =
      'dev check did not pass: blocked\n{"adapter":"codex","devCheckState":"blocked","redactionScan":"clean"}'
    expect(classifySessionError(childOutput).errorClass).toBe("verification_failed")
  })

  test("a genuine redaction-scan failure is still redaction_gate", () => {
    expect(classifySessionError(new Error("retained proof failed redaction scan: pattern.local_path")).errorClass).toBe(
      "redaction_gate",
    )
  })

  test("workspace and account failures keep their classes", () => {
    expect(classifySessionError(new Error("worktree_path_missing")).errorClass).toBe("workspace_materialization")
    expect(classifySessionError(new Error("account home not found")).errorClass).toBe("account_selection")
  })
})

const QUOTA_OUTPUT =
  "Codex turn failed: You've hit your usage limit. Visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at Jun 14th, 2026 9:58 PM."

describe("multi-session quota-aware routing", () => {
  test("routes around a quota-blocked account to the next pool member", async () => {
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const worktree = join(root, "worktree")
      const homeA = join(root, "codex-a")
      const homeB = join(root, "codex-b")
      for (const d of [pylonHome, worktree, homeA, homeB]) await mkdir(d, { recursive: true })

      const summary = await runMultiSessionPlan(
        {
          concurrency: 1,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.quota-route",
          plan: parsePlanJson([
            {
              id: "pool-session",
              adapter: "codex",
              worktreePath: worktree,
              accountPool: [{ codexHome: homeA }, { codexHome: homeB }],
              objective: "route around quota",
              verify: ["bun", "--version"],
            },
          ]),
        },
        {
          proofRunner: async (child) => {
            if (child.accountHome === homeA) {
              return { exitCode: 1, stdout: "", stderr: QUOTA_OUTPUT }
            }
            await writeFile(
              child.proofOutput,
              `${JSON.stringify({ schema: "test.proof", executor: { totalTokens: 50 } })}\n`,
            )
            return { exitCode: 0, stdout: "ok", stderr: "" }
          },
        },
      )

      expect(summary.completedCount).toBe(1)
      expect(summary.failedCount).toBe(0)
      expect(summary.deviations).toEqual([])
      const outcome = summary.outcomes[0]!
      expect(outcome.state).toBe("completed")
      expect(outcome.routingReason).toBe("succeeded")
      expect(outcome.attempts.map((attempt) => attempt.reason)).toEqual(["quota_block", "succeeded"])
      expect(outcome.attempts[0]?.retryAtIso).not.toBeNull()

      const recordA = await loadQuotaRecord(
        createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome }),
        hashPylonAccountRef("codex", homeA),
      )
      expect(recordA).not.toBeNull()
      expect(JSON.stringify(summary)).not.toContain(root)
      expect(JSON.stringify(summary)).not.toContain(QUOTA_OUTPUT)
      assertPublicProjectionSafe(summary)
    })
  })

  test("fails with all_accounts_exhausted when the whole pool is quota-blocked", async () => {
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const worktree = join(root, "worktree")
      const homeA = join(root, "codex-a")
      const homeB = join(root, "codex-b")
      for (const d of [pylonHome, worktree, homeA, homeB]) await mkdir(d, { recursive: true })

      const summary = await runMultiSessionPlan(
        {
          concurrency: 1,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.quota-exhausted",
          plan: parsePlanJson([
            {
              id: "pool-session",
              adapter: "codex",
              worktreePath: worktree,
              accountPool: [{ codexHome: homeA }, { codexHome: homeB }],
              objective: "exhaust the pool",
              verify: ["bun", "--version"],
            },
          ]),
        },
        { proofRunner: async () => ({ exitCode: 1, stdout: "", stderr: QUOTA_OUTPUT }) },
      )

      expect(summary.completedCount).toBe(0)
      expect(summary.failedCount).toBe(1)
      expect(summary.deviations).toContain("deviation.pylon.multi_session.some_sessions_failed")
      expect(summary.deviations).toContain("deviation.pylon.multi_session.all_accounts_exhausted")
      const outcome = summary.outcomes[0]!
      expect(outcome.state).toBe("failed")
      expect(outcome.routingReason).toBe("quota_block")
      expect(outcome.retryAtIso).not.toBeNull()
      expect(outcome.attempts.map((attempt) => attempt.reason)).toEqual(["quota_block", "quota_block"])
      expect(JSON.stringify(summary)).not.toContain(QUOTA_OUTPUT)
      assertPublicProjectionSafe(summary)
    })
  })

  test("skips an account the ledger marks unavailable and uses the next", async () => {
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const worktree = join(root, "worktree")
      const homeA = join(root, "codex-a")
      const homeB = join(root, "codex-b")
      for (const d of [pylonHome, worktree, homeA, homeB]) await mkdir(d, { recursive: true })

      const ledgerSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
      const futureIso = new Date(Date.now() + 3600_000).toISOString()
      await recordQuotaBlock(ledgerSummary, {
        accountRefHash: hashPylonAccountRef("codex", homeA),
        provider: "codex",
        retryAtIso: futureIso,
        sourceDigestRef: "digest.pylon.account_quota.testseed",
        now: new Date(),
      })

      const summary = await runMultiSessionPlan(
        {
          concurrency: 1,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.quota-skip",
          plan: parsePlanJson([
            {
              id: "pool-session",
              adapter: "codex",
              worktreePath: worktree,
              accountPool: [{ codexHome: homeA }, { codexHome: homeB }],
              objective: "skip unavailable",
              verify: ["bun", "--version"],
            },
          ]),
        },
        {
          proofRunner: async (child) => {
            if (child.accountHome === homeA) throw new Error("account A should have been skipped")
            await writeFile(child.proofOutput, `${JSON.stringify({ schema: "test.proof" })}\n`)
            return { exitCode: 0, stdout: "ok", stderr: "" }
          },
        },
      )

      const outcome = summary.outcomes[0]!
      expect(outcome.state).toBe("completed")
      expect(outcome.attempts.map((attempt) => attempt.reason)).toEqual(["skipped_unavailable", "succeeded"])
      expect(outcome.attempts[0]?.retryAtIso).toBe(futureIso)
    })
  })
})

describe("multi-session instant failover (run-level pool, no per-session accountPool)", () => {
  test("a single-account session whose primary is unavailable routes to a run-level pool account in one pass", async () => {
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const worktree = join(root, "worktree")
      const homeA = join(root, "codex-a")
      const homeB = join(root, "codex-b")
      for (const d of [pylonHome, worktree, homeA, homeB]) await mkdir(d, { recursive: true })

      // Pre-mark the primary account A unavailable in the quota ledger.
      const ledgerSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
      await recordQuotaBlock(ledgerSummary, {
        accountRefHash: hashPylonAccountRef("codex", homeA),
        provider: "codex",
        retryAtIso: new Date(Date.now() + 3600_000).toISOString(),
        sourceDigestRef: "digest.pylon.account_quota.testseed",
        now: new Date(),
      })

      const calls: ProofChildInput[] = []
      const summary = await runMultiSessionPlan(
        {
          concurrency: 1,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.runlevel-failover",
          // Run-level failover pool — the session itself declares NO accountPool.
          accountPool: [{ codexHome: homeB }],
          plan: parsePlanJson([
            {
              id: "single",
              adapter: "codex",
              codexHome: homeA,
              worktreePath: worktree,
              objective: "instant failover",
              verify: ["bun", "--version"],
            },
          ]),
        },
        {
          proofRunner: async (child) => {
            calls.push(child)
            await writeFile(child.proofOutput, `${JSON.stringify({ schema: "test.proof" })}\n`)
            return { exitCode: 0, stdout: "ok", stderr: "" }
          },
        },
      )

      const outcome = summary.outcomes[0]!
      expect(outcome.state).toBe("completed")
      expect(outcome.routingReason).toBe("succeeded")
      // Primary A skipped, then B succeeded — all within this single run.
      expect(outcome.attempts.map((attempt) => attempt.reason)).toEqual([
        "skipped_unavailable",
        "succeeded",
      ])
      expect(calls).toHaveLength(1)
      expect(calls[0]?.accountHome).toBe(homeB)
      assertPublicProjectionSafe(summary)
    })
  })

  test("ambient pool is drawn from sibling sessions' accounts too", async () => {
    await withTempRoot(async (root) => {
      const pylonHome = join(root, "pylon-home")
      const proofsDir = join(root, "proofs")
      const worktree = join(root, "worktree")
      const homeA = join(root, "codex-a")
      const homeB = join(root, "codex-b")
      for (const d of [pylonHome, worktree, homeA, homeB]) await mkdir(d, { recursive: true })

      const ledgerSummary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: pylonHome })
      await recordQuotaBlock(ledgerSummary, {
        accountRefHash: hashPylonAccountRef("codex", homeA),
        provider: "codex",
        retryAtIso: new Date(Date.now() + 3600_000).toISOString(),
        sourceDigestRef: "digest.pylon.account_quota.testseed",
        now: new Date(),
      })

      // No run-level accountPool; session 0's only listed account (A) is blocked,
      // but sibling session 1 uses account B, which becomes session 0's fallback.
      const summary = await runMultiSessionPlan(
        {
          concurrency: 1,
          proofsDir,
          pylonHome,
          runId: "run.multi-session.sibling-failover",
          plan: parsePlanJson([
            { id: "s0", adapter: "codex", codexHome: homeA, worktreePath: worktree, objective: "blocked primary", verify: ["bun", "--version"] },
            { id: "s1", adapter: "codex", codexHome: homeB, worktreePath: worktree, objective: "sibling on B", verify: ["bun", "--version"] },
          ]),
        },
        {
          proofRunner: async (child) => {
            await writeFile(child.proofOutput, `${JSON.stringify({ schema: "test.proof" })}\n`)
            return { exitCode: 0, stdout: "ok", stderr: "" }
          },
        },
      )

      expect(summary.completedCount).toBe(2)
      const s0 = summary.outcomes.find((o) => o.sessionIndex === 0)!
      expect(s0.state).toBe("completed")
      expect(s0.attempts.map((a) => a.reason)).toEqual(["skipped_unavailable", "succeeded"])
    })
  })
})
