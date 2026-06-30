import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
import {
  KhalaFleetDelegationAdmittedParametersEnv,
  KhalaFleetDelegationParameterSetSchemaVersion,
} from "@openagentsinc/khala-tools"
import {
  createKhalaCodexFleetTools,
  ensureLocalPylon,
  inspectCodexFleet,
  spawnCodexInstances,
  type KhalaCodexFleetCommandInput,
  type KhalaCodexFleetCommandResult,
} from "../src/bun/khala-codex-fleet-tools"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

async function tempPylonFixture(): Promise<{
  readonly appPath: string
  readonly env: Record<string, string>
  readonly home: string
}> {
  const root = await mkdtemp(join(tmpdir(), "khala-code-fleet-"))
  tempDirs.push(root)
  const appPath = join(root, "apps", "pylon")
  const home = join(root, "pylon-home")
  await mkdir(appPath, { recursive: true })
  await mkdir(home, { recursive: true })
  await writeFile(join(appPath, "package.json"), JSON.stringify({ name: "@openagentsinc/pylon" }))
  return {
    appPath,
    env: {
      OPENAGENTS_BUN_PATH: process.execPath,
      OPENAGENTS_PYLON_APP_PATH: appPath,
      PYLON_HOME: home,
    },
    home,
  }
}

function ok(stdout: unknown): KhalaCodexFleetCommandResult {
  return {
    exitCode: 0,
    signal: null,
    stderr: "",
    stdout: typeof stdout === "string" ? stdout : JSON.stringify(stdout),
    timedOut: false,
  }
}

function failed(stderr: string): KhalaCodexFleetCommandResult {
  return {
    exitCode: 1,
    signal: null,
    stderr,
    stdout: "",
    timedOut: false,
  }
}

function pylonArgs(input: KhalaCodexFleetCommandInput): readonly string[] {
  const index = input.cmd.indexOf("src/index.ts")
  return index === -1 ? input.cmd : input.cmd.slice(index + 1)
}

const MATRIX_ACCOUNT_KEY = "4db4cc18ebc55f39fb4da894"
const MATRIX_ACCOUNT_REF_HASH = `account.pylon.codex.${MATRIX_ACCOUNT_KEY}`

function matrixBatchSpawnSuccess(
  assignmentRef: string,
  accountRef = "codex-2",
): Record<string, unknown> {
  return {
    aggregate: {
      acceptedCount: 1,
      assignmentRefs: [assignmentRef],
      durableRequestIds: [`durable.${assignmentRef.split(".").at(-1) ?? "matrix"}`],
      ownerOnlyRawEventCount: 1,
      ownerOnlyTraceCount: 1,
      totalTokenRows: 1,
      totalVerifiedTokens: 100,
    },
    blockerRefs: [],
    counter: { expectedMinimumDelta: 0, state: "not_checked" },
    ok: true,
    plan: {
      requestedCount: 1,
      slots: [{ account: { accountRef }, slotIndex: 0 }],
      targetPylonRef: "pylon.local.test",
    },
    results: [{
      assignmentRef,
      blockerRefs: [],
      closeoutStatus: "accepted",
      ok: true,
      proof: { rawEventCount: 1, tokenRows: 1, totalTokens: 100, traceCount: 1 },
      runAccepted: true,
      slotIndex: 0,
      state: "completed",
    }],
    schema: "openagents.pylon.khala_spawn_run.v0.1",
  }
}

function matrixBatchSpawnBlocker(
  blockerRef: string,
  message: string,
): Record<string, unknown> {
  return {
    aggregate: {
      acceptedCount: 0,
      assignmentRefs: [],
      durableRequestIds: [],
      ownerOnlyRawEventCount: 0,
      ownerOnlyTraceCount: 0,
      totalTokenRows: 0,
      totalVerifiedTokens: 0,
    },
    blockerRefs: [blockerRef],
    counter: { expectedMinimumDelta: 0, state: "not_checked" },
    ok: false,
    plan: {
      requestedCount: 1,
      slots: [{ account: { accountRef: "codex-2" }, slotIndex: 0 }],
      targetPylonRef: "pylon.local.test",
    },
    results: [{
      assignmentRef: null,
      blockerRefs: [blockerRef],
      closeoutStatus: null,
      failure: { message },
      ok: false,
      proof: null,
      runAccepted: false,
      slotIndex: 0,
      state: "blocked",
    }],
    schema: "openagents.pylon.khala_spawn_run.v0.1",
  }
}

