import { Effect, Schema as S } from "effect"
import { createMacosSeatbeltKhalaProcessService } from "./process-sandbox-macos.js"
import { redactKhalaPublicText } from "./redaction.js"

export {
  KhalaPrivacyRedactionLive,
  KhalaPrivacyRedactionService,
  makeKhalaPrivacyRedactionService,
  redactKhalaPublicText,
  type KhalaPrivacyRedactionMode,
  type KhalaPrivacyRedactionResult,
  type KhalaPrivacyRedactionServiceOptions,
  type KhalaPrivacyRedactionServiceShape,
  type KhalaRampartGuard,
  type KhalaRampartGuardFactory,
} from "./redaction.js"

export const KhalaToolAuthority = S.Literals([
  "read",
  "search",
  "edit",
  "write",
  "patch",
  "shell",
  "process_stdin",
  "interaction",
  "session_state",
  "network",
  "browser",
  "external_directory",
  "memory_write",
  "credential",
  "persistent_config_write",
  "owner_full_access",
])
export type KhalaToolAuthority = typeof KhalaToolAuthority.Type

export const KhalaToolAvailability = S.Literals(["inspect", "coding", "owner_local_full", "network", "browser", "extension"])
export type KhalaToolAvailability = typeof KhalaToolAvailability.Type

export const KhalaPermissionMode = S.Literals(["allow", "approval_required", "deny"])
export type KhalaPermissionMode = typeof KhalaPermissionMode.Type

export const KhalaToolExecutionMode = S.Literals(["local", "hosted", "delegated"])
export type KhalaToolExecutionMode = typeof KhalaToolExecutionMode.Type

export const KhalaJsonSchema = S.Record(S.String, S.Unknown)
export type KhalaJsonSchema = typeof KhalaJsonSchema.Type

export const KhalaRendererMetadata = S.Struct({
  kind: S.String,
  rendererRef: S.optional(S.String),
})
export type KhalaRendererMetadata = typeof KhalaRendererMetadata.Type

export const KhalaToolDefinition = S.Struct({
  authority: KhalaToolAuthority,
  availability: S.Array(KhalaToolAvailability),
  description: S.String,
  executionMode: KhalaToolExecutionMode,
  inputSchema: KhalaJsonSchema,
  internalId: S.String,
  label: S.String,
  name: S.String,
  outputSchema: S.optional(KhalaJsonSchema),
  permissionMode: KhalaPermissionMode,
  prompt: S.String,
  promptGuidelines: S.Array(S.String),
  renderer: S.optional(KhalaRendererMetadata),
})
export type KhalaToolDefinition = typeof KhalaToolDefinition.Type

export const KhalaToolInvocation = S.Struct({
  arguments: S.Record(S.String, S.Unknown),
  id: S.String,
  name: S.String,
  sessionId: S.String,
})
export type KhalaToolInvocation = typeof KhalaToolInvocation.Type

export const KhalaPublicSafety = S.Literals(["public_safe", "private", "redacted"])
export type KhalaPublicSafety = typeof KhalaPublicSafety.Type

export const KhalaToolArtifact = S.Struct({
  artifactRef: S.String,
  mediaType: S.optional(S.String),
  private: S.Boolean,
  summary: S.optional(S.String),
})
export type KhalaToolArtifact = typeof KhalaToolArtifact.Type

export const KhalaToolResultStatus = S.Literals(["ok", "failed", "denied", "needs_input", "unavailable"])
export type KhalaToolResultStatus = typeof KhalaToolResultStatus.Type

export const KhalaToolResult = S.Struct({
  artifacts: S.Array(KhalaToolArtifact),
  modelOutput: S.Struct({
    text: S.String,
  }),
  privateDataRefs: S.Array(S.String),
  publicSafety: KhalaPublicSafety,
  publicSummary: S.String,
  redactionRefs: S.Array(S.String),
  status: KhalaToolResultStatus,
  ui: S.Unknown,
})
export type KhalaToolResult = typeof KhalaToolResult.Type

export const KhalaToolEventKind = S.Literals([
  "model_content",
  "tool_requested",
  "approval_requested",
  "approval_answered",
  "tool_started",
  "tool_progress",
  "stdout_chunk",
  "stderr_chunk",
  "stdin_chunk",
  "diff_chunk",
  "artifact_written",
  "user_input_requested",
  "user_input_answered",
  "user_input_unavailable",
  "user_input_timed_out",
  "todo_list_updated",
  "tool_completed",
  "tool_failed",
  "tool_cancelled",
])
export type KhalaToolEventKind = typeof KhalaToolEventKind.Type

export const KhalaToolEvent = S.Struct({
  eventId: S.String,
  invocationId: S.optional(S.String),
  kind: KhalaToolEventKind,
  payload: S.Unknown,
  sessionId: S.String,
})
export type KhalaToolEvent = typeof KhalaToolEvent.Type

export const KhalaPermissionDecision = S.Literals(["allow", "deny", "always"])
export type KhalaPermissionDecision = typeof KhalaPermissionDecision.Type

export const KhalaPermissionRequest = S.Struct({
  action: KhalaToolAuthority,
  authorityMode: S.String,
  publicSafety: KhalaPublicSafety,
  resources: S.Array(S.String),
  saveScope: S.Literals(["once", "session", "project"]),
  sessionId: S.String,
  toolCallId: S.String,
  toolName: S.String,
  workingDirectory: S.optional(S.String),
})
export type KhalaPermissionRequest = typeof KhalaPermissionRequest.Type

