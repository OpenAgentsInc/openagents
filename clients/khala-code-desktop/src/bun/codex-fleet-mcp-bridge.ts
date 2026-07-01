import { resolve } from "node:path"

import type { CodexAppServerHost } from "./codex-app-server-client.js"
import { KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_NAME } from "./khala-fleet-mcp-server.js"

type FleetMcpBridgeEnv = Readonly<Record<string, string | undefined>>

export type CodexFleetMcpConfigWrite = {
  readonly keyPath: string
  readonly value: unknown
}

export type CodexFleetMcpBridgeConfig = {
  readonly args: readonly string[]
  readonly command: string
  readonly cwd: string
  readonly enabledTools: readonly string[]
  readonly serverName: typeof KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_NAME
  readonly writes: readonly CodexFleetMcpConfigWrite[]
}

export type CodexFleetMcpBridgeEnsureResult = {
  readonly ok: boolean
  readonly changed: boolean
  readonly config: CodexFleetMcpBridgeConfig
  readonly error?: string
}

export function codexFleetMcpBridgeEnabled(env: FleetMcpBridgeEnv): boolean {
  return env.KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE !== "0"
}

export function codexFleetMcpBridgeConfig(input: {
  readonly repoRoot: string
  readonly bunCommand?: string | undefined
}): CodexFleetMcpBridgeConfig {
  const serverScript = resolve(
    input.repoRoot,
    "clients/khala-code-desktop/src/bun/khala-fleet-mcp-server.ts",
  )
  const command = input.bunCommand?.trim() || "bun"
  const enabledTools = [
    "pylon_ensure",
    "codex_fleet_status",
    "codex_spawn",
    "fleet_run_start",
    "fleet_run_status",
    "fleet_run_control",
  ] as const
  const prefix = `mcp_servers.${KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_NAME}`
  const writes: CodexFleetMcpConfigWrite[] = [
    { keyPath: `${prefix}.command`, value: command },
    { keyPath: `${prefix}.args`, value: [serverScript] },
    { keyPath: `${prefix}.cwd`, value: input.repoRoot },
    { keyPath: `${prefix}.enabled`, value: true },
    { keyPath: `${prefix}.default_tools_approval_mode`, value: "prompt" },
    { keyPath: `${prefix}.enabled_tools`, value: [...enabledTools] },
  ]
  return {
    args: [serverScript],
    command,
    cwd: input.repoRoot,
    enabledTools,
    serverName: KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_NAME,
    writes,
  }
}

export async function ensureCodexFleetMcpBridge(input: {
  readonly env: FleetMcpBridgeEnv
  readonly host?: CodexAppServerHost | undefined
  readonly repoRoot: string
}): Promise<CodexFleetMcpBridgeEnsureResult> {
  const config = codexFleetMcpBridgeConfig({
    bunCommand: input.env.KHALA_CODE_DESKTOP_BUN_COMMAND,
    repoRoot: input.repoRoot,
  })
  if (!codexFleetMcpBridgeEnabled(input.env)) {
    return { ok: true, changed: false, config }
  }
  if (input.host === undefined) {
    return {
      ok: false,
      changed: false,
      config,
      error: "Codex app-server host is not configured.",
    }
  }
  try {
    for (const write of config.writes) {
      await input.host.request("config/value/write", {
        keyPath: write.keyPath,
        mergeStrategy: "replace",
        value: write.value,
      })
    }
    await input.host.request("config/mcpServer/reload")
    return { ok: true, changed: true, config }
  } catch (error) {
    return {
      ok: false,
      changed: false,
      config,
      error: error instanceof Error ? error.message : String(error),
    }
  }
}