describe("Khala Code Codex fleet tools", () => {
  test("ensureLocalPylon starts a missing local Pylon and re-probes it", async () => {
    const fixture = await tempPylonFixture()
    const calls: KhalaCodexFleetCommandInput[] = []
    let goOnlineCalls = 0
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      calls.push(input)
      const args = pylonArgs(input)
      if (args.join(" ") === "provider go-online --json") {
        goOnlineCalls += 1
        return goOnlineCalls === 1
          ? failed("offline")
          : ok({
            ok: true,
            ownCapacityDispatch: {
              availableCodexAssignments: 3,
              maxCodexAssignments: 4,
            },
            pylonRef: "pylon.local.test",
          })
      }
      if (input.detached && args.length === 0) return ok("")
      return failed(`unexpected command: ${args.join(" ")}`)
    }

    const result = await ensureLocalPylon({
      start: true,
      waitMs: 1_000,
    }, {
      env: fixture.env,
      runner,
      sleep: async () => {},
    })

    expect(result).toMatchObject({
      ok: true,
      availableCodexAssignments: 3,
      maxCodexAssignments: 4,
      pylonRef: "pylon.local.test",
      started: true,
      status: "started",
    })
    expect(calls.some(call => call.detached === true && pylonArgs(call).length === 0)).toBe(true)
    expect(goOnlineCalls).toBe(2)
    expect(calls.some(call => call.env?.PYLON_HOME !== undefined)).toBe(true)
  })

  test("inspectCodexFleet reports capacity from provider go-online", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            maxCodexAssignments: 5,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex",
              homeState: "present",
              provider: "codex",
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await inspectCodexFleet({
      includeProcesses: false,
    }, {
      env: fixture.env,
      runner,
    })

    expect(result.availableCodexAssignments).toBe(2)
    expect(result.maxCodexAssignments).toBe(5)
    expect(result.accounts).toHaveLength(1)
  })

  test("codex_fleet_status counts only real codex exec agent turns", async () => {
    const fixture = await tempPylonFixture()
    await writeFile(join(fixture.home, "config.json"), JSON.stringify({ pylonRef: "pylon.local.test" }))
    const markerRoot = join(fixture.home, "active-assignment-runs")
    await mkdir(markerRoot, { recursive: true })
    await writeFile(join(markerRoot, "assignment.public.one.json"), JSON.stringify({
      assignmentRef: "assignment.public.one",
      issueRef: "issue:7737",
      updatedAt: "2026-06-30T12:00:00.000Z",
    }))
    await writeFile(join(markerRoot, "assignment.public.stale.json"), JSON.stringify({
      assignmentRef: "assignment.public.stale",
      issueRef: "issue:7737",
      updatedAt: "2026-06-30T11:59:00.000Z",
    }))

    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      if (input.cmd[0] === "ps") {
        return ok([
          "  PID  PPID     ELAPSED COMMAND",
          "  200   199    00:02:03 /opt/homebrew/bin/codex exec --json --sandbox workspace-write",
          "  201     1    00:44:10 /Applications/Codex.app/Contents/MacOS/Codex",
          "  202     1    00:10:00 /Users/example/bin/durable-runner-pool.sh codex exec --json",
          "  203   202    00:00:01 rg codex exec",
          "  204     1    02:00:00 bun apps/pylon/src/index.ts provider go-online",
        ].join("\n"))
      }
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 1,
            maxCodexAssignments: 2,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex-2",
              homeState: "present",
              provider: "codex",
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex-2",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await inspectCodexFleet({}, {
      env: fixture.env,
      runner,
    })

    expect(result.activeAssignments).toHaveLength(2)
    expect(result.processes).toEqual([{
      elapsed: "00:02:03",
      kind: "codex_exec",
      parentPid: "199",
      pid: "200",
    }])

    const tool = createKhalaCodexFleetTools({ env: fixture.env, runner })
      .find(item => item.definition.name === "codex_fleet_status")
    const toolResult = await Effect.runPromise(tool!.execute!({}, {} as never))

    expect(toolResult.modelOutput.text).toContain("Active assignment markers: 2")
    expect(toolResult.modelOutput.text).toContain("Active Codex exec processes: 1")
    expect(toolResult.modelOutput.text)
      .toContain("Assignment/process reconciliation: 2 marker(s), 1 codex exec process(es)")
    expect(toolResult.modelOutput.text).toContain("- 200 parent=199 elapsed=00:02:03 codex_exec")
    expect(toolResult.modelOutput.text).not.toContain("Local Pylon/Codex processes")
    expect(toolResult.modelOutput.text).not.toContain("Codex.app")
    expect(toolResult.modelOutput.text).not.toContain("durable-runner-pool")
  })

  test("spawnCodexInstances defaults unpinned smoke requests to the Pylon fixture", async () => {
    const fixture = await tempPylonFixture()
    const calls: KhalaCodexFleetCommandInput[] = []
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      calls.push(input)
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({ ok: true, pylonRef: "pylon.local.test" })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex",
              homeState: "present",
              provider: "codex",
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex",
              provider: "codex",
              quota: { state: "available" },
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        expect(args).toContain("--workflow")
        expect(args).toContain("codex_agent_task")
        expect(args).toContain("--fixture")
        expect(args).toContain("--account-ref")
        expect(args).toContain("codex")
        expect(args).toContain("--pylon-ref")
        expect(args).toContain("pylon.local.test")
        expect(args).toContain("--base-url")
        expect(args).toContain("https://openagents.com")
        expect(args).toContain("--json")
        expect(input.env?.PYLON_OPENAGENTS_BASE_URL).toBeUndefined()
        return ok({
          assignmentRef: "assignment.public.codex_agent_task.test",
          autoRun: {
            attempted: false,
            reason: "disabled_by_no_run",
          },
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await spawnCodexInstances({
      count: 1,
      noRun: true,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })

    expect(result).toMatchObject({
      acceptedCount: 1,
      pylonRef: "pylon.local.test",
      requestedCount: 1,
    })
    expect(result.results[0]).toMatchObject({
      accountRef: "codex",
      assignmentRef: "assignment.public.codex_agent_task.test",
      status: "accepted",
    })
    expect(result.results[0]?.summary).toContain("assignment: assignment.public.codex_agent_task.test")
    expect(result.results[0]?.summary).toContain("auto-run: not attempted (disabled_by_no_run)")
    expect(result.results[0]?.summary).toContain("assignment run: no result returned")
    expect(result.results[0]?.summary).toContain("no local output path was returned")
    expect(calls.some(call => pylonArgs(call)[0] === "khala" && pylonArgs(call)[1] === "request")).toBe(true)
  })

  test("spawnCodexInstances heartbeats and omits the display-only default account ref", async () => {
    const fixture = await tempPylonFixture()
    const calls: KhalaCodexFleetCommandInput[] = []
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      calls.push(input)
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({ ok: true, pylonRef: "pylon.local.test" })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: null,
              homeState: "present",
              provider: "codex",
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: null,
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.fresh.1",
          pylonRef: "pylon.local.fresh",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        expect(args).not.toContain("--account-ref")
        expect(args).toContain("--pylon-ref")
        expect(args).toContain("pylon.local.fresh")
        expect(args).toContain("--execute")
        expect(args).toContain("--json")
        expect(input.maxOutputBytes).toBeGreaterThanOrEqual(5_000_000)
        return ok({
          aggregate: {
            acceptedCount: 1,
            assignmentRefs: ["assignment.public.codex_agent_task.default"],
            durableRequestIds: ["durable.default"],
            ownerOnlyRawEventCount: 1,
            ownerOnlyTraceCount: 1,
            totalTokenRows: 1,
            totalVerifiedTokens: 16,
          },
          blockerRefs: [],
          counter: {
            delta: 16,
            expectedMinimumDelta: 16,
            state: "increment_observed",
          },
          ok: true,
          plan: {
            requestedCount: 1,
            slots: [{
              account: { accountRef: null },
              slotIndex: 0,
            }],
            targetPylonRef: "pylon.local.fresh",
          },
          results: [{
            assignmentRef: "assignment.public.codex_agent_task.default",
            blockerRefs: [],
            closeoutStatus: "accepted",
            durableRequestId: "durable.default",
            lifecycleEvents: [
              {
                assignmentEvent: "assignment_run.runtime_started",
                message: "assignment_run.runtime_started",
                observedAt: "2026-06-30T00:00:00.000Z",
                slotIndex: 0,
                state: "running",
              },
              {
                assignmentEvent: "assignment_run.completed",
                message: "assignment_run.completed",
                observedAt: "2026-06-30T00:00:01.000Z",
                slotIndex: 0,
                state: "accepted",
                status: "accepted",
              },
            ],
            ok: true,
            proof: {
              rawEventCount: 1,
              tokenRows: 1,
              totalTokens: 16,
              traceCount: 1,
            },
            runAccepted: true,
            slotIndex: 0,
            state: "accepted",
          }],
          schema: "openagents.pylon.khala_spawn_run.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await spawnCodexInstances({
      count: 1,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })

    expect(result).toMatchObject({
      acceptedCount: 1,
      pylonRef: "pylon.local.fresh",
      requestedCount: 1,
    })
    expect(result.results[0]?.summary).toContain("assignment run: completed")
    expect(result.results[0]?.summary).toContain("closeout: accepted")
    expect(result.results[0]?.summary).toContain("blocker refs: none")
    expect(result.results[0]?.summary).toContain("proof: 16 verified tokens across 1 row(s)")
    expect(result.results[0]?.summary).toContain("lifecycle:")
    expect(result.results[0]?.summary).toContain("assignment_run.completed")
    const heartbeatIndex = calls.findIndex(call => pylonArgs(call)[0] === "presence")
    const requestIndex = calls.findIndex(call => pylonArgs(call)[0] === "khala" && pylonArgs(call)[1] === "spawn")
    expect(heartbeatIndex).toBeGreaterThanOrEqual(0)
    expect(requestIndex).toBeGreaterThan(heartbeatIndex)
  })

  test("spawnCodexInstances prefers named ready accounts before the display-only default account", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            maxCodexAssignments: 3,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "(default)",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
            {
              accountRef: "codex-2",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: "(default)",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        expect(args).toContain("--account-ref")
        expect(args).toContain("codex-2")
        return ok({
          assignmentRef: "assignment.public.codex_agent_task.named",
          autoRun: {
            attempted: false,
            reason: "disabled_by_no_run",
          },
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await spawnCodexInstances({
      count: 1,
      noRun: true,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })

    expect(result.results[0]).toMatchObject({
      accountRef: "codex-2",
      assignmentRef: "assignment.public.codex_agent_task.named",
      status: "accepted",
    })
  })

  test("spawnCodexInstances ignores a requested display default when a named account is ready", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            maxCodexAssignments: 3,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "(default)",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
            {
              accountRef: "codex-2",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: "(default)",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        expect(args).toContain("--account-ref")
        expect(args).toContain("codex-2")
        expect(args).not.toContain("(default)")
        return ok({
          assignmentRef: "assignment.public.codex_agent_task.default_override",
          autoRun: {
            attempted: false,
            reason: "disabled_by_no_run",
          },
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await spawnCodexInstances({
      accountRef: "(default)",
      count: 1,
      noRun: true,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })

    expect(result.results[0]).toMatchObject({
      accountRef: "codex-2",
      assignmentRef: "assignment.public.codex_agent_task.default_override",
      status: "accepted",
    })
  })

  test("spawnCodexInstances applies admitted delegation parameters and clears back to defaults", async () => {
    const fixture = await tempPylonFixture()
    const requests: Array<{
      readonly accountArg: string | null
      readonly prompt: string
      readonly verify: string | null
    }> = []
    const providerEnvValues: string[] = []
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        providerEnvValues.push(input.env?.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY ?? "")
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            maxCodexAssignments: 3,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "(default)",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
            {
              accountRef: "codex-2",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: "(default)",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        const accountIndex = args.indexOf("--account-ref")
        requests.push({
          accountArg: accountIndex === -1 ? null : args[accountIndex + 1] ?? null,
          prompt: args[args.indexOf("--prompt") + 1] ?? "",
          verify: args.includes("--verify") ? args[args.indexOf("--verify") + 1] ?? null : null,
        })
        return ok({
          assignmentRef: `assignment.public.codex_agent_task.gd4.${requests.length}`,
          autoRun: {
            attempted: false,
            reason: "disabled_by_no_run",
          },
        })
      }
      return failed(`unexpected command: ${joined}`)
    }
    const admittedEnv = {
      ...fixture.env,
      [KhalaFleetDelegationAdmittedParametersEnv]: JSON.stringify({
        accountRanking: { heuristic: "default_ready_highest_slots" },
        actionSubmissionRef: "action_submission.khala_fleet_delegation.desktop",
        advertiseCapacity: { perAccountConcurrency: 7 },
        candidateRef: "candidate.khala_fleet_delegation.desktop",
        objectiveTemplate: "GD4 tuned: {objective} :: verify={verify}",
        parameterSetRef: "parameter_set.khala_fleet_delegation.desktop.v1",
        schemaVersion: KhalaFleetDelegationParameterSetSchemaVersion,
        source: "admitted_candidate",
        verifyCriteria: { defaultVerify: "bun test packages/khala-tools" },
      }),
    }

    const tuned = await spawnCodexInstances({
      accountRef: "(default)",
      commit: "0123456789abcdef0123456789abcdef01234567",
      count: 1,
      noRun: true,
      prompt: "Wire issue 7736",
      repo: "OpenAgentsInc/openagents",
    }, {
      env: admittedEnv,
      runner,
    })
    const reverted = await spawnCodexInstances({
      accountRef: "(default)",
      commit: "0123456789abcdef0123456789abcdef01234567",
      count: 1,
      noRun: true,
      prompt: "Wire issue 7736",
      repo: "OpenAgentsInc/openagents",
      verify: "bun test",
    }, {
      env: fixture.env,
      runner,
    })

    expect(tuned.results[0]).toMatchObject({
      accountRef: "(default)",
      assignmentRef: "assignment.public.codex_agent_task.gd4.1",
      status: "accepted",
    })
    expect(reverted.results[0]).toMatchObject({
      accountRef: "codex-2",
      assignmentRef: "assignment.public.codex_agent_task.gd4.2",
      status: "accepted",
    })
    expect(providerEnvValues).toContain("7")
    expect(requests[0]).toEqual({
      accountArg: null,
      prompt: "GD4 tuned: Wire issue 7736 :: verify=bun test packages/khala-tools",
      verify: "bun test packages/khala-tools",
    })
    expect(requests[1]).toEqual({
      accountArg: "codex-2",
      prompt: "Wire issue 7736",
      verify: "bun test",
    })
  })

  test("spawnCodexInstances prefers named accounts with advertised free slots", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            codexAccounts: [
              {
                accountKey: "651c03fed68925d7acb2c02f",
                available: 0,
                busy: 1,
                queued: 0,
                ready: 1,
              },
              {
                accountKey: "4db4cc18ebc55f39fb4da894",
                available: 1,
                busy: 0,
                queued: 0,
                ready: 1,
              },
            ],
            maxCodexAssignments: 3,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex-2",
              accountRefHash: "account.pylon.codex.651c03fed68925d7acb2c02f",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
            {
              accountRef: "status",
              accountRefHash: "account.pylon.codex.4db4cc18ebc55f39fb4da894",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        expect(args).toContain("--account-ref")
        expect(args).toContain("status")
        expect(args).not.toContain("codex-2")
        return ok({
          assignmentRef: "assignment.public.codex_agent_task.available_account",
          autoRun: {
            attempted: false,
            reason: "disabled_by_no_run",
          },
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await spawnCodexInstances({
      count: 1,
      noRun: true,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })

    expect(result.results[0]).toMatchObject({
      accountRef: "status",
      assignmentRef: "assignment.public.codex_agent_task.available_account",
      status: "accepted",
    })
  })

  test("spawnCodexInstances launches planned advertised slots concurrently", async () => {
    const fixture = await tempPylonFixture()
    let inFlightRequests = 0
    let maxInFlightRequests = 0
    let requestCount = 0
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 2,
            codexAccounts: [
              {
                accountKey: "4db4cc18ebc55f39fb4da894",
                available: 2,
                busy: 0,
                queued: 0,
                ready: 2,
              },
            ],
            maxCodexAssignments: 2,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "status",
              accountRefHash: "account.pylon.codex.4db4cc18ebc55f39fb4da894",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        expect(args).toContain("--account-ref")
        expect(args).toContain("status")
        requestCount += 1
        const slot = requestCount
        inFlightRequests += 1
        maxInFlightRequests = Math.max(maxInFlightRequests, inFlightRequests)
        await new Promise(resolve => setTimeout(resolve, 25))
        inFlightRequests -= 1
        return ok({
          assignmentRef: `assignment.public.codex_agent_task.concurrent_${slot}`,
          autoRun: {
            attempted: false,
            reason: "disabled_by_no_run",
          },
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await spawnCodexInstances({
      count: 2,
      noRun: true,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })

    expect(result).toMatchObject({
      acceptedCount: 2,
      requestedCount: 2,
    })
    expect(result.results.map(item => item.accountRef)).toEqual(["status", "status"])
    expect(maxInFlightRequests).toBe(2)
  })

  test("spawnCodexInstances refuses when requested count exceeds advertised slots", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 1,
            codexAccounts: [
              {
                accountKey: "4db4cc18ebc55f39fb4da894",
                available: 1,
                busy: 0,
                queued: 0,
                ready: 1,
              },
            ],
            maxCodexAssignments: 2,
          },
          pylonRef: "pylon.local.test",
        })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "status",
              accountRefHash: "account.pylon.codex.4db4cc18ebc55f39fb4da894",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "request") {
        throw new Error("khala request should not run when slots are oversubscribed")
      }
      return failed(`unexpected command: ${joined}`)
    }

    await expect(spawnCodexInstances({
      count: 2,
      noRun: true,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })).rejects.toThrow("Only 1/2 advertised Pylon Codex account slot")
  })

  test("spawnCodexInstances advertises capacity before dispatch when Pylon initially reports no Codex capacity", async () => {
    const fixture = await tempPylonFixture()
    const calls: KhalaCodexFleetCommandInput[] = []
    let advertised = false
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      calls.push(input)
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return advertised
          ? ok({
              ok: true,
              ownCapacityDispatch: {
                availableCodexAssignments: 4,
                codexAccounts: [
                  {
                    accountKey: "4db4cc18ebc55f39fb4da894",
                    available: 4,
                    busy: 1,
                    queued: 0,
                    ready: 5,
                  },
                ],
                maxCodexAssignments: 5,
              },
              pylonRef: "pylon.local.test",
            })
          : ok({
              ok: true,
              ownCapacityDispatch: {
                availableCodexAssignments: 0,
                maxCodexAssignments: 1,
              },
              pylonRef: "pylon.local.test",
            })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex-2",
              accountRefHash: "account.pylon.codex.4db4cc18ebc55f39fb4da894",
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        expect(input.env?.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY).toBe("5")
        expect(input.env?.OPENAGENTS_PYLON_CODEX_CONCURRENCY).toBe("5")
        advertised = true
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        return ok({
          aggregate: {
            acceptedCount: 1,
            assignmentRefs: ["assignment.public.codex_agent_task.capacity_recovered"],
            durableRequestIds: ["durable.public.capacity_recovered"],
            ownerOnlyRawEventCount: 1,
            ownerOnlyTraceCount: 1,
            totalTokenRows: 1,
            totalVerifiedTokens: 100,
          },
          counter: { expectedMinimumDelta: 0, state: "not_checked" },
          ok: true,
          plan: {
            requestedCount: 1,
            slots: [{ account: { accountRef: "codex-2" }, slotIndex: 0 }],
            targetPylonRef: "pylon.local.test",
          },
          results: [{
            assignmentRef: "assignment.public.codex_agent_task.capacity_recovered",
            blockerRefs: [],
            closeoutStatus: "accepted",
            ok: true,
            proof: { rawEventCount: 1, tokenRows: 1, totalTokens: 100, traceCount: 1 },
            runAccepted: true,
            slotIndex: 0,
            state: "completed",
          }],
          schema: "openagents.pylon.khala_spawn_run.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }

    const result = await spawnCodexInstances({
      count: 1,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })

    expect(result.acceptedCount).toBe(1)
    expect(result.results[0]?.assignmentRef).toBe("assignment.public.codex_agent_task.capacity_recovered")
    const commandOrder = calls.map(call => pylonArgs(call).join(" "))
    expect(commandOrder.indexOf("presence heartbeat --base-url https://openagents.com --json"))
      .toBeLessThan(commandOrder.findIndex(command => command.startsWith("khala spawn ")))
  })

  test("codex_spawn tool renders a recordable delegate trace for the cold 0/1 capacity smoke", async () => {
    const fixture = await tempPylonFixture()
    let advertised = false
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return advertised
          ? ok({
              ok: true,
              ownCapacityDispatch: {
                availableCodexAssignments: 4,
                codexAccounts: [
                  {
                    accountKey: MATRIX_ACCOUNT_KEY,
                    available: 4,
                    busy: 1,
                    queued: 0,
                    ready: 5,
                  },
                ],
                maxCodexAssignments: 5,
              },
              pylonRef: "pylon.local.test",
            })
          : ok({
              ok: true,
              ownCapacityDispatch: {
                availableCodexAssignments: 0,
                maxCodexAssignments: 1,
              },
              pylonRef: "pylon.local.test",
            })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex-2",
              accountRefHash: MATRIX_ACCOUNT_REF_HASH,
              homeState: "present",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        expect(input.env?.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY).toBe("5")
        advertised = true
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.part2",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        return ok(matrixBatchSpawnSuccess("assignment.public.codex_agent_task.part2_demo"))
      }
      return failed(`unexpected command: ${joined}`)
    }
    const tool = createKhalaCodexFleetTools({ env: fixture.env, runner })
      .find(item => item.definition.name === "codex_spawn")

    const result = await Effect.runPromise(tool!.execute!({
      prompt: "Test delegating a piece of work to one worker for analysis only.",
    }, {} as never))

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("Khala fleet delegate: khala.fleet.delegate (completed)")
    for (const module of [
      "ensure_pylon",
      "advertise_capacity",
      "select_account",
      "prepare_work",
      "dispatch",
      "verify_closeout",
    ]) {
      expect(result.modelOutput.text).toContain(`- ${module}:`)
    }
    expect(result.modelOutput.text).toContain("Advertised Codex capacity 4/5")
    expect(result.modelOutput.text).toContain("assignment.public.codex_agent_task.part2_demo")
    expect(result.modelOutput.text)
      .not.toContain("codex_spawn_failed: No Pylon Codex assignment capacity is available right now")
    expect((result.ui as { delegateTrace?: unknown[] }).delegateTrace).toHaveLength(6)
  })

  for (const scenario of [
    {
      blockerRef: "blocker.public.pylon_dispatch.no_available_codex_capacity",
      message: "No Pylon Codex assignment capacity is available right now (0/1 available).",
      name: "no available capacity",
    },
    {
      blockerRef: "blocker.public.pylon_dispatch.stale_heartbeat",
      message: "presence.stale_heartbeat: refresh heartbeat and retry",
      name: "stale heartbeat",
    },
    {
      blockerRef: "blocker.public.pylon_dispatch.duplicate_active_assignment",
      message: "duplicate_active_assignment: assignment already active for this account",
      name: "duplicate active assignment",
    },
  ] as const) {
    test(`spawnCodexInstances adverse dispatch matrix recovers from ${scenario.name}`, async () => {
      const fixture = await tempPylonFixture()
      const calls: KhalaCodexFleetCommandInput[] = []
      let heartbeatCalls = 0
      let spawnCalls = 0
      const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
        calls.push(input)
        const args = pylonArgs(input)
        const joined = args.join(" ")
        if (joined === "provider go-online --json") {
          return ok({
            ok: true,
            ownCapacityDispatch: {
              availableCodexAssignments: 1,
              codexAccounts: [
                {
                  accountKey: MATRIX_ACCOUNT_KEY,
                  available: 1,
                  busy: 0,
                  queued: 0,
                  ready: 5,
                },
              ],
              maxCodexAssignments: 5,
            },
            pylonRef: "pylon.local.test",
          })
        }
        if (joined === "codex accounts list --json") {
          return ok({
            accounts: [
              {
                accountRef: "codex-2",
                accountRefHash: MATRIX_ACCOUNT_REF_HASH,
                homeState: "present",
                provider: "codex",
                readiness: { state: "ready" },
              },
            ],
            schema: "openagents.pylon.accounts_list.v0.3",
          })
        }
        if (joined === "accounts status --provider codex --json") {
          return ok({
            accounts: [],
            schema: "openagents.pylon.accounts_status.v0.1",
          })
        }
        if (joined === "presence heartbeat --base-url https://openagents.com --json") {
          heartbeatCalls += 1
          expect(input.env?.OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY).toBe("5")
          return ok({
            heartbeatRef: `heartbeat.pylon.local.test.${heartbeatCalls}`,
            pylonRef: "pylon.local.test",
          })
        }
        if (args[0] === "khala" && args[1] === "spawn") {
          spawnCalls += 1
          return ok(spawnCalls === 1
            ? matrixBatchSpawnBlocker(scenario.blockerRef, scenario.message)
            : matrixBatchSpawnSuccess(`assignment.public.codex_agent_task.matrix_${spawnCalls}`))
        }
        return failed(`unexpected command: ${joined}`)
      }

      const result = await spawnCodexInstances({
        count: 1,
        prompt: "Run the public fixture.",
      }, {
        env: fixture.env,
        runner,
      })

      expect(result.acceptedCount).toBe(1)
      expect(result.results[0]?.assignmentRef).toBe("assignment.public.codex_agent_task.matrix_2")
      expect(spawnCalls).toBe(2)
      expect(heartbeatCalls).toBe(scenario.name === "duplicate active assignment" ? 1 : 2)
      const commandOrder = calls.map(call => pylonArgs(call).join(" "))
      expect(commandOrder.filter(command => command === "presence heartbeat --base-url https://openagents.com --json"))
        .toHaveLength(heartbeatCalls)
      expect(commandOrder.filter(command => command.startsWith("khala spawn "))).toHaveLength(2)
    })
  }

  for (const scenario of [
    {
      expected: "credentials",
      name: "credentials_missing",
      readiness: { state: "credentials_missing" },
    },
    {
      expected: "revoked",
      name: "revoked",
      readiness: { state: "revoked" },
    },
  ] as const) {
    test(`spawnCodexInstances adverse account matrix blocks ${scenario.name} with a typed action`, async () => {
      const fixture = await tempPylonFixture()
      const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
        const args = pylonArgs(input)
        const joined = args.join(" ")
        if (joined === "provider go-online --json") {
          return ok({
            ok: true,
            ownCapacityDispatch: {
              availableCodexAssignments: 1,
              codexAccounts: [
                {
                  accountKey: MATRIX_ACCOUNT_KEY,
                  available: 1,
                  busy: 0,
                  queued: 0,
                  ready: 1,
                },
              ],
              maxCodexAssignments: 1,
            },
            pylonRef: "pylon.local.test",
          })
        }
        if (joined === "codex accounts list --json") {
          return ok({
            accounts: [
              {
                accountRef: "codex-2",
                accountRefHash: MATRIX_ACCOUNT_REF_HASH,
                homeState: "present",
                provider: "codex",
                readiness: scenario.readiness,
              },
            ],
            schema: "openagents.pylon.accounts_list.v0.3",
          })
        }
        if (joined === "accounts status --provider codex --json") {
          return ok({
            accounts: [],
            schema: "openagents.pylon.accounts_status.v0.1",
          })
        }
        if (joined === "presence heartbeat --base-url https://openagents.com --json") {
          return ok({
            heartbeatRef: "heartbeat.pylon.local.test.1",
            pylonRef: "pylon.local.test",
          })
        }
        if (args[0] === "khala" && args[1] === "spawn") {
          throw new Error("khala spawn should not run for a non-ready account")
        }
        return failed(`unexpected command: ${joined}`)
      }

      let thrown: unknown
      try {
        await spawnCodexInstances({
          accountRef: "codex-2",
          count: 1,
          prompt: "Run the public fixture.",
        }, {
          env: fixture.env,
          runner,
        })
      } catch (error) {
        thrown = error
      }

      expect(thrown).toBeDefined()
      const message = String(thrown instanceof Error ? thrown.message : thrown)
      expect(message).toContain("Khala fleet delegate blocked at select_account")
      expect(message).toContain(scenario.expected)
      expect(message).not.toContain("No Pylon Codex assignment capacity is available right now")
    })
  }

  test("codex_spawn tool treats failed local auto-runs as failed even with an assignment ref", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({ ok: true, pylonRef: "pylon.local.test" })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex",
              homeState: "present",
              provider: "codex",
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: "codex",
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        return ok({
          aggregate: {
            acceptedCount: 0,
            assignmentRefs: ["assignment.public.codex_agent_task.failed_autorun"],
            durableRequestIds: [],
            ownerOnlyRawEventCount: 0,
            ownerOnlyTraceCount: 0,
            totalTokenRows: 0,
            totalVerifiedTokens: 0,
          },
          blockerRefs: ["blocker.assignment.timeout"],
          counter: { expectedMinimumDelta: 0, state: "not_checked" },
          ok: false,
          plan: {
            requestedCount: 1,
            slots: [{ account: { accountRef: "codex" }, slotIndex: 0 }],
            targetPylonRef: "pylon.local.test",
          },
          results: [{
            assignmentRef: "assignment.public.codex_agent_task.failed_autorun",
            blockerRefs: ["blocker.assignment.timeout"],
            closeoutStatus: "timed-out",
            lifecycleEvents: [{
              assignmentEvent: "assignment_run.completed",
              message: "assignment_run.completed",
              observedAt: "2026-06-30T00:00:00.000Z",
              slotIndex: 0,
              state: "rejected",
              status: "timed-out",
            }],
            ok: false,
            proof: null,
            runAccepted: false,
            slotIndex: 0,
            state: "failed",
          }],
          schema: "openagents.pylon.khala_spawn_run.v0.1",
        })
      }
      return failed(`unexpected command: ${joined}`)
    }
    const tool = createKhalaCodexFleetTools({ env: fixture.env, runner })
      .find(item => item.definition.name === "codex_spawn")

    const result = await Effect.runPromise(tool!.execute!({
      prompt: "Run the public fixture.",
    }, {} as never))

    expect(result.status).toBe("failed")
    expect(result.modelOutput.text).toContain("Codex spawn: accepted 0/1")
    expect(result.modelOutput.text).toContain("assignment run: failed")
    expect(result.modelOutput.text).toContain("blocker refs: blocker.assignment.timeout")
    expect(result.modelOutput.text).toContain("assignment_run.completed")
    expect(result.modelOutput.text).toContain("status=timed-out")
  })

  test("codex_spawn tool reports zero accepted timed-out dispatches as failed", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({ ok: true, pylonRef: "pylon.local.test" })
      }
      if (joined === "codex accounts list --json") {
        return ok({
          accounts: [
            {
              accountRef: null,
              homeState: "present",
              provider: "codex",
            },
          ],
          schema: "openagents.pylon.accounts_list.v0.3",
        })
      }
      if (joined === "accounts status --provider codex --json") {
        return ok({
          accounts: [
            {
              accountRef: null,
              provider: "codex",
              readiness: { state: "ready" },
            },
          ],
          schema: "openagents.pylon.accounts_status.v0.1",
        })
      }
      if (joined === "presence heartbeat --base-url https://openagents.com --json") {
        return ok({
          heartbeatRef: "heartbeat.pylon.local.test.1",
          pylonRef: "pylon.local.test",
        })
      }
      if (args[0] === "khala" && args[1] === "spawn") {
        return {
          exitCode: null,
          signal: null,
          stderr: JSON.stringify({
            assignmentRef: "assignment.public.codex_agent_task.timeout",
            assignmentEvent: "assignment_run.runtime_started",
            leaseRef: "assignment.public.codex_agent_task.timeout",
            message: "assignment lifecycle event",
            observedAt: "2026-06-30T00:00:00.000Z",
            phase: "runtime_active",
            schema: "openagents.pylon.khala_spawn_worker_event.v0.1",
            slotIndex: 0,
            state: "running",
          }),
          stdout: "",
          timedOut: true,
        }
      }
      return failed(`unexpected command: ${joined}`)
    }
    const tool = createKhalaCodexFleetTools({ env: fixture.env, runner })
      .find(item => item.definition.name === "codex_spawn")

    expect(tool?.execute).toBeDefined()
    const result = await Effect.runPromise(tool!.execute!({
      prompt: "Run the public fixture.",
    }, {} as never))

    expect(result.status).toBe("failed")
    expect(result.modelOutput.text).toContain("Codex spawn: accepted 0/1")
    expect(result.modelOutput.text).toContain("command timed out")
    expect(result.modelOutput.text).toContain("assignment_run.runtime_started")
    expect(result.modelOutput.text).toContain("phase=runtime_active")
  })
})