export interface KhalaPermissionService {
  readonly decide: (request: KhalaPermissionRequest) => Effect.Effect<KhalaPermissionDecision, never>
}

export interface KhalaWorkspaceService {
  readonly workingDirectory: string
  readonly resolvePath?: (path: string) => Effect.Effect<string, KhalaToolRuntimeError>
}

export type KhalaProcessOutputChannel = "stdin" | "stdout" | "stderr"

export type KhalaProcessEvent = Readonly<{
  channel: KhalaProcessOutputChannel
  text: string
  timestampMs: number
}>

export type KhalaProcessExecInput = Readonly<{
  argv?: ReadonlyArray<string>
  cancelAfterMs?: number
  command: string
  cwd: string
  maxCaptureBytes: number
  shell?: string
  timeoutMs: number
  workspaceRoot?: string
}>

export type KhalaProcessSessionStartInput = KhalaProcessExecInput & Readonly<{
  khalaSessionId: string
  yieldTimeMs: number
}>

export type KhalaProcessStdinInput = Readonly<{
  chars?: string
  khalaSessionId: string
  maxCaptureBytes: number
  sessionId: string
  yieldTimeMs: number
}>

export type KhalaProcessExecResult = Readonly<{
  cancelled: boolean
  durationMs: number
  events: ReadonlyArray<KhalaProcessEvent>
  exitCode: number | null
  sandbox: Readonly<{
    enforced: boolean
    kind: "none" | "external"
    note: string
  }>
  signal: string | null
  stderr: string
  stderrTruncated: boolean
  stdout: string
  stdoutTruncated: boolean
  timedOut: boolean
}>

export type KhalaProcessSessionResult = KhalaProcessExecResult & Readonly<{
  sessionId: string
}>

export interface KhalaProcessService {
  readonly execCommand: (input: KhalaProcessExecInput) => Effect.Effect<KhalaProcessExecResult, KhalaToolRuntimeError>
  readonly startSession: (input: KhalaProcessSessionStartInput) => Effect.Effect<KhalaProcessSessionResult, KhalaToolRuntimeError>
  readonly writeStdin: (input: KhalaProcessStdinInput) => Effect.Effect<KhalaProcessSessionResult, KhalaToolRuntimeError>
  readonly marker: "khala.process_service"
}

export interface KhalaOutputStore {
  readonly writeArtifact: (input: {
    readonly bytes: Uint8Array
    readonly mediaType?: string
    readonly summary?: string
  }) => Effect.Effect<KhalaToolArtifact, KhalaToolRuntimeError>
}

export type KhalaInteractionChoice = Readonly<{
  description?: string
  id: string
  label: string
}>

export type KhalaInteractionAnswer =
  | Readonly<{
    choiceId: string
    kind: "choice"
    text: string
  }>
  | Readonly<{
    kind: "freeform"
    text: string
  }>
  | Readonly<{
    kind: "default"
    text: string
  }>

export type KhalaInteractionAskStatus = "answered" | "pending" | "timed_out" | "unavailable"

export type KhalaInteractionEventKind =
  | "user_input_requested"
  | "user_input_answered"
  | "user_input_unavailable"
  | "user_input_timed_out"

export type KhalaInteractionEvent = Readonly<{
  kind: KhalaInteractionEventKind
  payload: unknown
  timestampMs: number
}>

export type KhalaInteractionAskInput = Readonly<{
  allowFreeform: boolean
  choices: ReadonlyArray<KhalaInteractionChoice>
  defaultAnswer?: string
  invocationId: string
  khalaSessionId: string
  nonBlocking: boolean
  prompt: string
  publicSafe: boolean
  timeoutMs?: number
}>

export type KhalaInteractionAskResult = Readonly<{
  answer?: KhalaInteractionAnswer
  events: ReadonlyArray<KhalaInteractionEvent>
  reason?: string
  requestId: string
  status: KhalaInteractionAskStatus
}>

export interface KhalaInteractionService {
  readonly askUser: (input: KhalaInteractionAskInput) => Effect.Effect<KhalaInteractionAskResult, KhalaToolRuntimeError>
  readonly marker: "khala.interaction_service"
}

export type KhalaTodoStatus = "pending" | "in_progress" | "blocked" | "completed" | "cancelled"

export type KhalaTodoItemInput = Readonly<{
  blockerReason?: string
  content: string
  id: string
  status: KhalaTodoStatus
}>

export type KhalaTodoItem = KhalaTodoItemInput & Readonly<{
  order: number
  updatedAtMs: number
}>

export type KhalaTodoEvent = Readonly<{
  kind: "todo_list_updated"
  payload: unknown
  timestampMs: number
}>

export type KhalaTodoWriteInput = Readonly<{
  invocationId: string
  khalaSessionId: string
  todos: ReadonlyArray<KhalaTodoItemInput>
}>

export type KhalaTodoWriteResult = Readonly<{
  events: ReadonlyArray<KhalaTodoEvent>
  revision: number
  sessionId: string
  todos: ReadonlyArray<KhalaTodoItem>
}>

export interface KhalaTodoService {
  readonly marker: "khala.todo_service"
  readonly writeTodos: (input: KhalaTodoWriteInput) => Effect.Effect<KhalaTodoWriteResult, KhalaToolRuntimeError>
}

export type KhalaNetworkFetchInput = Readonly<{
  maxBytes: number
  maxRedirects: number
  timeoutMs: number
  url: string
}>

export type KhalaNetworkRedirect = Readonly<{
  from: string
  status: number
  to: string
}>

