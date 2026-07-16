/**
 * FA-H13 (#8886): the shared thin client for the Full Auto local control API,
 * used by both scripts/full-auto-cli.ts and scripts/full-auto-mcp.ts. It is
 * deliberately a pass-through: discover the loopback server from the
 * mode-0600 connection file Desktop main writes (full-auto/control.json under
 * the Electron userData directory), attach the bearer credential, call the
 * HTTP API described by GET /v1/openapi.json, and return the JSON verbatim.
 * No client-side policy, no schema re-implementation -- the OpenAPI document
 * served by Desktop main is the single source of truth.
 */
import { existsSync, readFileSync } from "node:fs"
import { homedir } from "node:os"
import path from "node:path"

export type ControlConnection = Readonly<{ url: string; token: string }>

/** Matches Electron's `app.getPath("userData")` for productName "OpenAgents". */
const defaultUserDataDir = (): string => {
  if (process.platform === "darwin") {
    return path.join(homedir(), "Library", "Application Support", "OpenAgents")
  }
  if (process.platform === "win32") {
    return path.join(process.env.APPDATA ?? path.join(homedir(), "AppData", "Roaming"), "OpenAgents")
  }
  return path.join(process.env.XDG_CONFIG_HOME ?? path.join(homedir(), ".config"), "OpenAgents")
}

export const resolveUserDataDir = (explicit: string | undefined): string =>
  explicit ?? process.env.OPENAGENTS_DESKTOP_USER_DATA ?? defaultUserDataDir()

export const controlFilePathFor = (userDataDir: string): string =>
  path.join(userDataDir, "full-auto", "control.json")

export class ControlUnavailableError extends Error {
  override readonly name = "ControlUnavailableError"
}

export const readControlConnection = (userDataDir: string): ControlConnection => {
  const filePath = controlFilePathFor(userDataDir)
  if (!existsSync(filePath)) {
    throw new ControlUnavailableError(
      `Full Auto control connection file not found at ${filePath}. ` +
        "The control server is off by default: launch OpenAgents Desktop with " +
        "OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 to enable it (and pass --user-data " +
        "if the app runs against a non-default userData directory).",
    )
  }
  const parsed = JSON.parse(readFileSync(filePath, "utf8")) as Record<string, unknown>
  if (typeof parsed.url !== "string" || typeof parsed.token !== "string") {
    throw new ControlUnavailableError(
      `Full Auto control connection file at ${filePath} is malformed; relaunch the app to rewrite it.`,
    )
  }
  return { url: parsed.url, token: parsed.token }
}

export type ControlResult = Readonly<{ status: number; body: unknown }>

const call = async (
  connection: ControlConnection,
  method: "GET" | "POST",
  pathname: string,
  body?: unknown,
): Promise<ControlResult> => {
  const response = await fetch(`${connection.url}${pathname}`, {
    method,
    headers: {
      authorization: `Bearer ${connection.token}`,
      ...(body === undefined ? {} : { "content-type": "application/json" }),
    },
    ...(body === undefined ? {} : { body: JSON.stringify(body) }),
  })
  return { status: response.status, body: await response.json() }
}

export const controlOperations = (connection: ControlConnection) => ({
  openapi: () => call(connection, "GET", "/v1/openapi.json"),
  list: () => call(connection, "GET", "/v1/full-auto"),
  status: (threadRef: string) =>
    call(connection, "GET", `/v1/full-auto/${encodeURIComponent(threadRef)}`),
  enable: (threadRef: string, workspaceRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/enable`, { workspaceRef }),
  disable: (threadRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/disable`),
  continueNow: (threadRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/continue-now`),
  turns: (threadRef: string) =>
    call(connection, "GET", `/v1/full-auto/${encodeURIComponent(threadRef)}/turns`),
})
