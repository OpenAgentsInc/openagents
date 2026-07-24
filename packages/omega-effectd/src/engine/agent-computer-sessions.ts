/**
 * AC-01: Agent Computer session join inside omega-effectd.
 *
 * Uses `HarnessEnvironment.openagents_cloud` and the released
 * `HarnessEnvironmentRunner` for Worker cloud coding sessions.
 * Credentials stay runtime-only and never enter durable projections.
 * GPUI must not become receipt authority.
 */

import { createHash } from "node:crypto"
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import { makeReferenceAdapter } from "@openagentsinc/agent-harness-contract"
import {
  makeCloudCodingSessionClient,
  makeOpenAgentsCloudHarnessEnvironment,
  makeOpenAgentsCloudHarnessEnvironmentRunner,
  openAgentsCloudCodingSessionLaunchUrl,
  startHarnessInEnvironment,
  type CloudCodingAdapter,
  type CloudCodingLane,
  type CloudCodingSessionProjection,
  type FetchLike,
} from "@openagentsinc/agent-harness-environment"
import { Effect, Stream } from "effect"

export const AGENT_COMPUTER_SESSION_SCHEMA =
  "openagents.omega.agent_computer_session.v1" as const

export type AgentComputerSessionRecord = Readonly<{
  sessionRef: string
  environment: "openagents_cloud"
  controlPlaneBaseUrl: string
  repoRef: string
  objectiveDigest: string
  state: string
  adapter: string | null
  lane: string | null
  placementRef: string | null
  artifactRef: string | null
  agentComputerRef: string | null
  agentComputerState: string | null
  startedAt: string
  updatedAt: string
}>

const digestObjective = (objective: string): string =>
  createHash("sha256").update(objective).digest("hex")

const projectRecord = (
  projection: CloudCodingSessionProjection,
  meta: Readonly<{
    controlPlaneBaseUrl: string
    repoRef: string
    objectiveDigest: string
    startedAt: string
  }>,
): AgentComputerSessionRecord => ({
  sessionRef: projection.id,
  environment: "openagents_cloud",
  controlPlaneBaseUrl: meta.controlPlaneBaseUrl,
  repoRef: meta.repoRef,
  objectiveDigest: meta.objectiveDigest,
  state: projection.state,
  adapter: projection.adapter ?? null,
  lane: projection.lane ?? null,
  placementRef: projection.placement_ref ?? null,
  artifactRef: projection.artifact_ref ?? null,
  agentComputerRef: projection.agent_computer_ref ?? null,
  agentComputerState: projection.agent_computer_state ?? null,
  startedAt: meta.startedAt,
  updatedAt: new Date().toISOString(),
})

type SessionFile = Readonly<{
  schema: typeof AGENT_COMPUTER_SESSION_SCHEMA
  sessions: ReadonlyArray<AgentComputerSessionRecord>
}>

export type AgentComputerSessionStore = Readonly<{
  get: (sessionRef: string) => AgentComputerSessionRecord | null
  put: (session: AgentComputerSessionRecord) => AgentComputerSessionRecord
  list: () => ReadonlyArray<AgentComputerSessionRecord>
}>

export const openAgentComputerSessionStore = (file: string): AgentComputerSessionStore => {
  const filePath = path.resolve(file)
  mkdirSync(path.dirname(filePath), { recursive: true, mode: 0o700 })
  let sessions: AgentComputerSessionRecord[] = []
  if (existsSync(filePath)) {
    try {
      const parsed = JSON.parse(readFileSync(filePath, "utf8")) as SessionFile
      if (parsed.schema === AGENT_COMPUTER_SESSION_SCHEMA && Array.isArray(parsed.sessions)) {
        sessions = [...parsed.sessions]
      }
    } catch {
      sessions = []
    }
  }
  const persist = (): void => {
    const tmp = `${filePath}.${process.pid}.tmp`
    writeFileSync(
      tmp,
      JSON.stringify(
        { schema: AGENT_COMPUTER_SESSION_SCHEMA, sessions } satisfies SessionFile,
        null,
        2,
      ),
      { mode: 0o600 },
    )
    renameSync(tmp, filePath)
  }
  return {
    get: sessionRef => sessions.find(row => row.sessionRef === sessionRef) ?? null,
    put: session => {
      const index = sessions.findIndex(row => row.sessionRef === session.sessionRef)
      if (index === -1) sessions.push(session)
      else sessions[index] = session
      persist()
      return session
    },
    list: () => [...sessions],
  }
}

export type StartAgentComputerSessionInput = Readonly<{
  bearerToken: string
  controlPlaneBaseUrl: string
  repoRef: string
  objective: string
  adapter?: CloudCodingAdapter
  lane?: CloudCodingLane
  verify?: ReadonlyArray<string>
  fetch?: FetchLike
  now?: () => Date
}>

export type StartAgentComputerSessionResult =
  | Readonly<{ ok: true; session: AgentComputerSessionRecord }>
  | Readonly<{ ok: false; code: string; message: string }>