export type KhalaNetworkFetchResult = Readonly<{
  body: Uint8Array
  bodyTruncated: boolean
  contentType: string
  fetchedAtMs: number
  finalUrl: string
  redirectChain: ReadonlyArray<KhalaNetworkRedirect>
  status: number
  statusText: string
  url: string
}>

export interface KhalaNetworkService {
  readonly fetchUrl: (input: KhalaNetworkFetchInput) => Effect.Effect<KhalaNetworkFetchResult, KhalaToolRuntimeError>
  readonly marker: "khala.network_service"
}

export type KhalaWebSearchInput = Readonly<{
  domains: ReadonlyArray<string>
  limit: number
  query: string
  recencyDays?: number
}>

export type KhalaWebSearchItem = Readonly<{
  publishedAt?: string
  snippet: string
  title: string
  url: string
}>

export type KhalaWebSearchResult = Readonly<{
  provider: string
  results: ReadonlyArray<KhalaWebSearchItem>
  searchedAtMs: number
}>

export interface KhalaWebSearchService {
  readonly marker: "khala.web_search_service"
  readonly search: (input: KhalaWebSearchInput) => Effect.Effect<KhalaWebSearchResult, KhalaToolRuntimeError>
}

export type KhalaBrowserActionInput = Readonly<{
  label?: string
  selector: string
  timeoutMs: number
}>

export type KhalaBrowserTypeInput = KhalaBrowserActionInput & Readonly<{
  text: string
}>

export type KhalaBrowserNavigateInput = Readonly<{
  timeoutMs: number
  url: string
}>

export type KhalaBrowserPageSnapshot = Readonly<{
  timestampMs: number
  title?: string
  url: string
}>

export type KhalaBrowserReadInput = Readonly<{
  selector?: string
}>

export type KhalaBrowserReadTextResult = KhalaBrowserPageSnapshot & Readonly<{
  text: string
}>

export type KhalaBrowserReadDomResult = KhalaBrowserPageSnapshot & Readonly<{
  html: string
}>

export type KhalaBrowserWaitKind = "selector-visible" | "text-visible" | "url-includes"

export type KhalaBrowserWaitInput = Readonly<{
  kind: KhalaBrowserWaitKind
  selector?: string
  timeoutMs: number
  value?: string
}>

export type KhalaBrowserWaitResult = KhalaBrowserPageSnapshot & Readonly<{
  met: boolean
}>

export type KhalaBrowserScreenshotInput = Readonly<{
  label?: string
}>

export type KhalaBrowserScreenshotResult = KhalaBrowserPageSnapshot & Readonly<{
  bytes: Uint8Array
  height?: number
  mediaType: string
  width?: number
}>

export interface KhalaBrowserService {
  readonly click: (input: KhalaBrowserActionInput) => Effect.Effect<KhalaBrowserPageSnapshot, KhalaToolRuntimeError>
  readonly marker: "khala.browser_service"
  readonly navigate: (input: KhalaBrowserNavigateInput) => Effect.Effect<KhalaBrowserPageSnapshot, KhalaToolRuntimeError>
  readonly readDom: (input: KhalaBrowserReadInput) => Effect.Effect<KhalaBrowserReadDomResult, KhalaToolRuntimeError>
  readonly readText: (input: KhalaBrowserReadInput) => Effect.Effect<KhalaBrowserReadTextResult, KhalaToolRuntimeError>
  readonly screenshot: (input: KhalaBrowserScreenshotInput) => Effect.Effect<KhalaBrowserScreenshotResult, KhalaToolRuntimeError>
  readonly typeText: (input: KhalaBrowserTypeInput) => Effect.Effect<KhalaBrowserPageSnapshot, KhalaToolRuntimeError>
  readonly waitFor: (input: KhalaBrowserWaitInput) => Effect.Effect<KhalaBrowserWaitResult, KhalaToolRuntimeError>
}

export interface KhalaToolServices {
  readonly browser: KhalaBrowserService
  readonly interaction: KhalaInteractionService
  readonly network: KhalaNetworkService
  readonly outputStore: KhalaOutputStore
  readonly permission: KhalaPermissionService
  readonly process: KhalaProcessService
  readonly search: KhalaWebSearchService
  readonly todo: KhalaTodoService
  readonly workspace: KhalaWorkspaceService
}

export class KhalaToolRuntimeError extends S.TaggedErrorClass<KhalaToolRuntimeError>()(
  "KhalaToolRuntimeError",
  {
    code: S.String,
    reason: S.String,
  },
) {}

export interface KhalaToolExecuteContext {
  readonly definition: KhalaToolDefinition
  readonly invocation: KhalaToolInvocation
  readonly services: KhalaToolServices
}

export interface RegisteredKhalaTool {
  readonly definition: KhalaToolDefinition
  readonly execute?: (
    input: Readonly<Record<string, unknown>>,
    context: KhalaToolExecuteContext,
  ) => Effect.Effect<KhalaToolResult, KhalaToolRuntimeError>
}

export interface KhalaToolRegistry {
  readonly list: () => ReadonlyArray<KhalaToolDefinition>
  readonly materialize: (availability: KhalaToolAvailability) => ReadonlyArray<KhalaToolDefinition>
  readonly register: (tool: RegisteredKhalaTool) => void
  readonly resolve: (name: string) => RegisteredKhalaTool | undefined
}

