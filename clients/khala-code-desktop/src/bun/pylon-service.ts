import { existsSync } from "node:fs"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"

import { Context, Effect, Layer, Queue, Schema as S, Stream } from "effect"
import {
  decodePylonLifecycleWireEvent,
  decodePylonLifecycleWireEventJson,
  type PylonAssignmentRunLifecycleEvent,
  type PylonLifecycleWireEvent,
} from "@openagentsinc/agent-runtime-schema"

import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"
import {
  KhalaProcess,
  KhalaProcessLive,
  KhalaProcessNonZeroExit,
  makeKhalaProcessService,
  type KhalaProcessFailure,
  type KhalaProcessServiceShape,
} from "./khala-process.js"

type ChatEnv = Readonly<Record<string, string | undefined>>

type PylonPaths = {
  readonly appPath: string
  readonly bunExecutable: string
  readonly pylonHome: string
}

export type PylonServiceCommandInput = {
  readonly args: readonly string[]
  readonly env?: ChatEnv | undefined
  readonly maxOutputBytes?: number | undefined
  readonly timeoutMs?: number | undefined
}

export type PylonServiceCommandResult = {
  readonly exitCode: number | null
  readonly lifecycle: readonly PylonAssignmentRunLifecycleEvent[]
  readonly stderr: string
  readonly stdout: string
  readonly timedOut: boolean
}

export type PylonServiceAssignmentInput = {
  readonly accountRef?: string | undefined
  readonly baseUrl?: string | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly env?: ChatEnv | undefined
  readonly fixture?: boolean | undefined
  readonly objective: string
  readonly pylonRef?: string | undefined
  readonly repo?: string | undefined
  readonly timeoutMs?: number | undefined
  readonly verify?: string | undefined
  readonly workerKind?: "auto" | "claude" | "codex" | undefined
}

export type PylonServiceAssignmentResult = {
  readonly assignmentRef: string | null
  readonly lifecycle: readonly PylonAssignmentRunLifecycleEvent[]
  readonly status: "accepted" | "blocked" | "failed" | "completed"
  readonly summary: string
}

export class PylonServiceCommandFailure extends S.TaggedErrorClass<PylonServiceCommandFailure>()(
  "PylonServiceCommandFailure",
  {
    message: S.String,
  },
) {}

export class PylonServiceDecodeFailure extends S.TaggedErrorClass<PylonServiceDecodeFailure>()(
  "PylonServiceDecodeFailure",
  {
    boundary: S.Literals(["response", "lifecycle"]),
    message: S.String,
  },
) {}

export type PylonServiceFailure = PylonServiceCommandFailure | PylonServiceDecodeFailure

export type PylonServiceShape = {
  readonly lifecycle: Stream.Stream<PylonAssignmentRunLifecycleEvent, PylonServiceDecodeFailure>
  readonly request: (
    input: PylonServiceCommandInput,
  ) => Effect.Effect<PylonServiceCommandResult, PylonServiceFailure>
  readonly requestDecoded: <Result>(
    schema: S.Decoder<Result>,
    input: PylonServiceCommandInput,
  ) => Effect.Effect<Result, PylonServiceFailure>
  readonly runAssignment: (
    input: PylonServiceAssignmentInput,
  ) => Effect.Effect<PylonServiceAssignmentResult, PylonServiceFailure>
}

type PylonServiceOptions = {
  readonly env?: ChatEnv | undefined
  readonly process?: KhalaProcessServiceShape | undefined
  readonly runner?: PylonServiceCommandRunner | undefined
}

export type PylonServiceCommandRunnerInput = {
  readonly cmd: readonly string[]
  readonly cwd?: string | undefined
  readonly env?: ChatEnv | undefined
  readonly maxOutputBytes?: number | undefined
  readonly onStderrLine?: ((line: string) => void | Promise<void>) | undefined
  readonly timeoutMs: number
}

export type PylonServiceCommandRunnerResult = {
  readonly exitCode: number | null
  readonly signal?: NodeJS.Signals | null | undefined
  readonly stderr: string
  readonly stdout: string
  readonly timedOut: boolean
}

export type PylonServiceCommandRunner = (
  input: PylonServiceCommandRunnerInput,
) => Promise<PylonServiceCommandRunnerResult>

