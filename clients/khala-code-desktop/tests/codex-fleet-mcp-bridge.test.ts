import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"
import { describe, expect, test } from "bun:test"

import {
  codexFleetMcpBridgeConfig,
  codexFleetMcpBridgeEnabled,
  ensureCodexFleetMcpBridge,
} from "../src/bun/codex-fleet-mcp-bridge"
import {
  createKhalaCodeDesktopFleetMcpRegistry,
  khalaCodeDesktopFleetMcpPolicy,
} from "../src/bun/khala-fleet-mcp-server"
import type {
  KhalaFleetRunControlInput,
  KhalaFleetRunSnapshot,
  KhalaFleetRunStartInput,
  KhalaFleetRunStatusInput,
  KhalaFleetRunSupervisorManager,
} from "../src/bun/khala-fleet-tools"
import { createCodexAppServerHost } from "../src/bun/codex-app-server-client"
import { handleKhalaMcpRequest } from "@openagentsinc/khala-tools"

const fixtureAppServerPath = fileURLToPath(
  new URL("../src/bun/fixture-codex-app-server.ts", import.meta.url),
)

const fleetRunSnapshot = (input: {
  readonly active?: boolean
  readonly runRef?: string
  readonly state?: "draft" | "running" | "paused" | "draining" | "stopped" | "completed"
} = {}): KhalaFleetRunSnapshot => {
  const now = "2026-07-01T12:00:00.000Z"
  return {
    active: input.active ?? true,
    lastTick: {
      activeAssignments: 1,
      claimed: 1,
      dispatched: 1,
      freeSlots: 0,
      run: {
        schema: "openagents.khala_code.fleet_run.v1",
        runRef: input.runRef ?? "fleet_run.test",
        objective: "Burn down the fixture backlog.",
        workSource: "fixture",
        targetConcurrency: 2,
        workerKind: "codex",
        refillPolicy: {
          cooldownAware: true,
          maxPerAccount: 1,
          stopCondition: "backlog_empty",
        },
        state: input.state ?? "running",
        dispatchKind: "supervised_dispatch",
        dagTracked: true,
        startedAt: now,
        counters: {
          activeAssignments: 1,
          blockedAssignments: 0,
          completedAssignments: 0,
          failedAssignments: 0,
          workUnitsTotal: 5,
        },
        createdAt: now,
        updatedAt: now,
      },
    },
    lifecycle: [{ kind: "tick", runRef: input.runRef ?? "fleet_run.test", activeAssignments: 1, freeSlots: 1 }],
    pylonRef: "pylon.owner",
    run: {
      schema: "openagents.khala_code.fleet_run.v1",
      runRef: input.runRef ?? "fleet_run.test",
      objective: "Burn down the fixture backlog.",
      workSource: "fixture",
      targetConcurrency: 2,
      workerKind: "codex",
      refillPolicy: {
        cooldownAware: true,
        maxPerAccount: 1,
        stopCondition: "backlog_empty",
      },
      state: input.state ?? "running",
      dispatchKind: "supervised_dispatch",
      dagTracked: true,
      startedAt: now,
      counters: {
        activeAssignments: 1,
        blockedAssignments: 0,
        completedAssignments: 0,
        failedAssignments: 0,
        workUnitsTotal: 5,
      },
      createdAt: now,
      updatedAt: now,
    },
  }
}

const mockFleetRunSupervisor = () => {
  const calls: {
    controls: KhalaFleetRunControlInput[]
    starts: KhalaFleetRunStartInput[]
    statuses: KhalaFleetRunStatusInput[]
  } = { controls: [], starts: [], statuses: [] }
  const manager: KhalaFleetRunSupervisorManager = {
    control: async input => {
      calls.controls.push(input)
      const state = input.verb === "pause" ? "paused" : input.verb === "drain" ? "draining" : input.verb === "stop" ? "stopped" : "running"
      return { ...fleetRunSnapshot({ active: input.verb !== "stop", runRef: input.runRef, state }), verb: input.verb }
    },
    start: async input => {
      calls.starts.push(input)
      return fleetRunSnapshot({ runRef: input.runRef ?? "fleet_run.mock" })
    },
    status: async input => {
      calls.statuses.push(input)
      return input.runRef === undefined
        ? [fleetRunSnapshot({ runRef: "fleet_run.mock" })]
        : fleetRunSnapshot({ runRef: input.runRef })
    },
  }
  return { calls, manager }
}

