import {
  makeKhalaToolRegistry,
  runKhalaMcpServerStdio,
  type KhalaMcpClientPolicy,
  type KhalaToolRegistry,
} from "@openagentsinc/khala-tools"
import {
  createKhalaCodexFleetTools,
  type KhalaCodexFleetToolOptions,
} from "./khala-fleet-tools.js"

export const KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_NAME = "khala_fleet" as const
export const KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_VERSION = "0.1.0" as const

export const khalaCodeDesktopFleetMcpPolicy: KhalaMcpClientPolicy = {
  allowedAuthorities: ["local_node_control"],
  denyHighRisk: false,
}

export function createKhalaCodeDesktopFleetMcpRegistry(
  options: KhalaCodexFleetToolOptions = {},
): KhalaToolRegistry {
  return makeKhalaToolRegistry(createKhalaCodexFleetTools(options))
}

export async function runKhalaCodeDesktopFleetMcpServerStdio(): Promise<void> {
  await runKhalaMcpServerStdio({
    policy: khalaCodeDesktopFleetMcpPolicy,
    registry: createKhalaCodeDesktopFleetMcpRegistry(),
    serverName: KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_NAME,
    serverVersion: KHALA_CODE_DESKTOP_FLEET_MCP_SERVER_VERSION,
  })
}

if (import.meta.main) {
  await runKhalaCodeDesktopFleetMcpServerStdio()
}
