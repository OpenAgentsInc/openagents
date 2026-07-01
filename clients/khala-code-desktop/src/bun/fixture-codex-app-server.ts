import { readFile } from "node:fs/promises"
import { createInterface } from "node:readline"

type JsonRpcId = number | string
type JsonObject = Record<string, unknown>

type FixtureScript = Readonly<{
  model: string
  modelProvider: string
  name: string
  schema: "khala-code-desktop.fixture-codex-app-server-script.v1"
  steps: readonly FixtureScriptStep[]
}>

type FixtureScriptStep =
  | Readonly<{
      kind: "notification"
      method: string
      params: JsonObject
    }>
  | Readonly<{
      kind: "serverRequest"
      method: string
      params: JsonObject
      requestId: string
    }>

type JsonRpcMessage = Readonly<{
  id?: JsonRpcId
  method?: string
  params?: unknown
  result?: unknown
}>

type ThreadRecord = {
  archived: boolean
  cwd: string
  id: string
  name: string | null
  status: string
  turns: TurnRecord[]
  updatedAt: string
}

type TurnRecord = {
  id: string
  items: JsonObject[]
  status: string
}

type ActiveTurn = {
  completed: boolean
  interrupted: boolean
  threadId: string
  turnId: string
}

type TemplateContext = Readonly<{
  approvalResponse?: unknown
  cwd: string
  threadId: string
  turnId: string
}>

const DEFAULT_SCRIPT_URL = new URL(
  "../../fixtures/codex-app-server/default-notification-script.json",
  import.meta.url,
)

const isoNow = (): string => new Date().toISOString()

const isObject = (value: unknown): value is JsonObject =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringField = (value: unknown, field: string): string | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return typeof candidate === "string" && candidate.length > 0 ? candidate : null
}

const numberField = (value: unknown, field: string): number | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return typeof candidate === "number" && Number.isFinite(candidate) ? candidate : null
}

const objectField = (value: unknown, field: string): JsonObject | null => {
  if (!isObject(value)) return null
  const candidate = value[field]
  return isObject(candidate) ? candidate : null
}

const arrayField = (value: unknown, field: string): readonly unknown[] => {
  if (!isObject(value)) return []
  const candidate = value[field]
  return Array.isArray(candidate) ? candidate : []
}

const cloneJson = <Value>(value: Value): Value =>
  JSON.parse(JSON.stringify(value)) as Value

const writeJsonLine = (value: unknown): void => {
  process.stdout.write(`${JSON.stringify(value)}\n`)
}

const writeErrorLine = (message: string): void => {
  process.stderr.write(`${JSON.stringify({
    level: "debug",
    target: "khala-code-fixture-codex-app-server",
    message,
  })}\n`)
}

const jsonRpcError = (
  id: JsonRpcId | undefined,
  code: number,
  message: string,
): void => {
  if (id === undefined) return
  writeJsonLine({ id, error: { code, message } })
}

const parseArgs = (
  argv: readonly string[],
  env: Readonly<Record<string, string | undefined>>,
): { readonly scriptPath: string | URL } => {
  let scriptPath: string | URL = env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE_SCRIPT ?? DEFAULT_SCRIPT_URL
  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index]
    if (arg === "--script") {
      const next = argv[index + 1]
      if (next === undefined || next.trim().length === 0) {
        throw new Error("--script requires a path")
      }
      scriptPath = next
      index += 1
    }
  }
  return { scriptPath }
}

const readFixtureScript = async (
  scriptPath: string | URL,
): Promise<FixtureScript> => {
  const parsed = JSON.parse(await readFile(scriptPath, "utf8")) as unknown
  if (!isObject(parsed) || parsed.schema !== "khala-code-desktop.fixture-codex-app-server-script.v1") {
    throw new Error("Fixture app-server script has an unsupported schema.")
  }
  const steps = arrayField(parsed, "steps")
  if (steps.length === 0) {
    throw new Error("Fixture app-server script must include at least one step.")
  }
  return parsed as FixtureScript
}

const applyTemplateString = (
  value: string,
  context: TemplateContext,
): string =>
  value
    .replaceAll("{{threadId}}", context.threadId)
    .replaceAll("{{turnId}}", context.turnId)
    .replaceAll("{{cwd}}", context.cwd)

