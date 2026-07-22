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

/** FA-WIRE-01 (#8996): pass-through shapes for the ordered routing policy and
 * owner guardrails -- the server-side schemas are the source of truth. */
export type ControlRoutingCandidate = Readonly<{ lane: string; accountRef?: string }>
export type ControlGuardrails = Readonly<{
  maxWallClockMs?: number
  maxTurns?: number
  maxPerTurnFailures?: number
  tokenBudgetRef?: string
}>
export type ControlPolicyOptions = Readonly<{
  routingPolicy?: ReadonlyArray<ControlRoutingCandidate>
  guardrails?: ControlGuardrails
}>

/**
 * FA-WIRE-01 (#8996) / FA-RT-03 (fleet lane refs): the namespaced lane-ref
 * prefixes whose FIRST colon is part of the lane ref itself, not an account
 * separator. These are the real admitted fleet lane namespaces -- the ACP
 * peers (`acp:grok-cli`, `acp:cursor-agent`) and the SDK harness lanes
 * (`harness:opencode`, `harness:pi`, `harness:goose`). A bare `<namespace>:<ref>`
 * with exactly one colon is the whole lane ref; pin an account with a SECOND
 * colon (`harness:opencode:<accountRef>`).
 */
export const FULL_AUTO_NAMESPACED_LANE_PREFIXES: ReadonlyArray<string> = ["acp:", "harness:"]

/**
 * FA-WIRE-01 (#8996): parse one repeatable CLI `--lane <laneRef[:accountRef]>`
 * value into an ordered routing candidate. Lane refs may themselves contain a
 * colon (`acp:grok-cli`, `harness:opencode`), so the account separator is the
 * LAST colon, and a single-colon value in a known lane namespace (see
 * FULL_AUTO_NAMESPACED_LANE_PREFIXES) is treated as a bare lane ref.
 *
 * FA-RT-03: before this generalization the account split fired for every
 * prefix except `acp:`, so `--lane harness:opencode` was mis-parsed to
 * laneRef `harness` + accountRef `opencode` and rejected by the control API as
 * `lane_unknown: harness` -- making the opencode/pi/goose fleet lanes
 * unreachable from the CLI/control path. Splitting only inside a known
 * namespace keeps them whole.
 */
export const parseFullAutoLaneOption = (value: string): ControlRoutingCandidate => {
  const lastColon = value.lastIndexOf(":")
  if (lastColon === -1) return { lane: value }
  const colonCount = value.split(":").length - 1
  if (
    colonCount === 1 &&
    FULL_AUTO_NAMESPACED_LANE_PREFIXES.some(prefix => value.startsWith(prefix))
  ) {
    return { lane: value }
  }
  const lane = value.slice(0, lastColon)
  const accountRef = value.slice(lastColon + 1)
  return accountRef.length === 0 ? { lane } : { lane, accountRef }
}

/** FA-WIRE-01 (#8996): fold the CLI's repeatable lanes + guardrail flags into
 * the request options. One bare lane keeps the legacy single-lane shape (no
 * routingPolicy); an accountRef or a second lane creates the ordered policy. */
export const buildFullAutoPolicyOptions = (input: Readonly<{
  lanes: ReadonlyArray<string>
  maxTurns?: number
  maxWallClockMs?: number
}>): Readonly<{ lane?: string; options: ControlPolicyOptions }> => {
  const candidates = input.lanes.map(parseFullAutoLaneOption)
  const guardrails: ControlGuardrails = {
    ...(input.maxTurns === undefined ? {} : { maxTurns: input.maxTurns }),
    ...(input.maxWallClockMs === undefined ? {} : { maxWallClockMs: input.maxWallClockMs }),
  }
  const hasGuardrails = Object.keys(guardrails).length > 0
  const usePolicy = candidates.length > 1 ||
    (candidates.length === 1 && candidates[0]!.accountRef !== undefined)
  return {
    ...(candidates.length === 0 ? {} : { lane: candidates[0]!.lane }),
    options: {
      ...(usePolicy ? { routingPolicy: candidates } : {}),
      ...(hasGuardrails ? { guardrails } : {}),
    },
  }
}

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
  start: (workspaceRef: string, title?: string, lane?: string, options?: ControlPolicyOptions) =>
    call(connection, "POST", "/v1/full-auto/start", {
      workspaceRef,
      ...(title === undefined ? {} : { title }),
      ...(lane === undefined ? {} : { lane }),
      ...(options?.routingPolicy === undefined ? {} : { routingPolicy: options.routingPolicy }),
      ...(options?.guardrails === undefined ? {} : { guardrails: options.guardrails }),
    }),
  enable: (threadRef: string, workspaceRef: string, lane?: string, options?: ControlPolicyOptions) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/enable`, {
      workspaceRef,
      ...(lane === undefined ? {} : { lane }),
      ...(options?.routingPolicy === undefined ? {} : { routingPolicy: options.routingPolicy }),
      ...(options?.guardrails === undefined ? {} : { guardrails: options.guardrails }),
    }),
  disable: (threadRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/disable`),
  continueNow: (threadRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/continue-now`),
  // FA-WIRE-01 (#8996): the explicit resume for a FA-GD-01 low-confidence
  // pause -- a thin pass-through like everything else here.
  resume: (threadRef: string) =>
    call(connection, "POST", `/v1/full-auto/${encodeURIComponent(threadRef)}/resume`),
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
    routingPolicy?: ReadonlyArray<ControlRoutingCandidate>
    guardrails?: ControlGuardrails
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