export function makeKhalaToolRegistry(initial: ReadonlyArray<RegisteredKhalaTool> = []): KhalaToolRegistry {
  const tools = new Map<string, RegisteredKhalaTool>()
  const register = (tool: RegisteredKhalaTool): void => {
    tools.set(tool.definition.name, tool)
  }
  for (const tool of initial) register(tool)
  return {
    list: () => [...tools.values()].map(tool => tool.definition),
    materialize: availability =>
      [...tools.values()]
        .map(tool => tool.definition)
        .filter(definition => definition.availability.includes(availability)),
    register,
    resolve: name => tools.get(name),
  }
}

export function executeKhalaTool(
  registry: KhalaToolRegistry,
  invocation: KhalaToolInvocation,
  services: KhalaToolServices,
): Effect.Effect<KhalaToolResult, never> {
  const tool = registry.resolve(invocation.name)
  if (tool === undefined) {
    return Effect.succeed(khalaToolError("unknown_tool", `Unknown tool: ${invocation.name}`))
  }
  if (tool.execute === undefined) {
    return Effect.succeed(khalaToolError("missing_handler", `Tool has no execute handler: ${invocation.name}`))
  }
  if (!isRecord(invocation.arguments)) {
    return Effect.succeed(khalaToolError("invalid_arguments", "Invalid tool input: expected an object"))
  }
  const definition = tool.definition
  if (definition.permissionMode === "deny") {
    return Effect.succeed(khalaToolDenied("permission_policy_denied", `${definition.name} is denied by policy`))
  }
  const permissionEffect =
    definition.permissionMode === "approval_required"
      ? services.permission.decide(permissionRequestFor(definition, invocation, services))
      : Effect.succeed("allow" as const)

  return permissionEffect.pipe(
    Effect.flatMap(decision => {
      if (decision === "deny") {
        return Effect.succeed(khalaToolDenied("permission_denied", `${definition.name} denied by permission service`))
      }
      return tool.execute!(invocation.arguments, { definition, invocation, services }).pipe(
        Effect.map(sanitizeToolResult),
        Effect.catchTag("KhalaToolRuntimeError", error =>
          Effect.succeed(khalaToolError(error.code, error.reason)),
        ),
      )
    }),
  )
}

export function khalaToolOk(input: {
  readonly modelText: string
  readonly publicSummary?: string
  readonly ui?: unknown
  readonly artifacts?: ReadonlyArray<KhalaToolArtifact>
  readonly publicSafety?: KhalaPublicSafety
  readonly privateDataRefs?: ReadonlyArray<string>
  readonly redactionRefs?: ReadonlyArray<string>
}): KhalaToolResult {
  return sanitizeToolResult({
    artifacts: [...(input.artifacts ?? [])],
    modelOutput: { text: input.modelText },
    privateDataRefs: [...(input.privateDataRefs ?? [])],
    publicSafety: input.publicSafety ?? "public_safe",
    publicSummary: input.publicSummary ?? input.modelText,
    redactionRefs: [...(input.redactionRefs ?? [])],
    status: "ok",
    ui: input.ui ?? null,
  })
}

export function khalaToolError(code: string, reason: string): KhalaToolResult {
  const safe = redactKhalaPublicText(`${code}: ${reason}`)
  return {
    artifacts: [],
    modelOutput: { text: safe },
    privateDataRefs: [],
    publicSafety: safe === `${code}: ${reason}` ? "public_safe" : "redacted",
    publicSummary: safe,
    redactionRefs: safe === `${code}: ${reason}` ? [] : ["redaction.khala_tool.error"],
    status: "failed",
    ui: { code, reason: safe },
  }
}

export function khalaToolDenied(code: string, reason: string): KhalaToolResult {
  const error = khalaToolError(code, reason)
  return { ...error, status: "denied" }
}

export function khalaToolNeedsInput(input: {
  readonly modelText: string
  readonly publicSummary: string
  readonly ui: unknown
  readonly publicSafety?: KhalaPublicSafety
}): KhalaToolResult {
  return sanitizeToolResult({
    artifacts: [],
    modelOutput: { text: input.modelText },
    privateDataRefs: [],
    publicSafety: input.publicSafety ?? "private",
    publicSummary: input.publicSummary,
    redactionRefs: [],
    status: "needs_input",
    ui: input.ui,
  })
}

export function khalaToolUnavailable(input: {
  readonly modelText: string
  readonly publicSummary: string
  readonly ui: unknown
  readonly publicSafety?: KhalaPublicSafety
}): KhalaToolResult {
  return sanitizeToolResult({
    artifacts: [],
    modelOutput: { text: input.modelText },
    privateDataRefs: [],
    publicSafety: input.publicSafety ?? "public_safe",
    publicSummary: input.publicSummary,
    redactionRefs: [],
    status: "unavailable",
    ui: input.ui,
  })
}

export function sanitizeToolResult(result: KhalaToolResult): KhalaToolResult {
  const publicSummary = redactKhalaPublicText(result.publicSummary)
  const modelText = redactKhalaPublicText(result.modelOutput.text)
  const redacted = publicSummary !== result.publicSummary || modelText !== result.modelOutput.text
  return {
    ...result,
    modelOutput: { text: modelText },
    publicSafety: redacted ? "redacted" : result.publicSafety,
    publicSummary,
    redactionRefs: redacted
      ? [...new Set([...result.redactionRefs, "redaction.khala_tool.public_text"])]
      : result.redactionRefs,
  }
}

export const allowAllKhalaPermissionService: KhalaPermissionService = {
  decide: () => Effect.succeed("allow"),
}

export const denyAllKhalaPermissionService: KhalaPermissionService = {
  decide: () => Effect.succeed("deny"),
}