type PylonServiceStubOptions = {
  readonly assignments?: readonly PylonServiceAssignmentResult[] | undefined
  readonly requests?: readonly PylonServiceCommandResult[] | undefined
}

const DEFAULT_COMMAND_TIMEOUT_MS = 45_000
const DEFAULT_ASSIGNMENT_TIMEOUT_MS = 1_800_000
const DEFAULT_MAX_OUTPUT_BYTES = 80_000
const DEFAULT_ASSIGNMENT_MAX_OUTPUT_BYTES = 5_000_000
const DEFAULT_OPENAGENTS_BASE_URL = "https://openagents.com"

const errorMessage = (cause: unknown): string =>
  cause instanceof Error ? cause.message : String(cause)

const commandFailure = (cause: unknown): PylonServiceCommandFailure =>
  new PylonServiceCommandFailure({ message: errorMessage(cause) })

const decodeFailure = (
  boundary: "response" | "lifecycle",
  cause: unknown,
): PylonServiceDecodeFailure =>
  new PylonServiceDecodeFailure({ boundary, message: errorMessage(cause) })

const cleanEnv = (env: ChatEnv): NodeJS.ProcessEnv => {
  const clean: NodeJS.ProcessEnv = {}
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) clean[key] = value
  }
  return clean
}

const dedupe = (values: readonly string[]): readonly string[] =>
  [...new Set(values.map(value => resolve(value)))]

const pathCandidates = (env: ChatEnv): readonly string[] => [
  env.PATH ?? "",
  join(homedir(), ".bun", "bin"),
  "/opt/homebrew/bin",
  "/usr/local/bin",
  "/usr/bin",
  "/bin",
]

const pylonHomeCandidates = (env: ChatEnv): readonly string[] => {
  const home = homedir()
  return dedupe([
    ...(env.PYLON_HOME === undefined ? [] : [env.PYLON_HOME]),
    join(home, ".openagents", "pylon"),
    join(home, ".pylon"),
    ...(env.PYLON_FABLE_HOME === undefined ? [] : [env.PYLON_FABLE_HOME]),
    join(home, ".pylon-fable"),
  ])
}

export function resolvePylonServiceHome(env: ChatEnv = khalaCodeConfigFromRuntimeEnv().env): string {
  const explicit = env.PYLON_HOME?.trim()
  if (explicit !== undefined && explicit.length > 0) return resolve(explicit)
  const candidates = pylonHomeCandidates(env)
  const withState = candidates.find(candidate =>
    existsSync(join(candidate, "identity.json")) || existsSync(join(candidate, "config.json")),
  )
  return withState ?? candidates[0] ?? join(homedir(), ".openagents", "pylon")
}

const ancestorPylonCandidates = (anchor: string): readonly string[] => {
  const candidates: string[] = []
  let current = resolve(anchor)
  for (let index = 0; index < 12; index += 1) {
    candidates.push(resolve(current, "apps/pylon"))
    candidates.push(resolve(current, "../../apps/pylon"))
    const parent = dirname(current)
    if (parent === current) break
    current = parent
  }
  return candidates
}

const resolvePylonAppPath = (env: ChatEnv): string => {
  const candidates = dedupe([
    ...(env.OPENAGENTS_PYLON_APP_PATH === undefined ? [] : [env.OPENAGENTS_PYLON_APP_PATH]),
    ...(env.OPENAGENTS_REPO_ROOT === undefined ? [] : [resolve(env.OPENAGENTS_REPO_ROOT, "apps/pylon")]),
    ...(env.INIT_CWD === undefined ? [] : ancestorPylonCandidates(env.INIT_CWD)),
    ...(env.PWD === undefined ? [] : ancestorPylonCandidates(env.PWD)),
    ...ancestorPylonCandidates(process.cwd()),
    join(homedir(), "work", "openagents", "apps", "pylon"),
    resolve(process.cwd(), "../../apps/pylon"),
    resolve(process.cwd(), "apps/pylon"),
  ])
  return candidates.find(candidate => existsSync(join(candidate, "package.json"))) ??
    candidates[0] ??
    resolve(process.cwd(), "../../apps/pylon")
}