const applyTemplate = (
  value: unknown,
  context: TemplateContext,
): unknown => {
  if (typeof value === "string") return applyTemplateString(value, context)
  if (Array.isArray(value)) return value.map(item => applyTemplate(item, context))
  if (!isObject(value)) return value
  return Object.fromEntries(
    Object.entries(value).map(([key, entry]) => [key, applyTemplate(entry, context)]),
  )
}

class FixtureCodexAppServer {
  private readonly activeTurns = new Map<string, ActiveTurn>()
  private readonly pendingServerResponses = new Map<JsonRpcId, (value: unknown) => void>()
  private readonly threads = new Map<string, ThreadRecord>()
  private nextThread = 0
  private nextTurn = 0

  constructor(
    private readonly script: FixtureScript,
    private readonly env: Readonly<Record<string, string | undefined>>,
  ) {}

  async handle(message: JsonRpcMessage): Promise<void> {
    if (message.method === undefined && message.id !== undefined) {
      this.resolveServerRequest(message.id, message.result ?? {})
      return
    }
    if (typeof message.method !== "string") {
      jsonRpcError(message.id, -32600, "Invalid JSON-RPC message.")
      return
    }
    if (message.id === undefined) {
      this.handleClientNotification(message.method)
      return
    }
    try {
      const result = await this.handleRequest(message.method, message.params ?? {})
      writeJsonLine({ id: message.id, result })
    } catch (error) {
      jsonRpcError(
        message.id,
        -32000,
        error instanceof Error ? error.message : String(error),
      )
    }
  }

  private handleClientNotification(method: string): void {
    if (method !== "initialized") {
      writeErrorLine(`ignored client notification: ${method}`)
    }
  }

  private async handleRequest(method: string, params: unknown): Promise<unknown> {
    switch (method) {
      case "initialize":
        return this.initializeResult()
      case "model/list":
        return this.modelListResult()
      case "permissionProfile/list":
        return this.permissionProfileListResult()
      case "config/value/write":
      case "config/mcpServer/reload":
        return { ok: true, fixture: true }
      case "thread/start":
        return this.threadStart(params)
      case "thread/resume":
        return this.threadResume(params)
      case "thread/list":
        return this.threadList(params)
      case "thread/read":
        return this.threadRead(params)
      case "thread/name/set":
        return this.threadNameSet(params)
      case "thread/archive":
        return this.threadArchive(params, true)
      case "thread/unarchive":
        return this.threadArchive(params, false)
      case "thread/delete":
        return this.threadDelete(params)
      case "thread/fork":
        return this.threadFork(params)
      case "thread/compact/start":
        return this.threadCompactStart(params)
      case "turn/start":
        return this.turnStart(params)
      case "turn/steer":
        return this.turnSteer(params)
      case "turn/interrupt":
        return this.turnInterrupt(params)
      default:
        throw new Error(`Fixture Codex app-server does not implement ${method}.`)
    }
  }

  private initializeResult(): JsonObject {
    return {
      protocolVersion: "khala-code-fixture-app-server-v1",
      serverInfo: {
        name: "khala-code-fixture-codex-app-server",
        version: "0.1.0",
      },
      userAgent: "khala-code-fixture-codex-app-server/0.1.0",
      codexHome: this.env.CODEX_HOME ?? null,
      platformFamily: process.platform === "win32" ? "windows" : "unix",
      platformOs: process.platform,
      fixtureScript: this.script.name,
    }
  }

  private modelListResult(): JsonObject {
    return {
      models: [{
        id: this.script.model,
        name: "Fixture Codex",
        provider: this.script.modelProvider,
      }],
    }
  }

  private permissionProfileListResult(): JsonObject {
    return {
      profiles: [{
        id: "fixture-read-write",
        name: "Fixture read/write",
        permissions: {
          fileSystem: {
            read: ["."],
            write: ["."],
          },
          network: { enabled: false },
        },
      }],
    }
  }

  private threadStart(params: unknown): JsonObject {
    const thread = this.createThread(stringField(params, "cwd") ?? process.cwd())
    return this.threadEnvelope(thread)
  }

  private threadResume(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    return this.threadEnvelope(thread)
  }

  private threadList(params: unknown): JsonObject {
    const archivedValue = isObject(params) ? params.archived : undefined
    const limit = numberField(params, "limit") ?? 50
    const archivedFilter = typeof archivedValue === "boolean" ? archivedValue : undefined
    const data = [...this.threads.values()]
      .filter(thread => archivedFilter === undefined || thread.archived === archivedFilter)
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
      .slice(0, limit)
      .map(thread => this.compactThread(thread))
    return {
      data,
      backwardsCursor: null,
      nextCursor: null,
    }
  }