export const inMemoryKhalaOutputStore = (): KhalaOutputStore & {
  readonly artifacts: ReadonlyArray<KhalaToolArtifact>
} => {
  const artifacts: KhalaToolArtifact[] = []
  return {
    artifacts,
    writeArtifact: input =>
      Effect.sync(() => {
        const artifact: KhalaToolArtifact = {
          artifactRef: `artifact.local.${artifacts.length + 1}`,
          private: true,
          ...(input.mediaType === undefined ? {} : { mediaType: input.mediaType }),
          ...(input.summary === undefined ? {} : { summary: redactKhalaPublicText(input.summary) }),
        }
        artifacts.push(artifact)
        return artifact
      }),
  }
}

export const nonInteractiveKhalaInteractionService: KhalaInteractionService = {
  askUser: input =>
    Effect.sync(() => {
      const requestId = createInteractionRequestId(input.invocationId)
      const requestedAt = Date.now()
      return {
        events: [
          {
            kind: "user_input_requested",
            payload: interactionRequestPayload(requestId, input),
            timestampMs: requestedAt,
          },
          {
            kind: "user_input_unavailable",
            payload: {
              reason: "host_interaction_unavailable",
              requestId,
            },
            timestampMs: requestedAt,
          },
        ],
        reason: "host_interaction_unavailable",
        requestId,
        status: "unavailable",
      }
    }),
  marker: "khala.interaction_service",
}

export function inMemoryKhalaTodoService(): KhalaTodoService {
  const sessions = new Map<string, Readonly<{ revision: number; todos: ReadonlyArray<KhalaTodoItem> }>>()
  return {
    marker: "khala.todo_service",
    writeTodos: input =>
      Effect.sync(() => {
        const previous = sessions.get(input.khalaSessionId)
        const revision = (previous?.revision ?? 0) + 1
        const updatedAtMs = Date.now()
        const todos = input.todos.map((todo, index): KhalaTodoItem => ({
          ...(todo.blockerReason === undefined ? {} : { blockerReason: todo.blockerReason }),
          content: todo.content,
          id: todo.id,
          order: index,
          status: todo.status,
          updatedAtMs,
        }))
        const event: KhalaTodoEvent = {
          kind: "todo_list_updated",
          payload: {
            invocationId: input.invocationId,
            revision,
            sessionId: input.khalaSessionId,
            todos,
          },
          timestampMs: updatedAtMs,
        }
        sessions.set(input.khalaSessionId, { revision, todos })
        return {
          events: [event],
          revision,
          sessionId: input.khalaSessionId,
          todos,
        }
      }),
  }
}

export function createFetchKhalaNetworkService(fetchImpl: typeof fetch = globalThis.fetch): KhalaNetworkService {
  return {
    fetchUrl: input =>
      Effect.promise(async () => {
        const controller = new AbortController()
        const timeout = setTimeout(() => controller.abort(), input.timeoutMs)
        const redirectChain: KhalaNetworkRedirect[] = []
        let currentUrl = input.url
        try {
          for (let redirects = 0; redirects <= input.maxRedirects; redirects += 1) {
            const response = await fetchImpl(currentUrl, {
              redirect: "manual",
              signal: controller.signal,
            })
            const location = response.headers.get("location")
            if (isRedirectStatus(response.status) && location !== null) {
              const nextUrl = new URL(location, currentUrl).toString()
              redirectChain.push({ from: currentUrl, status: response.status, to: nextUrl })
              currentUrl = nextUrl
              continue
            }
            const body = await readNetworkResponseBody(response, input.maxBytes)
            return {
              body: body.bytes,
              bodyTruncated: body.truncated,
              contentType: response.headers.get("content-type") ?? "application/octet-stream",
              fetchedAtMs: Date.now(),
              finalUrl: response.url || currentUrl,
              redirectChain,
              status: response.status,
              statusText: response.statusText,
              url: input.url,
            }
          }
          throw new Error(`too many redirects; limit ${input.maxRedirects}`)
        } catch (error) {
          if (controller.signal.aborted) throw new Error("network fetch timed out")
          throw error
        } finally {
          clearTimeout(timeout)
        }
      }),
    marker: "khala.network_service",
  }
}

export const unconfiguredKhalaWebSearchService: KhalaWebSearchService = {
  marker: "khala.web_search_service",
  search: () =>
    Effect.fail(new KhalaToolRuntimeError({
      code: "web_search_unconfigured",
      reason: "No Khala web search provider is configured.",
    })),
}

export const unconfiguredKhalaBrowserService: KhalaBrowserService = {
  click: () => browserUnavailable(),
  marker: "khala.browser_service",
  navigate: () => browserUnavailable(),
  readDom: () => browserUnavailable(),
  readText: () => browserUnavailable(),
  screenshot: () => browserUnavailable(),
  typeText: () => browserUnavailable(),
  waitFor: () => browserUnavailable(),
}

export function makeKhalaToolServices(input: {
  readonly browser?: KhalaBrowserService
  readonly interaction?: KhalaInteractionService
  readonly network?: KhalaNetworkService
  readonly permission?: KhalaPermissionService
  readonly process?: KhalaProcessService
  readonly search?: KhalaWebSearchService
  readonly todo?: KhalaTodoService
  readonly workingDirectory?: string
} = {}): KhalaToolServices {
  return {
    browser: input.browser ?? unconfiguredKhalaBrowserService,
    interaction: input.interaction ?? nonInteractiveKhalaInteractionService,
    network: input.network ?? createFetchKhalaNetworkService(),
    outputStore: inMemoryKhalaOutputStore(),
    permission: input.permission ?? allowAllKhalaPermissionService,
    process: input.process ?? defaultKhalaProcessService,
    search: input.search ?? unconfiguredKhalaWebSearchService,
    todo: input.todo ?? inMemoryKhalaTodoService(),
    workspace: { workingDirectory: input.workingDirectory ?? process.cwd() },
  }
}