const resolveBunExecutable = (env: ChatEnv): string => {
  const candidates = [
    ...(env.OPENAGENTS_BUN_PATH === undefined ? [] : [env.OPENAGENTS_BUN_PATH]),
    process.execPath,
    join(homedir(), ".bun", "bin", "bun"),
    "/opt/homebrew/bin/bun",
    "/usr/local/bin/bun",
    "/usr/bin/bun",
  ]
  return candidates.find(candidate => candidate.length > 0 && existsSync(candidate)) ?? "bun"
}

const resolvePylonPaths = (env: ChatEnv): PylonPaths => ({
  appPath: resolvePylonAppPath(env),
  bunExecutable: resolveBunExecutable(env),
  pylonHome: resolvePylonServiceHome(env),
})

const configuredOpenAgentsBaseUrl = (env: ChatEnv): string | undefined =>
  [env.PYLON_OPENAGENTS_BASE_URL, env.OPENAGENTS_BASE_URL]
    .map(value => value?.trim())
    .find((value): value is string => value !== undefined && value.length > 0)

const pylonCommandEnv = (env: ChatEnv, pylonHome: string): ChatEnv => {
  const mergedEnv = { ...khalaCodeConfigFromRuntimeEnv().env, ...env }
  const configuredBaseUrl = configuredOpenAgentsBaseUrl(mergedEnv)
  return {
    ...mergedEnv,
    PATH: pathCandidates(mergedEnv).filter(path => path.length > 0).join(":"),
    ...(configuredBaseUrl === undefined ? {} : { PYLON_OPENAGENTS_BASE_URL: configuredBaseUrl }),
    PYLON_HOME: pylonHome,
  }
}

const commandEnvForArgs = (
  env: ChatEnv,
  paths: PylonPaths,
  args: readonly string[],
): ChatEnv => {
  const commandEnv = pylonCommandEnv(env, paths.pylonHome)
  if (!args.includes("--base-url")) return commandEnv
  return Object.fromEntries(
    Object.entries(commandEnv).filter(([key]) =>
      key !== "PYLON_OPENAGENTS_BASE_URL" && key !== "OPENAGENTS_BASE_URL"
    ),
  )
}

const tailByBytes = (text: string, maxBytes: number): string => {
  const bytes = Buffer.from(text, "utf8")
  if (bytes.byteLength <= maxBytes) return text
  return bytes.subarray(Math.max(0, bytes.byteLength - maxBytes)).toString("utf8")
}

const appendLineByByteLimit = (text: string, line: string, maxBytes: number): string => {
  const next = `${text}${line}\n`
  if (Buffer.byteLength(next, "utf8") <= maxBytes) return next
  const lines = next.split("\n")
  if (lines.at(-1) === "") lines.pop()
  while (lines.length > 0 && Buffer.byteLength(`${lines.join("\n")}\n`, "utf8") > maxBytes) {
    lines.shift()
  }
  return lines.length === 0 ? "" : `${lines.join("\n")}\n`
}

const assignmentLifecycleEventFromLine = (
  line: string,
): PylonAssignmentRunLifecycleEvent | null => {
  const trimmed = line.trim()
  if (!trimmed.startsWith("{") || !trimmed.endsWith("}")) return null
  try {
    const event = decodePylonLifecycleWireEventJson(trimmed)
    return event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1"
      ? event
      : null
  } catch {
    return null
  }
}

const assignmentLifecycleEventsFromUnknown = (
  value: unknown,
): readonly PylonAssignmentRunLifecycleEvent[] => {
  if (!Array.isArray(value)) return []
  return value.flatMap(item => {
    try {
      const event: PylonLifecycleWireEvent = decodePylonLifecycleWireEvent(item)
      return event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1"
        ? [event]
        : []
    } catch {
      return []
    }
  })
}

const collectText = (
  stream: Stream.Stream<Uint8Array, KhalaProcessFailure>,
  input: {
    readonly maxOutputBytes: number
    readonly onLine?: ((line: string) => Effect.Effect<void>) | undefined
  },
): Effect.Effect<string> =>
  Effect.gen(function* () {
    let text = ""
    yield* stream.pipe(
      Stream.decodeText(),
      Stream.splitLines,
      Stream.runForEach(line =>
        Effect.gen(function* () {
          text = appendLineByByteLimit(text, line, input.maxOutputBytes)
          if (input.onLine !== undefined) yield* input.onLine(line)
        })
      ),
      Effect.catch(() => Effect.void),
    )
    return text
  })

