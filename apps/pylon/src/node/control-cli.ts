// CLI bridge to the loopback control server (issue #5035). The Autopilot
// desktop GUI drives a running `pylon node` over the loopback control API
// (127.0.0.1:4716 + a per-home bearer token). These helpers give the headless
// CLI the SAME steering surface — `pylon sessions`, `pylon approvals`,
// `pylon deploy` — by resolving the node home + control token the same way the
// node does, then sending typed control commands.
//
// A CLI verb here is a CONTROL SURFACE only: it forwards a command to the
// already-running node, which owns all execution + spend authority. No new
// money/wallet/spend authority is introduced on the CLI side.

import { join } from "node:path"
import {
  controlTokenFileName,
  defaultControlPort,
  type ControlCommand,
} from "./control-server.js"
import { sendControlCommand } from "./control-client.js"
import { resolvePylonHome } from "../bootstrap.js"

export type ResolvedControlEndpoint = {
  baseUrl: string
  token: string
  home: string
  tokenPath: string
}

export class ControlEndpointError extends Error {
  readonly code: "no_node" | "no_token"
  constructor(code: "no_node" | "no_token", message: string) {
    super(message)
    this.name = "ControlEndpointError"
    this.code = code
  }
}

// Resolve the node home the SAME way the node + the rest of the CLI does
// (issue: Orwell PYLON_HOME auto-discovery). An explicit PYLON_HOME always
// wins; otherwise the seed-bearing home (`~/.openagents/pylon` preferred over a
// bare `~/.pylon`) is discovered so a CLI control verb finds the running node's
// control token instead of a wrong, seedless `~/.pylon`.
function resolvePylonHomeDir(env: NodeJS.ProcessEnv): string {
  return resolvePylonHome(env).home
}

function resolveControlBaseUrl(env: NodeJS.ProcessEnv): string {
  const explicit = env.PYLON_CONTROL_URL
  if (explicit && explicit.trim().length > 0) return explicit.replace(/\/+$/, "")
  const host = env.PYLON_CONTROL_HOST ?? "127.0.0.1"
  const port = Number(env.PYLON_CONTROL_PORT ?? defaultControlPort)
  return `http://${host}:${Number.isFinite(port) ? port : defaultControlPort}`
}

// Resolve the loopback control endpoint the same way the node does: the home
// holds the `control-token` file written by `ensureControlToken`. We read it
// here (never write it) so a CLI verb can authenticate to an already-running
// node. The token can also be supplied via PYLON_CONTROL_TOKEN for environments
// that inject it directly.
export async function resolveControlEndpoint(
  env: NodeJS.ProcessEnv,
): Promise<ResolvedControlEndpoint> {
  const home = resolvePylonHomeDir(env)
  const tokenPath = join(home, controlTokenFileName)
  const baseUrl = resolveControlBaseUrl(env)

  const envToken = env.PYLON_CONTROL_TOKEN
  let token = envToken && envToken.trim().length > 0 ? envToken.trim() : ""
  if (token.length === 0) {
    const file = Bun.file(tokenPath)
    if (await file.exists()) {
      token = (await file.text()).trim()
    }
  }
  if (token.length === 0) {
    throw new ControlEndpointError(
      "no_token",
      `no control token found at ${tokenPath} (start a node with \`pylon node\` or set PYLON_CONTROL_TOKEN)`,
    )
  }
  return { baseUrl, token, home, tokenPath }
}

// Send a typed control command to the running node. Surfaces a clean
// ControlEndpointError("no_node") when nothing is listening on the loopback
// control port so callers can emit honest JSON + a nonzero exit.
export async function runControlCommand(
  command: ControlCommand,
  env: NodeJS.ProcessEnv = Bun.env,
): Promise<{ endpoint: ResolvedControlEndpoint; result: unknown }> {
  const endpoint = await resolveControlEndpoint(env)
  try {
    const result = await sendControlCommand(endpoint.baseUrl, endpoint.token, command)
    return { endpoint, result }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error)
    // A refused connection means no node is running on the loopback port.
    if (
      message.includes("ECONNREFUSED") ||
      message.includes("Unable to connect") ||
      message.includes("Failed to fetch") ||
      message.includes("fetch failed") ||
      message.includes("connection refused")
    ) {
      throw new ControlEndpointError(
        "no_node",
        `no Pylon node reachable at ${endpoint.baseUrl} (start one with \`pylon node\`): ${message}`,
      )
    }
    throw error
  }
}
