import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import {
  decodeSessionSummary,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"

type NodeHealth = {
  ok?: unknown
  schema?: unknown
}

type CommandResponse = {
  ok?: unknown
  result?: unknown
}

export function readControlToken(pylonHome: string): string | null {
  const tokenPath = join(pylonHome, "control-token")
  if (!existsSync(tokenPath)) return null

  const token = readFileSync(tokenPath, "utf8").trim()
  return token.length > 0 ? token : null
}

export async function fetchNodeState(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<{ ok: boolean; schema: string; sessions: SessionSummary[] }> {
  const fetchFn = input.fetchFn ?? fetch
  const baseUrl = input.baseUrl.replace(/\/+$/, "")

  const healthResponse = await fetchFn(`${baseUrl}/health`)
  if (!healthResponse.ok) {
    throw new Error(`Pylon health request failed: ${healthResponse.status}`)
  }

  const health = (await healthResponse.json()) as NodeHealth
  if (typeof health.ok !== "boolean" || typeof health.schema !== "string") {
    throw new Error("Pylon health response was not a control health payload")
  }

  const commandResponse = await fetchFn(`${baseUrl}/command`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${input.token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ type: "session.list" }),
  })
  if (!commandResponse.ok) {
    throw new Error(`Pylon session list request failed: ${commandResponse.status}`)
  }

  const command = (await commandResponse.json()) as CommandResponse
  if (command.ok !== true) {
    throw new Error("Pylon session list command failed")
  }
  if (!Array.isArray(command.result)) {
    throw new Error("Pylon session list command did not return an array")
  }

  return {
    ok: health.ok,
    schema: health.schema,
    sessions: command.result.map((row) => decodeSessionSummary(row)),
  }
}