const parseJsonObject = (value: string): Record<string, unknown> | null => {
  try {
    const parsed: unknown = JSON.parse(value)
    return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)
      ? parsed as Record<string, unknown>
      : null
  } catch {
    return null
  }
}

const stringField = (source: Record<string, unknown> | null, field: string): string | null => {
  const value = source?.[field]
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null
}

const booleanField = (source: Record<string, unknown> | null, field: string): boolean | null => {
  const value = source?.[field]
  return typeof value === "boolean" ? value : null
}

const recordField = (source: Record<string, unknown> | null, field: string): Record<string, unknown> | null => {
  const value = source?.[field]
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
}

const stringArrayField = (source: Record<string, unknown> | null, field: string): readonly string[] => {
  const value = source?.[field]
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string" && item.trim().length > 0)
    : []
}

const dedupeStrings = (values: readonly string[]): readonly string[] => [...new Set(values)]

const lifecycleSummaryLines = (
  events: readonly PylonAssignmentRunLifecycleEvent[],
): readonly string[] => {
  if (events.length === 0) return []
  return [
    "lifecycle:",
    ...events.slice(-8).map(event => {
      const details = [
        event.phase === undefined ? null : `phase=${event.phase}`,
        event.status === undefined ? null : `status=${event.status}`,
      ].filter((value): value is string => value !== null)
      return details.length === 0
        ? `  - ${event.event}`
        : `  - ${event.event} (${details.join(", ")})`
    }),
  ]
}

const safeFailureSummary = (result: PylonServiceCommandResult): string => {
  const nonLifecycle = `${result.stdout}\n${result.stderr}`
    .split(/\r?\n/u)
    .filter(line => assignmentLifecycleEventFromLine(line) === null)
    .map(line => line.replace(/\s+/gu, " ").trimEnd())
    .filter(line => line.trim().length > 0)
    .join("\n")
  const headline = result.timedOut
    ? "command timed out"
    : nonLifecycle.length === 0
      ? `command exited ${result.exitCode ?? "without status"}`
      : nonLifecycle
  return tailByBytes([headline, ...lifecycleSummaryLines(result.lifecycle)].join("\n"), 4_000)
}

const staleHeartbeatAdmissionPattern =
  /stale_or_missing_heartbeat|online heartbeat is stale or missing|stale heartbeat|presence\.stale_heartbeat/iu

const isStaleHeartbeatAdmissionFailure = (result: PylonServiceCommandResult): boolean =>
  stringField(parseJsonObject(result.stdout), "assignmentRef") === null &&
  staleHeartbeatAdmissionPattern.test(`${result.stdout}\n${result.stderr}`)

const heartbeatLooksFresh = (result: PylonServiceCommandResult): boolean => {
  if (result.exitCode !== 0) return false
  const payload = parseJsonObject(result.stdout)
  return booleanField(payload, "linked") !== false && booleanField(payload, "stale") !== true
}

const resolveOpenAgentsBaseUrl = (env: ChatEnv, explicit?: string | undefined): string =>
  (
    [explicit, env.PYLON_OPENAGENTS_BASE_URL, env.OPENAGENTS_BASE_URL, DEFAULT_OPENAGENTS_BASE_URL]
      .map(value => value?.trim())
      .find((value): value is string => value !== undefined && value.length > 0) ?? DEFAULT_OPENAGENTS_BASE_URL
  ).replace(/\/+$/u, "")

const workflowForWorkerKind = (
  workerKind: PylonServiceAssignmentInput["workerKind"],
): "claude_agent_task" | "codex_agent_task" =>
  workerKind === "claude" ? "claude_agent_task" : "codex_agent_task"