function browserUnavailable(): Effect.Effect<never, KhalaToolRuntimeError> {
  return Effect.fail(new KhalaToolRuntimeError({
    code: "browser_unavailable",
    reason: "No Khala browser surface is configured.",
  }))
}

function createInteractionRequestId(invocationId: string): string {
  const safeInvocation = invocationId.replace(/[^A-Za-z0-9_.-]/gu, "_").slice(0, 80) || "request"
  return `khala.ask.${safeInvocation}.${Date.now().toString(36)}`
}

function interactionRequestPayload(
  requestId: string,
  input: KhalaInteractionAskInput,
): Readonly<{
  allowFreeform: boolean
  choices: ReadonlyArray<KhalaInteractionChoice>
  nonBlocking: boolean
  prompt: string
  requestId: string
  timeoutMs?: number
}> {
  return {
    allowFreeform: input.allowFreeform,
    choices: input.choices,
    nonBlocking: input.nonBlocking,
    prompt: input.prompt,
    requestId,
    ...(input.timeoutMs === undefined ? {} : { timeoutMs: input.timeoutMs }),
  }
}

function isRedirectStatus(status: number): boolean {
  return status === 301 || status === 302 || status === 303 || status === 307 || status === 308
}

async function readNetworkResponseBody(
  response: Response,
  maxBytes: number,
): Promise<Readonly<{ bytes: Uint8Array; truncated: boolean }>> {
  const limit = Math.max(0, maxBytes)
  const chunks: Buffer[] = []
  let byteLength = 0
  let truncated = false
  if (response.body === null) {
    const bytes = new Uint8Array(await response.arrayBuffer())
    return {
      bytes: bytes.byteLength > limit ? bytes.slice(0, limit) : bytes,
      truncated: bytes.byteLength > limit,
    }
  }

  const reader = response.body.getReader()
  while (true) {
    const read = await reader.read()
    if (read.done) break
    const remaining = limit - byteLength
    if (remaining <= 0 || read.value.byteLength > remaining) {
      if (remaining > 0) {
        chunks.push(Buffer.from(read.value.slice(0, remaining)))
        byteLength += remaining
      }
      truncated = true
      await reader.cancel()
      break
    }
    chunks.push(Buffer.from(read.value))
    byteLength += read.value.byteLength
  }
  return {
    bytes: Buffer.concat(chunks, byteLength),
    truncated,
  }
}

export const unsandboxedKhalaProcessService: KhalaProcessService = {
  execCommand: input =>
    Effect.promise(async () => {
      const started = Date.now()
      let timedOut = false
      let cancelled = false
      const command = input.argv !== undefined && input.argv.length > 0
        ? [...input.argv]
        : [input.shell ?? process.env.SHELL ?? "/bin/sh", "-lc", input.command]
      const proc = Bun.spawn(command, {
        cwd: input.cwd,
        stderr: "pipe",
        stdout: "pipe",
      })
      const kill = (reason: "cancelled" | "timedOut"): void => {
        if (reason === "cancelled") cancelled = true
        else timedOut = true
        proc.kill()
      }
      const timeout = setTimeout(() => kill("timedOut"), input.timeoutMs)
      const cancel = input.cancelAfterMs === undefined
        ? undefined
        : setTimeout(() => kill("cancelled"), input.cancelAfterMs)
      const events: KhalaProcessEvent[] = []
      const [stdout, stderr, exitCode] = await Promise.all([
        readProcessStream(proc.stdout, "stdout", input.maxCaptureBytes, events, () => proc.kill()),
        readProcessStream(proc.stderr, "stderr", input.maxCaptureBytes, events, () => proc.kill()),
        proc.exited.catch(() => null),
      ])
      clearTimeout(timeout)
      if (cancel !== undefined) clearTimeout(cancel)
      return {
        cancelled,
        durationMs: Date.now() - started,
        events: events.sort((a, b) => a.timestampMs - b.timestampMs),
        exitCode,
        sandbox: {
          enforced: false,
          kind: "none",
          note: "No sandbox is enforced by the default local Khala process service.",
        },
        signal: null,
        stderr: stderr.text,
        stderrTruncated: stderr.truncated,
        stdout: stdout.text,
        stdoutTruncated: stdout.truncated,
        timedOut,
      }
    }),
  marker: "khala.process_service",
  startSession: input =>
    Effect.promise(async () => {
      const started = Date.now()
      let timedOut = false
      let cancelled = false
      const command = input.argv !== undefined && input.argv.length > 0
        ? [...input.argv]
        : [input.shell ?? process.env.SHELL ?? "/bin/sh", "-lc", input.command]
      const proc = Bun.spawn(command, {
        cwd: input.cwd,
        stderr: "pipe",
        stdin: "pipe",
        stdout: "pipe",
      })
      const sessionId = `khala.proc.${Date.now().toString(36)}.${Math.random().toString(36).slice(2)}`
      const session: DefaultKhalaProcessSession = {
        cancelled: false,
        command: input.command,
        createdAtMs: started,
        events: [],
        exitCode: null,
        khalaSessionId: input.khalaSessionId,
        maxCaptureBytes: input.maxCaptureBytes,
        proc,
        sessionId,
        stderr: "",
        stderrTruncated: false,
        stdout: "",
        stdoutTruncated: false,
        timedOut: false,
      }
      defaultProcessSessions.set(sessionId, session)
      void pumpSessionStream(session, proc.stdout, "stdout")
      void pumpSessionStream(session, proc.stderr, "stderr")
      void proc.exited.then(exitCode => {
        session.exitCode = exitCode
      }).catch(() => {
        session.exitCode = null
      })
      setTimeout(() => {
        if (session.exitCode === null) {
          timedOut = true
          session.timedOut = true
          proc.kill()
        }
      }, input.timeoutMs)
      await sleep(input.yieldTimeMs)
      timedOut = session.timedOut
      cancelled = session.cancelled
      return sessionSnapshot(session, Date.now() - started, timedOut, cancelled)
    }),
  writeStdin: input =>
    Effect.promise(async () => {
      const session = defaultProcessSessions.get(input.sessionId)
      if (session === undefined) throw new Error(`unknown process session: ${input.sessionId}`)
      if (session.khalaSessionId !== input.khalaSessionId) {
        throw new Error("process session does not belong to the active Khala session")
      }
      if (input.chars !== undefined && input.chars.length > 0) {
        if (session.exitCode !== null) throw new Error(`process session is closed: ${input.sessionId}`)
        if (input.chars === "\u0003") {
          session.cancelled = true
          session.proc.kill()
        } else {
          const stdin = session.proc.stdin as unknown as BunFileSink | undefined
          if (stdin === undefined) throw new Error(`process session stdin is closed: ${input.sessionId}`)
          stdin.write(input.chars)
          stdin.flush()
        }
        session.events.push({ channel: "stdin", text: input.chars, timestampMs: Date.now() })
      }
      await sleep(input.yieldTimeMs)
      return sessionSnapshot(
        session,
        Date.now() - session.createdAtMs,
        session.timedOut,
        session.cancelled,
      )
    }),
}