describe("Codex Fleet MCP bridge", () => {
  test("projects the local Fleet server as a prompted Codex MCP config", () => {
    const config = codexFleetMcpBridgeConfig({
      bunCommand: "/usr/local/bin/bun",
      repoRoot: "/repo/openagents",
    })

    expect(config).toMatchObject({
      args: ["/repo/openagents/clients/khala-code-desktop/src/bun/khala-fleet-mcp-server.ts"],
      command: "/usr/local/bin/bun",
      cwd: "/repo/openagents",
      enabledTools: [
        "pylon_ensure",
        "codex_fleet_status",
        "codex_spawn",
        "fleet_run_start",
        "fleet_run_status",
        "fleet_run_control",
      ],
      serverName: "khala_fleet",
    })
    expect(config.writes).toEqual([
      { keyPath: "mcp_servers.khala_fleet.command", value: "/usr/local/bin/bun" },
      {
        keyPath: "mcp_servers.khala_fleet.args",
        value: ["/repo/openagents/clients/khala-code-desktop/src/bun/khala-fleet-mcp-server.ts"],
      },
      { keyPath: "mcp_servers.khala_fleet.cwd", value: "/repo/openagents" },
      { keyPath: "mcp_servers.khala_fleet.enabled", value: true },
      { keyPath: "mcp_servers.khala_fleet.default_tools_approval_mode", value: "prompt" },
      {
        keyPath: "mcp_servers.khala_fleet.enabled_tools",
        value: [
          "pylon_ensure",
          "codex_fleet_status",
          "codex_spawn",
          "fleet_run_start",
          "fleet_run_status",
          "fleet_run_control",
        ],
      },
    ])
  })

  test("registers the Fleet config through app-server and reloads MCP", async () => {
    const requests: Array<{ method: string; params: unknown }> = []
    const host = {
      request: async (method: string, params?: unknown) => {
        requests.push({ method, params })
        return {}
      },
    }

    await expect(ensureCodexFleetMcpBridge({
      env: { KHALA_CODE_DESKTOP_BUN_COMMAND: "bun-test" },
      host: host as never,
      repoRoot: "/repo/openagents",
    })).resolves.toMatchObject({
      changed: true,
      ok: true,
    })

    expect(requests.map(request => request.method)).toEqual([
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/value/write",
      "config/mcpServer/reload",
    ])
    expect(requests[0]?.params).toEqual({
      keyPath: "mcp_servers.khala_fleet.command",
      mergeStrategy: "replace",
      value: "bun-test",
    })
  })

  test("can be disabled without writing Codex config", async () => {
    const host = {
      request: async () => {
        throw new Error("should not write")
      },
    }

    expect(codexFleetMcpBridgeEnabled({ KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE: "0" })).toBe(false)
    await expect(ensureCodexFleetMcpBridge({
      env: { KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE: "0" },
      host: host as never,
      repoRoot: "/repo/openagents",
    })).resolves.toMatchObject({
      changed: false,
      ok: true,
    })
  })

  test("Fleet MCP stdio registry exposes only supplemental swarm tools", async () => {
    const response = await handleKhalaMcpRequest(
      { id: "tools", jsonrpc: "2.0", method: "tools/list" },
      {
        policy: khalaCodeDesktopFleetMcpPolicy,
        registry: createKhalaCodeDesktopFleetMcpRegistry(),
      },
    )
    const tools = response.result?.tools as Array<{ name: string; annotations: { khalaAuthority: string } }>

    expect(tools.map(tool => tool.name)).toEqual([
      "pylon_ensure",
      "codex_fleet_status",
      "codex_spawn",
      "fleet_run_start",
      "fleet_run_status",
      "fleet_run_control",
    ])
    expect(tools.every(tool => tool.annotations.khalaAuthority === "owner_full_access")).toBe(true)
  })

  test("Fleet MCP run verbs call the mocked FleetRun supervisor with typed inputs", async () => {
    const mock = mockFleetRunSupervisor()
    const registry = createKhalaCodeDesktopFleetMcpRegistry({
      fleetRunSupervisor: mock.manager,
    })

    const start = await handleKhalaMcpRequest(
      {
        id: "start",
        jsonrpc: "2.0",
        method: "tools/call",
        params: {
          arguments: {
            fixture_count: 5,
            objective: "Burn down the fixture backlog.",
            pylon_ref: "pylon.owner",
            run_ref: "fleet_run.mock",
            target_concurrency: 2,
            worker_kind: "codex",
            work_source: "fixture",
          },
          name: "fleet_run_start",
        },
      },
      { policy: khalaCodeDesktopFleetMcpPolicy, registry },
    )

    expect(start.result?.content).toEqual([expect.objectContaining({
      text: expect.stringContaining("FleetRun fleet_run.mock: running"),
      type: "text",
    })])
    expect(mock.calls.starts).toEqual([{
      objective: "Burn down the fixture backlog.",
      fixtureCount: 5,
      pylonRef: "pylon.owner",
      runRef: "fleet_run.mock",
      targetConcurrency: 2,
      workerKind: "codex",
      workSource: "fixture",
      baseUrl: undefined,
      branch: undefined,
      commit: undefined,
      issues: undefined,
      repo: undefined,
      timeoutMs: undefined,
      verify: undefined,
    }])

    const status = await handleKhalaMcpRequest(
      {
        id: "status",
        jsonrpc: "2.0",
        method: "tools/call",
        params: {
          arguments: { run_ref: "fleet_run.mock" },
          name: "fleet_run_status",
        },
      },
      { policy: khalaCodeDesktopFleetMcpPolicy, registry },
    )

    expect(status.result?.content).toEqual([expect.objectContaining({
      text: expect.stringContaining("FleetRun fleet_run.mock: running"),
      type: "text",
    })])
    expect(mock.calls.statuses).toEqual([{ runRef: "fleet_run.mock" }])

    const control = await handleKhalaMcpRequest(
      {
        id: "control",
        jsonrpc: "2.0",
        method: "tools/call",
        params: {
          arguments: { run_ref: "fleet_run.mock", verb: "pause" },
          name: "fleet_run_control",
        },
      },
      { policy: khalaCodeDesktopFleetMcpPolicy, registry },
    )

    expect(control.result?.content).toEqual([expect.objectContaining({
      text: expect.stringContaining("FleetRun fleet_run.mock: paused"),
      type: "text",
    })])
    expect(mock.calls.controls).toEqual([{ runRef: "fleet_run.mock", verb: "pause" }])
  })

  test("Fleet MCP run verbs round-trip through the scripted Codex app-server fixture", async () => {
    const root = await mkdtemp(join(tmpdir(), "khala-fleet-mcp-app-server-"))
    const host = createCodexAppServerHost({
      env: {
        CODEX_HOME: join(root, "codex-home"),
        KHALA_CODE_BUN_BINARY: process.execPath,
        KHALA_CODE_CODEX_APP_SERVER_FIXTURE: "1",
        KHALA_CODE_CODEX_APP_SERVER_FIXTURE_PATH: fixtureAppServerPath,
      } as NodeJS.ProcessEnv,
      initializeTimeoutMs: 1_000,
      requestTimeoutMs: 1_000,
    })

    try {
      await expect(host.start()).resolves.toMatchObject({ ok: true })
      await expect(ensureCodexFleetMcpBridge({
        env: { KHALA_CODE_DESKTOP_BUN_COMMAND: process.execPath },
        host,
        repoRoot: process.cwd(),
      })).resolves.toMatchObject({ ok: true })

      const start = await host.request<{ content: Array<{ text: string; type: string }> }>(
        "mcpServer/tool/call",
        {
          arguments: {
            fixture_count: 5,
            objective: "Burn down the fixture backlog.",
            run_ref: "fleet_run.fixture_mcp",
            target_concurrency: 2,
            work_source: "fixture",
          },
          server: "khala_fleet",
          threadId: "thread.fixture",
          tool: "fleet_run_start",
        },
      )
      expect(start.content).toEqual([expect.objectContaining({
        text: expect.stringContaining("FleetRun fleet_run.fixture_mcp: running"),
        type: "text",
      })])

      const status = await host.request<{ content: Array<{ text: string; type: string }> }>(
        "mcpServer/tool/call",
        {
          arguments: { run_ref: "fleet_run.fixture_mcp" },
          server: "khala_fleet",
          threadId: "thread.fixture",
          tool: "fleet_run_status",
        },
      )

      expect(status.content).toEqual([expect.objectContaining({
        text: expect.stringContaining("FleetRun fleet_run.fixture_mcp: running"),
        type: "text",
      })])
    } finally {
      host.dispose()
      await rm(root, { force: true, recursive: true })
    }
  })
})