const assignmentArgs = (
  input: PylonServiceAssignmentInput,
  env: ChatEnv,
): readonly string[] => {
  const fixture = input.fixture === true
  return [
    "khala",
    "request",
    "--workflow",
    workflowForWorkerKind(input.workerKind),
    "--prompt",
    input.objective,
    ...(input.pylonRef === undefined ? [] : ["--pylon-ref", input.pylonRef]),
    ...(input.accountRef === undefined ? [] : ["--account-ref", input.accountRef]),
    ...(fixture
      ? ["--fixture"]
      : [
          ...(input.repo === undefined ? [] : ["--repo", input.repo]),
          ...(input.branch === undefined ? [] : ["--branch", input.branch]),
          ...(input.commit === undefined ? [] : ["--commit", input.commit]),
          ...(input.verify === undefined ? [] : ["--verify", input.verify]),
        ]),
    "--base-url",
    resolveOpenAgentsBaseUrl(env, input.baseUrl),
    "--lifecycle-ndjson",
    "--json",
  ]
}

const assignmentResultFromCommand = (
  command: PylonServiceCommandResult,
): PylonServiceAssignmentResult => {
  const payload = parseJsonObject(command.stdout)
  const assignmentRef = stringField(payload, "assignmentRef")
  const autoRun = recordField(payload, "autoRun")
  const assignmentRun = recordField(payload, "assignmentRun")
  const closeout = recordField(assignmentRun, "closeout")
  const autoRunOk = booleanField(autoRun, "ok")
  const runOk = booleanField(assignmentRun, "ok")
  const closeoutStatus = stringField(closeout, "status") ?? stringField(payload, "closeoutStatus")
  const payloadLifecycle = assignmentLifecycleEventsFromUnknown(payload?.assignmentLifecycleEvents)
  const lifecycle = command.lifecycle.length > 0 ? command.lifecycle : payloadLifecycle
  const assignmentAccepted = command.exitCode === 0 && assignmentRef !== null
  const completed = assignmentAccepted && (autoRunOk === true || runOk === true || closeoutStatus === "accepted")
  const assignmentFailed = assignmentAccepted &&
    (autoRunOk === false || runOk === false || closeoutStatus === "rejected")
  const status: PylonServiceAssignmentResult["status"] =
    completed ? "completed" : assignmentFailed ? "failed" : assignmentAccepted ? "accepted" : "failed"
  const closeoutChecklist = recordField(closeout, "closeoutChecklist")
  const proof = recordField(closeout, "proof")
  const proofChecklist = recordField(proof, "proofChecklist")
  const blockerRefs = dedupeStrings([
    ...stringArrayField(closeout, "blockerRefs"),
    ...stringArrayField(closeoutChecklist, "blockerRefs"),
    ...stringArrayField(proofChecklist, "blockerRefs"),
    ...lifecycle.flatMap(event => event.blockerRefs ?? []),
  ])
  const summary = assignmentAccepted
    ? [
        `assignment: ${assignmentRef}`,
        autoRunOk === null ? "auto-run: unknown" : `auto-run: ${autoRunOk ? "completed" : "failed"}`,
        runOk === null ? null : `assignment run: ${runOk ? "completed" : "failed"}`,
        closeoutStatus === null ? null : `closeout: ${closeoutStatus}`,
        blockerRefs.length === 0 ? null : `blocker refs: ${blockerRefs.join(", ")}`,
        ...lifecycleSummaryLines(lifecycle),
        "next: summarize this status; no local output path was returned",
      ].filter((line): line is string => line !== null).join("\n")
    : safeFailureSummary(command)
  return {
    assignmentRef,
    lifecycle,
    status,
    summary,
  }
}

const makeLifecycleStream = (
  subscribers: Set<(event: PylonAssignmentRunLifecycleEvent) => void>,
): Stream.Stream<PylonAssignmentRunLifecycleEvent, PylonServiceDecodeFailure> =>
  Stream.unwrap(
    Effect.map(Queue.unbounded<PylonAssignmentRunLifecycleEvent>(), queue => {
      const subscriber = (event: PylonAssignmentRunLifecycleEvent): void => {
        Queue.offerUnsafe(queue, event)
      }
      subscribers.add(subscriber)
      return Stream.fromQueue(queue).pipe(
        Stream.ensuring(Effect.all([
          Effect.sync(() => subscribers.delete(subscriber)),
          Queue.shutdown(queue),
        ], { discard: true })),
      )
    }),
  )

