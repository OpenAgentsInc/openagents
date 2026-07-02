import { resolve } from "node:path"

type FleetMcpBridgeEnv = Readonly<Record<string, string | undefined>>
const CLAUDE_FLEET_MCP_SERVER_NAME = "khala_fleet"

export type ClaudeFleetMcpServerDescriptor = {
  readonly args: readonly string[]
  readonly command: string
  readonly cwd: string
  readonly type: "stdio"
}

export function claudeFleetMcpBridgeEnabled(env: FleetMcpBridgeEnv): boolean {
  return env.KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE !== "0"
}

export function claudeFleetMcpServerDescriptor(input: {
  readonly bunCommand?: string | undefined
  readonly repoRoot: string
}): ClaudeFleetMcpServerDescriptor {
  const serverScript = resolve(
    input.repoRoot,
    "clients/khala-code-desktop/src/bun/khala-fleet-mcp-server.ts",
  )
  return {
    args: [serverScript],
    command: input.bunCommand?.trim() || "bun",
    cwd: input.repoRoot,
    type: "stdio",
  }
}

export function withClaudeFleetMcpBridgeOptions(
  input: {
    readonly env: FleetMcpBridgeEnv
    readonly options: Record<string, unknown>
    readonly repoRoot: string
  },
): Record<string, unknown> {
  if (!claudeFleetMcpBridgeEnabled(input.env)) return input.options
  const existing = typeof input.options.mcpServers === "object" && input.options.mcpServers !== null
    ? input.options.mcpServers as Record<string, unknown>
    : {}
  return {
    ...input.options,
    mcpServers: {
      ...existing,
      [CLAUDE_FLEET_MCP_SERVER_NAME]: claudeFleetMcpServerDescriptor({
        bunCommand: input.env.KHALA_CODE_DESKTOP_BUN_COMMAND,
        repoRoot: input.repoRoot,
      }),
    },
  }
}
