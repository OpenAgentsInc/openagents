import { afterEach, describe, expect, test } from "bun:test"
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"
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
      if (args[0] === "khala" && args[1] === "request") {
        expect(args).not.toContain("--account-ref")
        expect(args).toContain("--pylon-ref")
        expect(args).toContain("pylon.local.fresh")
        return ok({
          assignmentRef: "assignment.public.codex_agent_task.default",
          assignmentLifecycleEvents: [
            {
              assignmentRef: "assignment.public.codex_agent_task.default",
              event: "assignment_run.runtime_started",
              leaseRef: "assignment.public.codex_agent_task.default",
              schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
            },
            {
              assignmentRef: "assignment.public.codex_agent_task.default",
              event: "assignment_run.completed",
              leaseRef: "assignment.public.codex_agent_task.default",
              schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
              status: "accepted",
            },
          ],
          autoRun: {
            attempted: true,
            ok: true,
          },
          assignmentRun: {
            closeout: {
              assignmentRef: "assignment.public.codex_agent_task.default",
              blockerRefs: [],
              paymentMode: "no-spend",
              resultRefs: ["result.public.pylon.codex_agent_task.fixture_repair_passed"],
              settlementState: "not_applicable",
              status: "accepted",
            },
            closeoutReceipt: {
              closeoutRef: "assignment.closeout.assignment.public.codex_agent_task.default",
            },
            ok: true,
          },
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
    expect(result.results[0]?.summary).toContain("auto-run: completed")
    expect(result.results[0]?.summary).toContain("assignment run: completed")
    expect(result.results[0]?.summary).toContain("closeout: accepted, no-spend, not_applicable")
    expect(result.results[0]?.summary).toContain("blocker refs: none")
    expect(result.results[0]?.summary).toContain("closeout ref: assignment.closeout.assignment.public.codex_agent_task.default")
    expect(result.results[0]?.summary).toContain("lifecycle:")
    expect(result.results[0]?.summary).toContain("assignment_run.completed (status=accepted)")
    const heartbeatIndex = calls.findIndex(call => pylonArgs(call)[0] === "presence")
    const requestIndex = calls.findIndex(call => pylonArgs(call)[0] === "khala" && pylonArgs(call)[1] === "request")
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
              accountRef: null,
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
              accountRef: null,
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

  test("spawnCodexInstances refuses immediately when Pylon reports no Codex capacity", async () => {
    const fixture = await tempPylonFixture()
    const runner = async (input: KhalaCodexFleetCommandInput): Promise<KhalaCodexFleetCommandResult> => {
      const args = pylonArgs(input)
      const joined = args.join(" ")
      if (joined === "provider go-online --json") {
        return ok({
          ok: true,
          ownCapacityDispatch: {
            availableCodexAssignments: 0,
            maxCodexAssignments: 4,
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
      return failed(`unexpected command: ${joined}`)
    }

    await expect(spawnCodexInstances({
      count: 1,
      prompt: "Run the public fixture.",
    }, {
      env: fixture.env,
      runner,
    })).rejects.toThrow("No Pylon Codex assignment capacity is available right now")
  })

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
      if (args[0] === "khala" && args[1] === "request") {
        return ok({
          assignmentRef: "assignment.public.codex_agent_task.failed_autorun",
          assignmentLifecycleEvents: [
            {
              assignmentRef: "assignment.public.codex_agent_task.failed_autorun",
              event: "assignment_run.runtime_started",
              leaseRef: "assignment.public.codex_agent_task.failed_autorun",
              schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
            },
            {
              assignmentRef: "assignment.public.codex_agent_task.failed_autorun",
              event: "assignment_run.completed",
              leaseRef: "assignment.public.codex_agent_task.failed_autorun",
              schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
              status: "timed-out",
            },
          ],
          assignmentRun: {
            closeout: {
              blockerRefs: ["blocker.assignment.timeout"],
              paymentMode: "no-spend",
              settlementState: "not_applicable",
              status: "timed-out",
            },
            ok: false,
          },
          autoRun: {
            attempted: true,
            ok: false,
          },
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
    expect(result.modelOutput.text).toContain("assignment_run.completed (status=timed-out)")
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
      if (args[0] === "khala" && args[1] === "request") {
        return {
          exitCode: null,
          signal: null,
          stderr: JSON.stringify({
            assignmentRef: "assignment.public.codex_agent_task.timeout",
            event: "assignment_run.runtime_started",
            leaseRef: "assignment.public.codex_agent_task.timeout",
            phase: "runtime_active",
            schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
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