const makeRequest = (
  process: KhalaProcessServiceShape,
  options: PylonServiceOptions,
  subscribers: Set<(event: PylonAssignmentRunLifecycleEvent) => void>,
): PylonServiceShape["request"] =>
  input => {
    if (options.runner !== undefined) {
      return Effect.tryPromise({
        try: async () => {
          const env = { ...(options.env ?? khalaCodeConfigFromRuntimeEnv().env), ...(input.env ?? {}) }
          const paths = resolvePylonPaths(env)
          const lifecycleEvents: PylonAssignmentRunLifecycleEvent[] = []
          const publishLine = async (line: string): Promise<void> => {
            const event = assignmentLifecycleEventFromLine(line)
            if (event === null) return
            lifecycleEvents.push(event)
            for (const subscriber of subscribers) subscriber(event)
          }
          const result = await options.runner?.({
            cmd: [paths.bunExecutable, "src/index.ts", ...input.args],
            cwd: paths.appPath,
            env: commandEnvForArgs(env, paths, input.args),
            maxOutputBytes: input.maxOutputBytes ?? DEFAULT_MAX_OUTPUT_BYTES,
            onStderrLine: publishLine,
            timeoutMs: Math.max(1, Math.trunc(input.timeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS)),
          })
          if (result === undefined) throw new Error("Pylon service runner is unavailable")
          return {
            exitCode: result.exitCode,
            lifecycle: lifecycleEvents,
            stderr: result.stderr,
            stdout: result.stdout,
            timedOut: result.timedOut,
          }
        },
        catch: commandFailure,
      })
    }
    return (
    Effect.scoped(
      Effect.gen(function* () {
        const env = { ...(options.env ?? khalaCodeConfigFromRuntimeEnv().env), ...(input.env ?? {}) }
        const paths = resolvePylonPaths(env)
        const args = ["src/index.ts", ...input.args]
        const timeoutMs = Math.max(1, Math.trunc(input.timeoutMs ?? DEFAULT_COMMAND_TIMEOUT_MS))
        const maxOutputBytes = Math.max(1, Math.trunc(input.maxOutputBytes ?? DEFAULT_MAX_OUTPUT_BYTES))
        const lifecycleEvents: PylonAssignmentRunLifecycleEvent[] = []
        let timedOut = false
        const handle = yield* process.spawn(paths.bunExecutable, args, {
          cwd: paths.appPath,
          env: cleanEnv(commandEnvForArgs(env, paths, input.args)),
          extendEnv: false,
          forceKillAfter: "1500 millis",
        }).pipe(Effect.mapError(commandFailure))
        yield* Effect.forkScoped(
          Effect.sleep(`${timeoutMs} millis`).pipe(
            Effect.tap(() => Effect.sync(() => {
              timedOut = true
            })),
            Effect.flatMap(() => handle.kill({ forceKillAfter: "1500 millis" }).pipe(Effect.ignore)),
          ),
        )
        const lineSink = (line: string): Effect.Effect<void> =>
          Effect.sync(() => {
            const event = assignmentLifecycleEventFromLine(line)
            if (event === null) return
            lifecycleEvents.push(event)
            for (const subscriber of subscribers) subscriber(event)
          })
        const exitCode = handle.exit.pipe(
          Effect.match({
            onFailure: failure =>
              failure instanceof KhalaProcessNonZeroExit ? failure.exitCode : 127,
            onSuccess: code => code,
          }),
        )
        const output = yield* Effect.all({
          exitCode,
          stderr: collectText(handle.stderr, { maxOutputBytes, onLine: lineSink }),
          stdout: collectText(handle.stdout, { maxOutputBytes }),
        }, { concurrency: "unbounded" }).pipe(
          Effect.mapError(commandFailure),
        )
        return {
          ...output,
          lifecycle: lifecycleEvents,
          timedOut,
        }
      }),
    )
    )
  }

