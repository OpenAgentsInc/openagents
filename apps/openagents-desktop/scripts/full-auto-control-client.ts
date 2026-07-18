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

export type ControlConnection = Readonly<{
  url: string
  token: string
  /** Additive #8928 ownership fields. Old v1 files legitimately omit them. */
  pid?: number
  serverInstanceId?: string
}>

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
  return {
    url: parsed.url,
    token: parsed.token,
    ...(typeof parsed.pid === "number" && Number.isInteger(parsed.pid) && parsed.pid > 0
      ? { pid: parsed.pid }
      : {}),
    ...(typeof parsed.serverInstanceId === "string" && parsed.serverInstanceId.length >= 16
      ? { serverInstanceId: parsed.serverInstanceId }
      : {}),
  }
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
  lanes: () => call(connection, "GET", "/v1/lanes"),
  list: () => call(connection, "GET", "/v1/full-auto"),
  status: (threadRef: string) =>
    call(connection, "GET", `/v1/full-auto/${encodeURIComponent(threadRef)}`),
  start: (workspaceRef: string, title?: string, lane?: string) =>
    call(connection, "POST", "/v1/full-auto/start", {
      workspaceRef,
      ...(title === undefined ? {} : { title }),
      ...(lane === undefined ? {} : { lane }),
    }),
  enable: (threadRef: string, workspaceRef: string, lane?: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/enable`, {
      workspaceRef,
      ...(lane === undefined ? {} : { lane }),
    }),
  disable: (threadRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/disable`),
  continueNow: (threadRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/continue-now`),
  turns: (threadRef: string) =>
    call(connection, "GET", `/v1/full-auto/${encodeURIComponent(threadRef)}/turns`),
  // FA-RUN-01 (#8969): the durable FullAutoRun lifecycle surface.
  runsList: () => call(connection, "GET", "/v1/full-auto/runs"),
  runsStart: (input: Readonly<{
    workspaceRef: string
    title: string
    objective: string
    doneCondition: string
    lane?: string
    turnCap?: number
  }>) => call(connection, "POST", "/v1/full-auto/runs/start", input),
  runStatus: (runRef: string) =>
    call(connection, "GET", `/v1/full-auto/runs/${encodeURIComponent(runRef)}`),
  runPause: (runRef: string) =>
    call(connection, "POST", `/v1/full-auto/runs/${encodeURIComponent(runRef)}/pause`),
  runResume: (runRef: string) =>
    call(connection, "POST", `/v1/full-auto/runs/${encodeURIComponent(runRef)}/resume`),
  runStop: (runRef: string) =>
    call(connection, "POST", `/v1/full-auto/runs/${encodeURIComponent(runRef)}/stop`),
  // FA-RUN-04 (#8972) / FA-RPT-01 (#8988): the private run report and its
  // derived public-safe receipt -- thin pass-throughs like everything else.
  runReport: (runRef: string) =>
    call(connection, "GET", `/v1/full-auto/runs/${encodeURIComponent(runRef)}/report`),
  runReceipt: (runRef: string) =>
    call(connection, "GET", `/v1/full-auto/runs/${encodeURIComponent(runRef)}/receipt`),
})

export type VerifiedControlProcessIdentity = Readonly<{
  pid: number
  serverInstanceId: string
}>

/**
 * #8928: fail-closed ownership guard for any cleanup automation that is
 * considering an OS signal. An old connection file remains usable for normal
 * API discovery, but cannot authorize a signal. A current file authorizes no
 * signal unless the bearer-gated live server echoes its exact opaque identity.
 * Callers must still prefer the child handle they spawned and re-read the file
 * immediately before acting, as documented in the shared-Mac runbook.
 */
export const verifyControlProcessIdentity = async (
  connection: ControlConnection,
): Promise<VerifiedControlProcessIdentity> => {
  if (connection.pid === undefined || connection.serverInstanceId === undefined) {
    throw new ControlUnavailableError(
      "Full Auto control ownership is unverifiable: this connection file has no PID/instance identity; do not signal a process.",
    )
  }
  const result = await controlOperations(connection).list()
  const body = typeof result.body === "object" && result.body !== null
    ? result.body as Record<string, unknown>
    : null
  if (result.status !== 200 || body?.serverInstanceId !== connection.serverInstanceId) {
    throw new ControlUnavailableError(
      "Full Auto control ownership is unverifiable: the authenticated server did not echo the connection file's instance identity; do not signal a process.",
    )
  }
  return { pid: connection.pid, serverInstanceId: connection.serverInstanceId }
}
