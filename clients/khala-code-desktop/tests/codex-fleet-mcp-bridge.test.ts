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
import { handleKhalaMcpRequest } from "@openagentsinc/khala-tools"

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
      enabledTools: ["pylon_ensure", "codex_fleet_status", "codex_spawn"],
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
        value: ["pylon_ensure", "codex_fleet_status", "codex_spawn"],
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
    ])
    expect(tools.every(tool => tool.annotations.khalaAuthority === "owner_full_access")).toBe(true)
  })
})