  private threadRead(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    const includeTurns = isObject(params) && params.includeTurns === true
    return this.threadEnvelope(thread, includeTurns)
  }

  private threadNameSet(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    thread.name = stringField(params, "name") ?? thread.name
    thread.updatedAt = isoNow()
    return this.threadEnvelope(thread)
  }

  private threadArchive(params: unknown, archived: boolean): JsonObject {
    const thread = this.requireThread(params)
    thread.archived = archived
    thread.updatedAt = isoNow()
    return this.threadEnvelope(thread)
  }

  private threadDelete(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    this.threads.delete(thread.id)
    return { ok: true, threadId: thread.id }
  }

  private threadFork(params: unknown): JsonObject {
    const source = this.requireThread(params)
    const fork = this.createThread(stringField(params, "cwd") ?? source.cwd)
    fork.turns = source.turns.map(turn => ({
      id: `${turn.id}-fork`,
      items: cloneJson(turn.items),
      status: turn.status,
    }))
    fork.name = source.name === null ? null : `${source.name} (fixture fork)`
    return this.threadEnvelope(fork, true)
  }

  private threadCompactStart(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    const turn = this.createTurn(thread, "completed")
    turn.items.push({
      type: "contextCompaction",
      id: `item-compact-${turn.id}`,
    })
    return {
      turn: {
        id: turn.id,
        status: turn.status,
      },
    }
  }

  private turnStart(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    const turn = this.createTurn(thread, "inProgress")
    const input = arrayField(params, "input")
    const clientUserMessageId = stringField(params, "clientUserMessageId") ?? `user-${turn.id}`
    const userText = input
      .filter(isObject)
      .filter(item => item.type === "text")
      .map(item => typeof item.text === "string" ? item.text : "")
      .filter(Boolean)
      .join("\n\n")
    turn.items.push({
      type: "userMessage",
      id: clientUserMessageId,
      content: [{ type: "text", text: userText }],
    })
    const active: ActiveTurn = {
      completed: false,
      interrupted: false,
      threadId: thread.id,
      turnId: turn.id,
    }
    this.activeTurns.set(turn.id, active)
    queueMicrotask(() => {
      void this.runScript(thread, turn, active).catch(error => {
        writeErrorLine(error instanceof Error ? error.message : String(error))
      })
    })
    return {
      turn: {
        id: turn.id,
        status: turn.status,
      },
    }
  }

  private turnSteer(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    const expectedTurnId = stringField(params, "expectedTurnId")
    const active = expectedTurnId === null ? null : this.activeTurns.get(expectedTurnId)
    if (active === undefined || active === null) {
      throw new Error("No active fixture turn is available for steering.")
    }
    return {
      turnId: active.turnId,
      threadId: thread.id,
    }
  }

  private turnInterrupt(params: unknown): JsonObject {
    const thread = this.requireThread(params)
    const turnId = stringField(params, "turnId")
    if (turnId === null) throw new Error("turn/interrupt requires turnId.")
    const active = this.activeTurns.get(turnId)
    if (active === undefined) {
      throw new Error(`No active fixture turn ${turnId}.`)
    }
    active.interrupted = true
    active.completed = true
    const turn = thread.turns.find(candidate => candidate.id === turnId)
    if (turn !== undefined) turn.status = "interrupted"
    this.notify("turn/completed", {
      threadId: thread.id,
      turn: {
        id: turnId,
        status: "interrupted",
      },
    })
    return {
      ok: true,
      threadId: thread.id,
      turnId,
    }
  }

  private async runScript(
    thread: ThreadRecord,
    turn: TurnRecord,
    active: ActiveTurn,
  ): Promise<void> {
    let approvalResponse: unknown
    for (const step of this.script.steps) {
      if (active.completed || active.interrupted) return
      const context = {
        approvalResponse,
        cwd: thread.cwd,
        threadId: thread.id,
        turnId: turn.id,
      }
      if (step.kind === "notification") {
        const params = applyTemplate(step.params, context)
        this.recordCompletedItem(turn, step.method, params)
        this.notify(step.method, params)
        if (step.method === "turn/completed") {
          turn.status = completedTurnStatus(params)
          active.completed = true
          this.activeTurns.delete(turn.id)
        }
      } else {
        const requestId = applyTemplateString(step.requestId, context)
        const params = applyTemplate(step.params, context)
        approvalResponse = await this.requestClient(requestId, step.method, params)
        if (active.completed || active.interrupted) return
        this.notify("serverRequest/resolved", {
          requestId,
          threadId: thread.id,
          turnId: turn.id,
        })
      }
      await Promise.resolve()
    }
  }