export const defaultKhalaProcessService: KhalaProcessService = process.platform === "darwin"
  ? createMacosSeatbeltKhalaProcessService({ fallback: unsandboxedKhalaProcessService })
  : unsandboxedKhalaProcessService

type DefaultKhalaProcessSession = {
  cancelled: boolean
  command: string
  createdAtMs: number
  events: KhalaProcessEvent[]
  exitCode: number | null
  khalaSessionId: string
  maxCaptureBytes: number
  proc: ReturnType<typeof Bun.spawn>
  sessionId: string
  stderr: string
  stderrTruncated: boolean
  stdout: string
  stdoutTruncated: boolean
  timedOut: boolean
}

type BunFileSink = Readonly<{
  flush: () => void
  write: (value: string | Uint8Array) => unknown
}>

const defaultProcessSessions = new Map<string, DefaultKhalaProcessSession>()

async function pumpSessionStream(
  session: DefaultKhalaProcessSession,
  stream: ReadableStream<Uint8Array>,
  channel: "stdout" | "stderr",
): Promise<void> {
  const reader = stream.getReader()
  while (true) {
    const read = await reader.read()
    if (read.done) break
    const chunk = Buffer.from(read.value).toString("utf8")
    session.events.push({ channel, text: chunk, timestampMs: Date.now() })
    if (channel === "stdout") {
      if (Buffer.byteLength(session.stdout + chunk, "utf8") > session.maxCaptureBytes) {
        session.stdoutTruncated = true
        session.stdout = tailByBytes(`${session.stdout}${chunk}`, session.maxCaptureBytes)
      } else {
        session.stdout += chunk
      }
    } else if (Buffer.byteLength(session.stderr + chunk, "utf8") > session.maxCaptureBytes) {
      session.stderrTruncated = true
      session.stderr = tailByBytes(`${session.stderr}${chunk}`, session.maxCaptureBytes)
    } else {
      session.stderr += chunk
    }
  }
}

function sessionSnapshot(
  session: DefaultKhalaProcessSession,
  durationMs: number,
  timedOut: boolean,
  cancelled: boolean,
): KhalaProcessSessionResult {
  return {
    cancelled,
    durationMs,
    events: [...session.events].sort((a, b) => a.timestampMs - b.timestampMs),
    exitCode: session.exitCode,
    sandbox: {
      enforced: false,
      kind: "none",
      note: "No sandbox is enforced by the default local Khala process service.",
    },
    sessionId: session.sessionId,
    signal: null,
    stderr: session.stderr,
    stderrTruncated: session.stderrTruncated,
    stdout: session.stdout,
    stdoutTruncated: session.stdoutTruncated,
    timedOut,
  }
}

function tailByBytes(text: string, maxBytes: number): string {
  const bytes = Buffer.from(text, "utf8")
  if (bytes.byteLength <= maxBytes) return text
  return bytes.subarray(bytes.byteLength - maxBytes).toString("utf8")
}

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

async function readProcessStream(
  stream: ReadableStream<Uint8Array>,
  channel: KhalaProcessOutputChannel,
  maxBytes: number,
  events: KhalaProcessEvent[],
  onTruncate: () => void,
): Promise<Readonly<{ text: string; truncated: boolean }>> {
  const reader = stream.getReader()
  const chunks: Buffer[] = []
  let bytes = 0
  let truncated = false
  while (true) {
    const read = await reader.read()
    if (read.done) break
    const chunk = read.value
    const remaining = maxBytes - bytes
    if (remaining <= 0 || chunk.byteLength > remaining) {
      if (remaining > 0) {
        const kept = chunk.slice(0, remaining)
        chunks.push(Buffer.from(kept))
        events.push({ channel, text: Buffer.from(kept).toString("utf8"), timestampMs: Date.now() })
      }
      truncated = true
      onTruncate()
      break
    }
    bytes += chunk.byteLength
    chunks.push(Buffer.from(chunk))
    events.push({ channel, text: Buffer.from(chunk).toString("utf8"), timestampMs: Date.now() })
  }
  return {
    text: Buffer.concat(chunks).toString("utf8"),
    truncated,
  }
}