export const startAgentComputerSession = async (
  store: AgentComputerSessionStore,
  input: StartAgentComputerSessionInput,
): Promise<StartAgentComputerSessionResult> => {
  const bearerToken = input.bearerToken.trim()
  const repoRef = input.repoRef.trim()
  const objective = input.objective.trim()
  const controlPlaneBaseUrl = input.controlPlaneBaseUrl.trim()
  if (bearerToken.length === 0) {
    return { ok: false, code: "invalid_request", message: "bearerToken is required." }
  }
  if (repoRef.length === 0) {
    return { ok: false, code: "invalid_request", message: "repoRef is required." }
  }
  if (objective.length === 0) {
    return { ok: false, code: "invalid_request", message: "objective is required." }
  }
  if (!/^https:\/\//.test(controlPlaneBaseUrl)) {
    return {
      ok: false,
      code: "invalid_request",
      message: "controlPlaneBaseUrl must be an https URL.",
    }
  }

  const environment = makeOpenAgentsCloudHarnessEnvironment(controlPlaneBaseUrl)
  const client = makeCloudCodingSessionClient({
    launchUrl: openAgentsCloudCodingSessionLaunchUrl(environment),
    bearerToken,
    ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
  })

  const launched = await Effect.runPromise(
    client.launch({
      repoRef,
      objective,
      ...(input.adapter === undefined ? {} : { adapter: input.adapter }),
      ...(input.lane === undefined ? {} : { lane: input.lane }),
      ...(input.verify === undefined ? {} : { verify: [...input.verify] }),
    }),
  ).then(
    value => ({ ok: true as const, value }),
    error => ({
      ok: false as const,
      message: error instanceof Error ? error.message : "launch failed",
    }),
  )

  if (!launched.ok) {
    return { ok: false, code: "launch_failed", message: launched.message }
  }

  const startedAt = (input.now ?? (() => new Date))().toISOString()
  const session = store.put(
    projectRecord(launched.value, {
      controlPlaneBaseUrl,
      repoRef,
      objectiveDigest: digestObjective(objective),
      startedAt,
    }),
  )
  return { ok: true, session }
}

export type RefreshAgentComputerSessionInput = Readonly<{
  bearerToken: string
  session: AgentComputerSessionRecord
  fetch?: FetchLike
}>

export const refreshAgentComputerSession = async (
  store: AgentComputerSessionStore,
  input: RefreshAgentComputerSessionInput,
): Promise<StartAgentComputerSessionResult> => {
  const bearerToken = input.bearerToken.trim()
  if (bearerToken.length === 0) {
    return { ok: false, code: "invalid_request", message: "bearerToken is required." }
  }
  const environment = makeOpenAgentsCloudHarnessEnvironment(input.session.controlPlaneBaseUrl)
  const client = makeCloudCodingSessionClient({
    launchUrl: openAgentsCloudCodingSessionLaunchUrl(environment),
    bearerToken,
    ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
  })
  const refreshed = await Effect.runPromise(client.get(input.session.sessionRef)).then(
    value => ({ ok: true as const, value }),
    error => ({
      ok: false as const,
      message: error instanceof Error ? error.message : "refresh failed",
    }),
  )
  if (!refreshed.ok) {
    return { ok: false, code: "refresh_failed", message: refreshed.message }
  }
  const session = store.put(
    projectRecord(refreshed.value, {
      controlPlaneBaseUrl: input.session.controlPlaneBaseUrl,
      repoRef: input.session.repoRef,
      objectiveDigest: input.session.objectiveDigest,
      startedAt: input.session.startedAt,
    }),
  )
  return { ok: true, session }
}

export type RunAgentComputerTurnInput = Readonly<{
  bearerToken: string
  controlPlaneBaseUrl: string
  repoRef: string
  objective: string
  adapter?: CloudCodingAdapter
  lane?: CloudCodingLane
  verify?: ReadonlyArray<string>
  fetch?: FetchLike
  now?: () => Date
  pollIntervalMs?: number
  maxPollAttempts?: number
  sleep?: (durationMs: number) => Effect.Effect<void>
}>

export type RunAgentComputerTurnResult =
  | Readonly<{
      ok: true
      session: AgentComputerSessionRecord
      finishReason: string
      eventKinds: ReadonlyArray<string>
    }>
  | Readonly<{ ok: false; code: string; message: string }>

const CLOUD_SESSION_REF_RE = /cloud coding session ([^\s;]+)/i

const extractCloudSessionRef = (eventKinds: ReadonlyArray<{ kind: string; text?: string }>): string | null => {
  for (const event of eventKinds) {
    if (typeof event.text !== "string") continue
    const match = CLOUD_SESSION_REF_RE.exec(event.text)
    if (match?.[1]) return match[1]
  }
  return null
}

/**
 * Start and observe one Agent Computer turn through the released
 * `openagents_cloud` HarnessEnvironmentRunner. Persists only public-safe
 * projection fields.
 */