  private requestClient(
    id: JsonRpcId,
    method: string,
    params: unknown,
  ): Promise<unknown> {
    writeJsonLine({ id, method, params })
    return new Promise(resolve => {
      this.pendingServerResponses.set(id, resolve)
    })
  }

  private resolveServerRequest(id: JsonRpcId, result: unknown): void {
    const resolve = this.pendingServerResponses.get(id)
    if (resolve === undefined) {
      writeErrorLine(`ignored response for unknown server request ${String(id)}`)
      return
    }
    this.pendingServerResponses.delete(id)
    resolve(result)
  }

  private notify(method: string, params: unknown): void {
    writeJsonLine({ method, params })
  }

  private createThread(cwd: string): ThreadRecord {
    this.nextThread += 1
    const thread: ThreadRecord = {
      archived: false,
      cwd,
      id: `fixture-thread-${this.nextThread}`,
      name: null,
      status: "running",
      turns: [],
      updatedAt: isoNow(),
    }
    this.threads.set(thread.id, thread)
    return thread
  }

  private createTurn(thread: ThreadRecord, status: string): TurnRecord {
    this.nextTurn += 1
    const turn: TurnRecord = {
      id: `fixture-turn-${this.nextTurn}`,
      items: [],
      status,
    }
    thread.turns.push(turn)
    thread.updatedAt = isoNow()
    return turn
  }

  private requireThread(params: unknown): ThreadRecord {
    const threadId = stringField(params, "threadId")
    if (threadId === null) throw new Error("Fixture app-server request requires threadId.")
    const thread = this.threads.get(threadId)
    if (thread === undefined) throw new Error(`Thread not found: ${threadId}`)
    return thread
  }

  private threadEnvelope(thread: ThreadRecord, includeTurns = true): JsonObject {
    return {
      cwd: thread.cwd,
      model: this.script.model,
      modelProvider: this.script.modelProvider,
      thread: this.fullThread(thread, includeTurns),
    }
  }

  private compactThread(thread: ThreadRecord): JsonObject {
    return {
      archived: thread.archived,
      cwd: thread.cwd,
      id: thread.id,
      name: thread.name,
      status: thread.status,
      updatedAt: thread.updatedAt,
    }
  }

  private fullThread(thread: ThreadRecord, includeTurns: boolean): JsonObject {
    return {
      ...this.compactThread(thread),
      ...(includeTurns
        ? {
            turns: thread.turns.map(turn => ({
              id: turn.id,
              status: turn.status,
              items: cloneJson(turn.items),
            })),
          }
        : {}),
    }
  }

  private recordCompletedItem(turn: TurnRecord, method: string, params: unknown): void {
    if (method !== "item/completed") return
    const item = objectField(params, "item")
    if (item === null) return
    const id = stringField(item, "id")
    if (id !== null) {
      const existingIndex = turn.items.findIndex(candidate => stringField(candidate, "id") === id)
      if (existingIndex !== -1) {
        turn.items.splice(existingIndex, 1, cloneJson(item))
        return
      }
    }
    turn.items.push(cloneJson(item))
  }
}

const completedTurnStatus = (params: unknown): string => {
  const turn = objectField(params, "turn")
  return stringField(turn, "status") ?? "completed"
}

export async function runFixtureCodexAppServer(
  input: {
    readonly argv?: readonly string[]
    readonly env?: Readonly<Record<string, string | undefined>>
  } = {},
): Promise<void> {
  const env = input.env ?? process.env
  const args = parseArgs(input.argv ?? Bun.argv.slice(2), env)
  const script = await readFixtureScript(args.scriptPath)
  const server = new FixtureCodexAppServer(script, env)
  const lines = createInterface({
    crlfDelay: Infinity,
    input: process.stdin,
  })

  for await (const line of lines) {
    const trimmed = line.trim()
    if (trimmed.length === 0) continue
    try {
      await server.handle(JSON.parse(trimmed) as JsonRpcMessage)
    } catch (error) {
      writeErrorLine(error instanceof Error ? error.message : String(error))
    }
  }
}

if (import.meta.main) {
  await runFixtureCodexAppServer()
}
