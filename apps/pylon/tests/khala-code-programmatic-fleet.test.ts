import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"
import { CODEX_AGENT_CAPABILITY_REF } from "../src/codex-agent"
import { recordPylonAccountUsageObservation } from "../src/account-usage"
import { PYLON_CODEX_DIRECT_LOCAL_USAGE_INGEST_PATH } from "../src/codex-direct-local-usage-reporter"
import { assertPublicProjectionSafe, ensurePylonLocalState } from "../src/state"

const INDEX = join(import.meta.dir, "..", "src", "index.ts")
const CWD = join(import.meta.dir, "..")

const servers: ReturnType<typeof Bun.serve>[] = []

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true)
})

async function withHome<T>(fn: (fixture: {
  codexHome: string
  home: string
  noSiblingRoot: string
  worktree: string
}) => Promise<T>): Promise<T> {
  const home = await mkdtemp(join(tmpdir(), "pylon-khala-code-programmatic-"))
  try {
    const codexHome = join(home, "codex-default")
    const noSiblingRoot = join(home, "no-siblings")
    const worktree = join(home, "worktree")
    await mkdir(codexHome, { recursive: true })
    await mkdir(noSiblingRoot, { recursive: true })
    await mkdir(worktree, { recursive: true })
    await mkdir(join(home, "auth"), { recursive: true })
    await writeFile(join(codexHome, "auth.json"), "{}\n")
    await writeFile(join(home, "auth", "openagents-agent-token"), "oa_agent_programmatic_fixture\n")
    return await fn({ codexHome, home, noSiblingRoot, worktree })
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function seedLocalCodexRuntime(home: string) {
  const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
  const state = await ensurePylonLocalState(summary)
  await writeFile(
    state.paths.runtimeState,
    `${JSON.stringify({
      blockerRefs: [],
      capabilityRefs: [CODEX_AGENT_CAPABILITY_REF],
      displayName: "Khala Code Programmatic Fixture",
      lifecycle: "assignment-ready",
      resourceMode: "background_20",
      updatedAt: "2026-07-01T13:00:00.000Z",
    })}\n`,
  )
  return summary
}

async function runPylonCli(args: string[], env: Record<string, string | undefined>) {
  const proc = Bun.spawn(["bun", INDEX, ...args], {
    cwd: CWD,
    env,
    stderr: "pipe",
    stdout: "pipe",
  })
  const timeout = setTimeout(() => proc.kill(), 10_000)
  try {
    const [exitCode, stdout, stderr] = await Promise.all([
      proc.exited,
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
    ])
    return { exitCode, stdout, stderr }
  } finally {
    clearTimeout(timeout)
  }
}

function devCheck() {
  return {
    schema: "openagents.pylon.dev_check.v0.3",
    observedAt: "2026-07-01T13:00:01.000Z",
    action: "check",
    state: "passed",
    changeSummary: {
      repo: {
        state: "ready",
        rootRef: "root.programmatic.fixture",
        branch: "branch.main",
        commit: "commit.fixture",
      },
      dirty: {
        state: "clean",
        changedCount: 0,
        stagedCount: 0,
        unstagedCount: 0,
        untrackedCount: 0,
      },
      changedFileRefs: [],
      areaRefs: [],
      blockerRefs: [],
    },
    checkPlan: {
      state: "ready",
      commandRefs: ["command.verify"],
      blockerRefs: [],
    },
    commandResults: [
      {
        commandRef: "command.verify",
        reasonRef: "check.verify",
        cwdRef: "command.cwd.programmatic_fixture",
        argvRef: "command.argv.verify",
        exitCode: 0,
        status: "passed",
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

function makeControlServer(token: string) {
  type Session = {
    adapter: "codex" | "claude_agent"
    completeAt: number
    lane: string
    objective: string
    sessionRef: string
    state: "running" | "completed"
    verify: string[]
    worktreePath: string | null
  }

  const sessions: Session[] = []
  const commands: Record<string, unknown>[] = []
  let maxActive = 0

  const refresh = () => {
    const now = Date.now()
    for (const session of sessions) {
      if (session.state === "running" && now >= session.completeAt) {
        session.state = "completed"
      }
    }
    maxActive = Math.max(
      maxActive,
      sessions.filter(session => session.state === "running").length,
    )
  }

  const projection = (session: Session) => ({
    sessionRef: session.sessionRef,
    parentSessionRef: null,
    adapter: session.adapter,
    lane: session.lane,
    account: null,
    accountRefHash: null,
    objectiveRef: `objective.${session.sessionRef}`,
    workspaceRef: `workspace.${session.sessionRef}`,
    workspaceCleanupRef: null,
    workspaceCleanupReceiptRef: null,
    workspaceRetentionReasonRef: null,
    objectiveDigestRef: `objective.${session.sessionRef}`,
    verifyRef: `verify.${session.sessionRef}`,
    state: session.state,
    artifactRef: `artifact.${session.sessionRef}`,
    resultRef: `result.${session.sessionRef}`,
    errorClass: null,
    errorDigestRef: null,
    createdAt: "2026-07-01T13:00:00.000Z",
    startedAt: "2026-07-01T13:00:00.000Z",
    completedAt: session.state === "completed" ? "2026-07-01T13:00:01.000Z" : null,
    updatedAt: "2026-07-01T13:00:01.000Z",
    eventCount: 2,
    latestActivity: `completed ${session.sessionRef}`,
    cloudRunner: null,
    resourceUsageReceiptRef: null,
  })

  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      if (request.headers.get("authorization") !== `Bearer ${token}`) {
        return Response.json({ ok: false, error: "unauthorized" }, { status: 401 })
      }
      if (url.pathname !== "/command") {
        return Response.json({ ok: false, error: "not_found" }, { status: 404 })
      }

      const command = JSON.parse(await request.text()) as Record<string, unknown>
      commands.push(command)
      refresh()

      switch (command.type) {
        case "session.spawn": {
          const session: Session = {
            adapter: command.adapter as "codex" | "claude_agent",
            completeAt: Date.now() + 80,
            lane: typeof command.lane === "string" ? command.lane : "auto",
            objective: String(command.objective),
            sessionRef: `session.pylon.control.programmatic.${sessions.length + 1}`,
            state: "running",
            verify: Array.isArray(command.verify) ? command.verify.map(String) : [],
            worktreePath: typeof command.worktreePath === "string" ? command.worktreePath : null,
          }
          sessions.push(session)
          refresh()
          return Response.json({
            ok: true,
            result: { sessionRef: session.sessionRef, state: session.state },
          })
        }
        case "session.list":
          refresh()
          return Response.json({ ok: true, result: sessions.map(projection) })
        case "session.events": {
          const session = sessions.find(item => item.sessionRef === command.sessionRef)
          if (session === undefined) {
            return Response.json({ ok: false, error: "session_not_found" }, { status: 404 })
          }
          return Response.json({
            ok: true,
            result: {
              sessionRef: session.sessionRef,
              eventsPath: `/events/${session.sessionRef}`,
              state: session.state,
              recentEvents: [
                {
                  schema: "openagents.pylon.control_session_event.v0.1",
                  sessionRef: session.sessionRef,
                  observedAt: "2026-07-01T13:00:01.000Z",
                  eventIndex: 1,
                  phase: "completed",
                  state: session.state,
                  adapter: session.adapter,
                  account: null,
                  workspaceRef: `workspace.${session.sessionRef}`,
                  messageText: `completed ${session.objective}`,
                },
              ],
            },
          })
        }
        case "session.artifact": {
          const session = sessions.find(item => item.sessionRef === command.sessionRef)
          if (session === undefined) {
            return Response.json({ ok: false, error: "session_not_found" }, { status: 404 })
          }
          return Response.json({
            ok: true,
            result: {
              sessionRef: session.sessionRef,
              kind: "proof",
              artifact: { devCheck: devCheck() },
            },
          })
        }
        case "approvals.list":
          return Response.json({ ok: true, result: { approvals: [] } })
        default:
          return Response.json({ ok: false, error: `unexpected ${String(command.type)}` }, { status: 400 })
      }
    },
  })
  servers.push(server)

  return {
    baseUrl: `http://127.0.0.1:${server.port}`,
    commands,
    maxActive: () => maxActive,
    sessions: () => sessions,
  }
}

function makeStatsIngestServer() {
  const ingests: Array<{ body: Record<string, unknown>; headers: Headers; path: string }> = []
  let total = 10_000
  const server = Bun.serve({
    port: 0,
    async fetch(request) {
      const url = new URL(request.url)
      if (url.pathname !== PYLON_CODEX_DIRECT_LOCAL_USAGE_INGEST_PATH) {
        return Response.json({ error: "not_found" }, { status: 404 })
      }
      const body = JSON.parse(await request.text()) as Record<string, unknown>
      ingests.push({ body, headers: request.headers, path: url.pathname })
      const usage = body.usage as { inputTokens?: number; outputTokens?: number } | undefined
      const delta = Math.max(0, usage?.inputTokens ?? 0) + Math.max(0, usage?.outputTokens ?? 0)
      total += delta
      return Response.json({
        schemaVersion: "openagents.pylon.codex_direct_local_usage_ingest_result.v1",
        inserted: true,
        tokensServed: total,
        tokensServedDelta: delta,
      })
    },
  })
  servers.push(server)
  return { baseUrl: `http://127.0.0.1:${server.port}`, ingests }
}

describe("Khala Code programmatic CLI fleet and stats coverage", () => {
  test("plans Khala Code fleet capacity, steers multiple Codex sessions through the CLI, and reports direct-local tokens to stats", async () => {
    await withHome(async ({ codexHome, home, noSiblingRoot, worktree }) => {
      const summary = await seedLocalCodexRuntime(home)
      const baseEnv = {
        ...Bun.env,
        CLAUDE_CONFIG_DIR: join(home, "claude-default"),
        CODEX_HOME: codexHome,
        PYLON_ACCOUNT_HOME_ROOT: noSiblingRoot,
        PYLON_OPENAGENTS_BASE_URL: "https://openagents.test",
        PYLON_HOME: home,
      } satisfies Record<string, string | undefined>

      const plan = await runPylonCli(
        [
          "khala",
          "spawn",
          "--fixture",
          "--count",
          "2",
          "--max-parallel",
          "2",
          "--objective",
          "programmatic Khala Code fleet smoke",
          "--json",
        ],
        baseEnv,
      )
      expect(plan.exitCode, plan.stderr).toBe(0)
      const planBody = JSON.parse(plan.stdout) as Record<string, unknown>
      assertPublicProjectionSafe(planBody)
      expect(planBody.schema).toBe("openagents.pylon.khala_spawn_plan.v0.1")
      expect(planBody.blockerRefs).toEqual([])
      expect(planBody.requestedCount).toBe(2)
      expect(planBody.readyCodexAccountCount).toBeGreaterThanOrEqual(1)
      expect(planBody.advertisedCodexAvailability).toBeGreaterThanOrEqual(2)
      expect(planBody.maxParallel).toBe(2)
      expect(planBody.slots).toHaveLength(2)

      const controlToken = "control-token-programmatic-fixture"
      const control = makeControlServer(controlToken)
      const tasksPath = join(home, "khala-code-programmatic-tasks.json")
      await writeFile(
        tasksPath,
        `${JSON.stringify([
          { id: "alpha", objective: "run Khala Code session alpha" },
          { id: "beta", objective: "run Khala Code session beta" },
          { id: "gamma", objective: "run Khala Code session gamma" },
        ])}\n`,
      )

      const batch = await runPylonCli(
        [
          "sessions",
          "batch",
          "--adapter",
          "codex",
          "--tasks",
          tasksPath,
          "--concurrency",
          "2",
          "--lane",
          "local",
          "--worktree",
          worktree,
          "--verify",
          "echo programmatic-ok",
        ],
        {
          ...baseEnv,
          PYLON_CONTROL_TOKEN: controlToken,
          PYLON_CONTROL_URL: control.baseUrl,
        },
      )
      expect(batch.exitCode, batch.stderr).toBe(0)
      const batchBody = JSON.parse(batch.stdout) as {
        concurrency: number
        ok: boolean
        results: Array<{ id: string; ok: boolean; result: { adapter: string; outcome: string; verify: { passed: boolean } } }>
        schema: string
        taskCount: number
      }
      expect(batchBody).toMatchObject({
        schema: "openagents.pylon.sessions_batch_result.v0.1",
        ok: true,
        taskCount: 3,
        concurrency: 2,
      })
      expect(batchBody.results.map(result => result.id)).toEqual(["alpha", "beta", "gamma"])
      expect(batchBody.results.every(result => result.ok)).toBe(true)
      expect(batchBody.results.every(result => result.result.adapter === "codex")).toBe(true)
      expect(batchBody.results.every(result => result.result.outcome === "completed")).toBe(true)
      expect(batchBody.results.every(result => result.result.verify.passed)).toBe(true)

      const spawns = control.commands.filter(command => command.type === "session.spawn")
      expect(spawns).toHaveLength(3)
      expect(control.maxActive()).toBeLessThanOrEqual(2)
      expect(control.sessions().map(session => session.worktreePath)).toEqual([worktree, worktree, worktree])
      expect(control.sessions().map(session => session.verify)).toEqual([
        ["sh", "-c", "echo programmatic-ok"],
        ["sh", "-c", "echo programmatic-ok"],
        ["sh", "-c", "echo programmatic-ok"],
      ])

      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.programmatic.alpha",
          inputTokens: 40,
          outputTokens: 10,
          totalTokens: 50,
        },
        observedAt: new Date("2026-07-01T13:00:02.000Z"),
      })
      await recordPylonAccountUsageObservation(summary, {
        provider: "codex",
        localSessionUsage: {
          provider: "codex",
          sessionRef: "session.pylon.codex.programmatic.alpha",
          inputTokens: 65,
          outputTokens: 35,
          totalTokens: 100,
        },
        observedAt: new Date("2026-07-01T13:00:03.000Z"),
      })

      const stats = makeStatsIngestServer()
      const usage = await runPylonCli(
        [
          "accounts",
          "usage",
          "--provider",
          "codex",
          "--report-local-codex-usage",
          "--json",
        ],
        {
          ...baseEnv,
          PYLON_OPENAGENTS_BASE_URL: stats.baseUrl,
        },
      )
      expect(usage.exitCode, usage.stderr).toBe(0)
      const usageBody = JSON.parse(usage.stdout) as {
        refresh: {
          directLocalCodexReport: {
            blockerRefs: string[]
            insertedCount: number
            performed: boolean
            requested: boolean
            sentCount: number
          }
        }
      }
      expect(usageBody.refresh.directLocalCodexReport).toMatchObject({
        requested: true,
        performed: true,
        sentCount: 2,
        insertedCount: 2,
        blockerRefs: [],
      })
      expect(stats.ingests).toHaveLength(2)
      expect(stats.ingests.every(entry => entry.path === PYLON_CODEX_DIRECT_LOCAL_USAGE_INGEST_PATH)).toBe(true)
      expect(stats.ingests.every(entry => entry.headers.get("authorization") === "Bearer oa_agent_programmatic_fixture")).toBe(true)
      expect(stats.ingests.map(entry => entry.body.usage)).toEqual([
        {
          inputTokens: 40,
          outputTokens: 10,
          totalTokens: 50,
          usageTruth: "exact",
        },
        {
          inputTokens: 25,
          outputTokens: 25,
          totalTokens: 50,
          usageTruth: "exact",
        },
      ])
      expect(stats.ingests.every(entry => entry.body.schemaVersion === "openagents.pylon.codex_direct_local_usage.v1")).toBe(true)
      expect(stats.ingests.every(entry => typeof entry.body.idempotencyKey === "string")).toBe(true)
      expect(JSON.stringify(stats.ingests.map(entry => entry.body))).not.toContain(home)
      expect(JSON.stringify(stats.ingests.map(entry => entry.body))).not.toContain("oa_agent")
      expect(JSON.stringify(stats.ingests.map(entry => entry.body))).not.toContain("auth.json")
    })
  })
})