export const runAgentComputerTurn = async (
  store: AgentComputerSessionStore,
  input: RunAgentComputerTurnInput,
): Promise<RunAgentComputerTurnResult> => {
  const bearerToken = input.bearerToken.trim()
  const repoRef = input.repoRef.trim()
  const objective = input.objective.trim()
  const controlPlaneBaseUrl = input.controlPlaneBaseUrl.trim()
  if (bearerToken.length === 0) {
    return { ok: false, code: "invalid_request", message: "bearerToken is required." }
  }
  if (repoRef.length === 0) {
    return { ok: false, code: "invalid_request", message: "repoRef is required." }
  }
  if (objective.length === 0) {
    return { ok: false, code: "invalid_request", message: "objective is required." }
  }
  if (!/^https:\/\//.test(controlPlaneBaseUrl)) {
    return {
      ok: false,
      code: "invalid_request",
      message: "controlPlaneBaseUrl must be an https URL.",
    }
  }

  const environment = makeOpenAgentsCloudHarnessEnvironment(controlPlaneBaseUrl)
  const runner = makeOpenAgentsCloudHarnessEnvironmentRunner({
    bearerToken,
    repoRef,
    ...(input.adapter === undefined ? {} : { adapter: input.adapter }),
    ...(input.lane === undefined ? {} : { lane: input.lane }),
    ...(input.verify === undefined ? {} : { verify: [...input.verify] }),
    ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
    ...(input.pollIntervalMs === undefined ? {} : { pollIntervalMs: input.pollIntervalMs }),
    ...(input.maxPollAttempts === undefined ? {} : { maxPollAttempts: input.maxPollAttempts }),
    ...(input.sleep === undefined ? {} : { sleep: input.sleep }),
  })
  const harness = makeReferenceAdapter({ harnessId: "omega-effectd-agent-computer" })
  const startedAt = (input.now ?? (() => new Date))().toISOString()
  const localSessionId = `omega.ac.${createHash("sha256").update(`${repoRef}\0${startedAt}`).digest("hex").slice(0, 16)}`

  const turnOutcome = await Effect.runPromise(
    Effect.gen(function* () {
      const session = yield* startHarnessInEnvironment({
        environment,
        harness,
        options: {
          sessionId: localSessionId,
          source: {
            lane: "managed_cloud",
            adapterKind: "openagents_native",
            surface: "server",
          },
        },
        runners: { openagentsCloud: runner },
      })
      const turn = yield* session.promptTurn({
        turnId: `turn.${localSessionId}`,
        prompt: objective,
      })
      const events = yield* Stream.runCollect(turn.events)
      const done = yield* turn.done
      return { events: [...events], done }
    }),
  ).then(
    value => ({ ok: true as const, value }),
    error => {
      const record =
        typeof error === "object" && error !== null
          ? (error as Record<string, unknown>)
          : null
      const failureClass =
        typeof record?.failureClass === "string" ? record.failureClass : null
      const detail = typeof record?.detail === "string" ? record.detail : null
      const nested =
        typeof record?.error === "object" && record.error !== null
          ? (record.error as Record<string, unknown>)
          : null
      const nestedClass =
        typeof nested?.failureClass === "string" ? nested.failureClass : null
      const message =
        failureClass ??
        nestedClass ??
        (error instanceof Error && error.message.length > 0
          ? error.message
          : detail) ??
        (typeof error === "string" ? error : "agent computer turn failed")
      return { ok: false as const, message }
    },
  )

  if (!turnOutcome.ok) {
    return { ok: false, code: "turn_failed", message: turnOutcome.message }
  }

  const eventRows = turnOutcome.value.events.map(event => ({
    kind: String((event as { kind?: string }).kind ?? "unknown"),
    text:
      typeof (event as { text?: unknown }).text === "string"
        ? (event as { text: string }).text
        : undefined,
  }))
  const eventKinds = eventRows.map(row => row.kind)
  const cloudSessionRef = extractCloudSessionRef(eventRows)
  if (cloudSessionRef === null) {
    return {
      ok: false,
      code: "turn_failed",
      message: "Agent Computer turn finished without a public session ref.",
    }
  }

  const client = makeCloudCodingSessionClient({
    launchUrl: openAgentsCloudCodingSessionLaunchUrl(environment),
    bearerToken,
    ...(input.fetch === undefined ? {} : { fetch: input.fetch }),
  })
  const projection = await Effect.runPromise(client.get(cloudSessionRef)).then(
    value => ({ ok: true as const, value }),
    error => ({
      ok: false as const,
      message: error instanceof Error ? error.message : "session lookup failed",
    }),
  )
  if (!projection.ok) {
    return { ok: false, code: "refresh_failed", message: projection.message }
  }

  const session = store.put(
    projectRecord(projection.value, {
      controlPlaneBaseUrl,
      repoRef,
      objectiveDigest: digestObjective(objective),
      startedAt,
    }),
  )
  return {
    ok: true,
    session,
    finishReason: String(turnOutcome.value.done.finishReason),
    eventKinds,
  }
}