export const makePylonService = (
  options: PylonServiceOptions = {},
): PylonServiceShape => {
  const process = options.process ?? makeKhalaProcessService()
  const subscribers = new Set<(event: PylonAssignmentRunLifecycleEvent) => void>()
  const request = makeRequest(process, options, subscribers)
  return {
    lifecycle: makeLifecycleStream(subscribers),
    request,
    requestDecoded: (schema, input) =>
      request(input).pipe(
        Effect.flatMap(result =>
          S.decodeUnknownEffect(schema)(parseJsonObject(result.stdout)).pipe(
            Effect.mapError(cause => decodeFailure("response", cause)),
          )
        ),
      ),
    runAssignment: input => Effect.gen(function* () {
      const env = { ...(options.env ?? khalaCodeConfigFromRuntimeEnv().env), ...(input.env ?? {}) }
      const args = assignmentArgs(input, env)
      const commandInput = {
        args,
        env,
        maxOutputBytes: DEFAULT_ASSIGNMENT_MAX_OUTPUT_BYTES,
        timeoutMs: input.timeoutMs ?? DEFAULT_ASSIGNMENT_TIMEOUT_MS,
      }
      let result = yield* request(commandInput)
      for (let attempt = 0; attempt < 2 && isStaleHeartbeatAdmissionFailure(result); attempt += 1) {
        const heartbeat = yield* request({
          args: ["presence", "heartbeat", "--base-url", resolveOpenAgentsBaseUrl(env, input.baseUrl), "--json"],
          env,
          maxOutputBytes: DEFAULT_MAX_OUTPUT_BYTES,
          timeoutMs: DEFAULT_COMMAND_TIMEOUT_MS,
        })
        if (!heartbeatLooksFresh(heartbeat)) break
        result = yield* request(commandInput)
      }
      return assignmentResultFromCommand(result)
    }),
  }
}

export const makePylonServiceLayer = (
  options: Omit<PylonServiceOptions, "process"> = {},
): Layer.Layer<PylonService, never, KhalaProcess> =>
  Layer.effect(
    PylonService,
    Effect.map(KhalaProcess, process => makePylonService({ ...options, process })),
  )

export const makePylonServiceStub = (
  options: PylonServiceStubOptions = {},
): PylonServiceShape => {
  const subscribers = new Set<(event: PylonAssignmentRunLifecycleEvent) => void>()
  const assignments = [...(options.assignments ?? [])]
  const requests = [...(options.requests ?? [])]
  const publish = (events: readonly PylonAssignmentRunLifecycleEvent[]): void => {
    for (const event of events) {
      for (const subscriber of subscribers) subscriber(event)
    }
  }
  const defaultAssignment = (input: PylonServiceAssignmentInput): PylonServiceAssignmentResult => {
    const observedAt = new Date(0).toISOString()
    const assignmentRef = `assignment.fixture.${input.workerKind ?? "codex"}`
    return {
      assignmentRef,
      lifecycle: [{
        schema: "openagents.pylon.assignment_run_lifecycle_event.v0.1",
        assignmentRef,
        event: "assignment_run.completed",
        observedAt,
        status: "closeout-submitted",
      }],
      status: "completed",
      summary: `assignment: ${assignmentRef}`,
    }
  }
  const request: PylonServiceShape["request"] = () =>
    Effect.sync(() => {
      const result = requests.shift() ?? {
        exitCode: 0,
        lifecycle: [],
        stderr: "",
        stdout: "{}",
        timedOut: false,
      }
      publish(result.lifecycle)
      return result
    })
  return {
    lifecycle: makeLifecycleStream(subscribers),
    request,
    requestDecoded: (schema, input) =>
      request(input).pipe(
        Effect.flatMap(result =>
          S.decodeUnknownEffect(schema)(parseJsonObject(result.stdout)).pipe(
            Effect.mapError(cause => decodeFailure("response", cause)),
          )
        ),
      ),
    runAssignment: input =>
      Effect.sync(() => {
        const result = assignments.shift() ?? defaultAssignment(input)
        publish(result.lifecycle)
        return result
      }),
  }
}

export const PylonServiceStub = (
  options: PylonServiceStubOptions = {},
): Layer.Layer<PylonService> =>
  Layer.succeed(PylonService, makePylonServiceStub(options))

export class PylonService extends Context.Service<PylonService, PylonServiceShape>()(
  "PylonService",
  { make: Effect.sync(makePylonService) },
) {}

export const PylonServiceLive = makePylonServiceLayer().pipe(Layer.provide(KhalaProcessLive))