export const KhalaBackendKind = S.Literals(["hosted_openagents", "mock"])
export type KhalaBackendKind = typeof KhalaBackendKind.Type

export const KhalaBackendSelection = S.Struct({
  baseUrl: S.optional(S.String),
  credentialSource: S.optional(S.Literals(["env:OPENROUTER_API_KEY", "khala-provider-key"])),
  kind: KhalaBackendKind,
  model: S.String,
  provider: S.optional(S.Literal("openrouter")),
})
export type KhalaBackendSelection = typeof KhalaBackendSelection.Type

export function resolveKhalaBackend(input: {
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly preferred?: KhalaBackendKind
  readonly storedProviderKey?: Readonly<{ provider: string; keyConfigured: boolean }>
} = {}): KhalaBackendSelection {
  if (input.preferred === "mock") {
    return { kind: "mock", model: "mock/khala-tools" }
  }
  const envKey = input.env?.OPENROUTER_API_KEY?.trim()
  const storedOpenRouter =
    input.storedProviderKey?.provider === "openrouter" && input.storedProviderKey.keyConfigured
  const credentialSource =
    envKey !== undefined && envKey.length > 0
      ? "env:OPENROUTER_API_KEY"
      : storedOpenRouter
        ? "khala-provider-key"
        : undefined

  return {
    baseUrl: input.env?.OPENAGENTS_BASE_URL?.trim() || "https://openagents.com",
    ...(credentialSource === undefined ? {} : { credentialSource, provider: "openrouter" as const }),
    kind: "hosted_openagents",
    model: "openagents/khala",
  }
}

export interface OpenAiCompatibleToolDefinition {
  readonly function: {
    readonly description: string
    readonly name: string
    readonly parameters: KhalaJsonSchema
  }
  readonly type: "function"
}

export function toOpenAiCompatibleTool(definition: KhalaToolDefinition): OpenAiCompatibleToolDefinition {
  return {
    function: {
      description: definition.description,
      name: definition.name,
      parameters: definition.inputSchema,
    },
    type: "function",
  }
}

export function toOpenAiCompatibleTools(
  definitions: ReadonlyArray<KhalaToolDefinition>,
): ReadonlyArray<OpenAiCompatibleToolDefinition> {
  return definitions.map(toOpenAiCompatibleTool)
}

export { createReadTool, readToolDefinition } from "./read.js"
export { createLsTool, lsToolDefinition } from "./ls.js"
export { createGlobTool, globToolDefinition } from "./glob.js"
export { createGrepTool, grepToolDefinition } from "./grep.js"
export { createEditTool, editToolDefinition } from "./edit.js"
export { createWriteTool, writeToolDefinition } from "./write.js"
export { applyPatchToolDefinition, createApplyPatchTool } from "./apply-patch.js"
export { createExecCommandTool, execCommandToolDefinition } from "./exec-command.js"
export { createMacosSeatbeltKhalaProcessService, seatbeltProfile } from "./process-sandbox-macos.js"
export { createWriteStdinTool, writeStdinToolDefinition } from "./write-stdin.js"
export { askUserToolDefinition, createAskUserTool } from "./ask-user.js"
export { createTodoWriteTool, todoWriteToolDefinition } from "./todo-write.js"
export { createViewImageTool, viewImageToolDefinition } from "./view-image.js"
export { createWebFetchTool, webFetchToolDefinition } from "./web-fetch.js"
export { createWebSearchTool, webSearchToolDefinition } from "./web-search.js"
export {
  browserClickToolDefinition,
  browserNavigateToolDefinition,
  browserReadDomToolDefinition,
  browserReadTextToolDefinition,
  browserScreenshotToolDefinition,
  browserToolDefinitions,
  browserTypeToolDefinition,
  browserWaitForToolDefinition,
  createBrowserClickTool,
  createBrowserNavigateTool,
  createBrowserReadDomTool,
  createBrowserReadTextTool,
  createBrowserScreenshotTool,
  createBrowserTools,
  createBrowserTypeTool,
  createBrowserWaitForTool,
} from "./browser.js"

function permissionRequestFor(
  definition: KhalaToolDefinition,
  invocation: KhalaToolInvocation,
  services: KhalaToolServices,
): KhalaPermissionRequest {
  const resources = typeof invocation.arguments.path === "string"
    ? [invocation.arguments.path]
    : typeof invocation.arguments.url === "string"
      ? [invocation.arguments.url]
      : typeof invocation.arguments.query === "string"
        ? [invocation.arguments.query]
        : typeof invocation.arguments.selector === "string"
          ? [invocation.arguments.selector]
          : typeof invocation.arguments.label === "string"
            ? [invocation.arguments.label]
            : typeof invocation.arguments.value === "string"
              ? [invocation.arguments.value]
              : []
  return {
    action: definition.authority,
    authorityMode: definition.executionMode,
    publicSafety: "private",
    resources,
    saveScope: "once",
    sessionId: invocation.sessionId,
    toolCallId: invocation.id,
    toolName: definition.name,
    workingDirectory: services.workspace.workingDirectory,
  }
}

function isRecord(value: unknown): value is Readonly<Record<string, unknown>> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}
