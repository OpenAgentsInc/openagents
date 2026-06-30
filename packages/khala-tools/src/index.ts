import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process"
import { existsSync } from "node:fs"
import { Effect, Schema as S } from "effect"
import { makeKhalaToolDispatcher, type KhalaToolDispatcherOptions } from "./dispatcher.js"
import { makeKhalaPermissionPolicyService } from "./permission-policy.js"
import { createMacosSeatbeltKhalaProcessService } from "./process-sandbox-macos.js"
import { redactKhalaPublicText } from "./redaction.js"

export {
  approvalCacheKeysFor,
  makeInMemoryKhalaApprovalStore,
  makeKhalaPermissionPolicyService,
  type KhalaApprovalCacheKey,
  type KhalaApprovalScope,
  type KhalaApprovalStore,
  type KhalaPermissionPolicyOptions,
} from "./permission-policy.js"

export {
  defineKhalaFeatureRegistry,
  isKhalaFeatureEnabled,
  KhalaFeatureSpec,
  KhalaFeatureStage,
  parseKhalaFeatureFlagArgs,
  type KhalaFeatureConfig,
  type KhalaFeatureOverride,
  type KhalaFeatureRegistry,
  type KhalaFeatureResolution,
  type KhalaFeatureSpec as KhalaFeatureSpecType,
  type KhalaFeatureStage as KhalaFeatureStageType,
} from "./feature-flags.js"

export {
  KhalaFleetDelegateBlockerCode,
  KhalaFleetDelegateModuleError,
  KhalaFleetDelegateModuleName,
  KhalaFleetDelegatePrecondition,
  KhalaFleetDelegateSignature,
  KhalaFleetDelegateStepStatus,
  khalaFleetDelegateBlockerRef,
  prepareKhalaFleetDelegateWork,
  runKhalaFleetDelegateProgram,
  selectKhalaFleetDelegateAccount,
  type KhalaFleetDelegateAccount,
  type KhalaFleetDelegateAdvertiseResult,
  type KhalaFleetDelegateBlockedResult,
  type KhalaFleetDelegateCapacity,
  type KhalaFleetDelegateCompletedResult,
  type KhalaFleetDelegateDispatchResult,
  type KhalaFleetDelegateEnsureResult,
  type KhalaFleetDelegateInput,
  type KhalaFleetDelegateModules,
  type KhalaFleetDelegateProgramResult,
  type KhalaFleetDelegateStep,
  type KhalaFleetDelegateVerifyResult,
  type KhalaFleetDelegateWork,
} from "./fleet-delegate-program.js"

export {
  createExternalMcpRegisteredTools,
  createKhalaPublicMcpToolRegistry,
  handleKhalaMcpRequest,
  KHALA_MCP_PROTOCOL_VERSION,
  listKhalaMcpToolDefinitions,
  makeKhalaMcpClient,
  runKhalaMcpServerStdio,
  type KhalaMcpClient,
  type KhalaMcpClientPolicy,
  type KhalaMcpExternalServerConfig,
  type KhalaMcpExternalServerProjection,
  type KhalaMcpExternalTool,
  type KhalaMcpExternalToolProjection,
  type KhalaMcpExternalTransport,
  type KhalaMcpJsonValue,
  type KhalaMcpRequest,
  type KhalaMcpResponse,
  type KhalaMcpServerLifecycle,
  type KhalaMcpServerOptions,
  type KhalaMcpToolCallResult,
  type KhalaMcpToolContent,
  type KhalaMcpToolDefinition,
} from "./mcp.js"

export {
  appendKhalaSessionModelItem,
  appendKhalaSessionRolloutRecord,
  appendKhalaSessionToolEvent,
  compactKhalaSessionRollout,
  createKhalaSessionRollout,
  forkKhalaSessionRollout,
  KhalaSessionModelItem,
  KhalaSessionModelItemRole,
  KhalaSessionRolloutRecord,
  KhalaSessionRolloutRecordKind,
  KhalaSessionRolloutSchemaVersion,
  khalaSessionModelItems,
  khalaSessionRolloutPath,
  khalaSessionToolEvents,
  listKhalaSessionRollouts,
  parseKhalaSessionRolloutText,
  readKhalaSessionRollout,
  type KhalaSessionModelItem as KhalaSessionModelItemType,
  type KhalaSessionModelItemRole as KhalaSessionModelItemRoleType,
  type KhalaSessionRolloutAppendInput,
  type KhalaSessionRolloutAppendOptions,
  type KhalaSessionRolloutCreateOptions,
  type KhalaSessionRolloutForkOptions,
  type KhalaSessionRolloutLoaded,
  type KhalaSessionRolloutRecord as KhalaSessionRolloutRecordType,
  type KhalaSessionRolloutRecordKind as KhalaSessionRolloutRecordKindType,
  type KhalaSessionRolloutSummary,
} from "./session-rollout.js"

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

export const KhalaToolPlannerSource = S.Literals(["built_in", "mcp", "plugin"])
export type KhalaToolPlannerSource = typeof KhalaToolPlannerSource.Type

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

export type KhalaToolModelMetadata = Readonly<{
  readonly capabilities?: ReadonlyArray<string>
  readonly contextWindowTokens?: number
  readonly id?: string
}>

export type KhalaToolPlannerEnvironment = Readonly<Record<string, string | undefined>>

export type KhalaToolFeatureFlags = Readonly<Record<string, boolean | undefined>>

export type KhalaToolPlannerRule = Readonly<{
  readonly defer?: boolean
  readonly featureFlags?: ReadonlyArray<string>
  readonly modelCapabilities?: ReadonlyArray<string>
  readonly modes?: ReadonlyArray<KhalaToolAvailability>
  readonly requiredEnv?: ReadonlyArray<string>
  readonly searchable?: boolean
  readonly source?: KhalaToolPlannerSource
}>

export type KhalaToolPlanInput = Readonly<{
  readonly env?: KhalaToolPlannerEnvironment
  readonly featureFlags?: KhalaToolFeatureFlags
  readonly includeDeferred?: boolean
  readonly mode: KhalaToolAvailability
  readonly model?: KhalaToolModelMetadata
}>

export type KhalaDeferredToolDefinition = Readonly<{
  readonly definition: KhalaToolDefinition
  readonly searchText: string
  readonly source: Exclude<KhalaToolPlannerSource, "built_in">
}>

export type KhalaToolPlan = Readonly<{
  readonly deferred: ReadonlyArray<KhalaDeferredToolDefinition>
  readonly searchDeferredTools: (query: string, limit?: number) => ReadonlyArray<KhalaDeferredToolDefinition>
  readonly searchTool: KhalaToolDefinition | undefined
  readonly visible: ReadonlyArray<KhalaToolDefinition>
}>

export interface RegisteredKhalaTool {
  readonly definition: KhalaToolDefinition
  readonly execute?: (
    input: Readonly<Record<string, unknown>>,
    context: KhalaToolExecuteContext,
  ) => Effect.Effect<KhalaToolResult, KhalaToolRuntimeError>
  readonly planner?: KhalaToolPlannerRule
}

export interface KhalaToolRegistry {
  readonly list: () => ReadonlyArray<KhalaToolDefinition>
  readonly materialize: (availability: KhalaToolAvailability) => ReadonlyArray<KhalaToolDefinition>
  readonly plan: (input: KhalaToolPlanInput) => KhalaToolPlan
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
    plan: input => planKhalaTools([...tools.values()], input),
    register,
    resolve: name => tools.get(name),
  }
}

export const khalaToolSearchDefinition: KhalaToolDefinition = {
  authority: "external_directory",
  availability: ["extension"],
  description: "Search deferred MCP and plugin tools available to this turn.",
  executionMode: "hosted",
  inputSchema: {
    additionalProperties: false,
    properties: {
      limit: {
        minimum: 1,
        type: "number",
      },
      query: {
        type: "string",
      },
    },
    required: ["query"],
    type: "object",
  },
  internalId: "khala.tools.tool_search",
  label: "Search Tools",
  name: "tool_search",
  permissionMode: "allow",
  prompt: "Search deferred external tools by name, label, description, and prompt guidance.",
  promptGuidelines: [
    "Use when a needed MCP or plugin tool is not directly visible.",
    "First-party Khala tools are already visible and do not need tool_search discovery.",
  ],
}

export function planKhalaTools(
  tools: ReadonlyArray<RegisteredKhalaTool>,
  input: KhalaToolPlanInput,
): KhalaToolPlan {
  const visible: KhalaToolDefinition[] = []
  const deferred: KhalaDeferredToolDefinition[] = []

  for (const tool of tools) {
    if (!toolMatchesPlannerInput(tool, input)) continue

    const source = tool.planner?.source ?? "built_in"
    const defer = source !== "built_in" && tool.planner?.defer === true
    if (defer && input.includeDeferred !== true) {
      if (tool.planner?.searchable !== false) {
        deferred.push({
          definition: tool.definition,
          searchText: searchableTextForTool(tool.definition),
          source,
        })
      }
      continue
    }
    visible.push(tool.definition)
  }

  const searchDeferredTools = (query: string, limit = 8): ReadonlyArray<KhalaDeferredToolDefinition> =>
    searchKhalaDeferredTools(deferred, query, limit)
  const searchTool = deferred.length > 0 && visible.every(tool => tool.name !== khalaToolSearchDefinition.name)
    ? khalaToolSearchDefinition
    : undefined

  return {
    deferred,
    searchDeferredTools,
    searchTool,
    visible: searchTool === undefined ? visible : [...visible, searchTool],
  }
}

export function searchKhalaDeferredTools(
  tools: ReadonlyArray<KhalaDeferredToolDefinition>,
  query: string,
  limit = 8,
): ReadonlyArray<KhalaDeferredToolDefinition> {
  const terms = query
    .toLocaleLowerCase()
    .split(/\s+/u)
    .map(term => term.trim())
    .filter(Boolean)
  const boundedLimit = Math.max(0, Math.floor(limit))
  if (boundedLimit === 0) return []
  if (terms.length === 0) return tools.slice(0, boundedLimit)

  return tools
    .map(tool => ({
      score: scoreSearchableTool(tool.searchText, terms),
      tool,
    }))
    .filter(result => result.score > 0)
    .sort((left, right) => right.score - left.score || left.tool.definition.name.localeCompare(right.tool.definition.name))
    .slice(0, boundedLimit)
    .map(result => result.tool)
}

function toolMatchesPlannerInput(tool: RegisteredKhalaTool, input: KhalaToolPlanInput): boolean {
  const rule = tool.planner
  const modes = rule?.modes ?? tool.definition.availability
  if (!modes.includes(input.mode)) return false
  for (const flag of rule?.featureFlags ?? []) {
    if (input.featureFlags?.[flag] !== true) return false
  }
  for (const envName of rule?.requiredEnv ?? []) {
    if ((input.env?.[envName] ?? "") === "") return false
  }
  const capabilities = new Set(input.model?.capabilities ?? [])
  for (const capability of rule?.modelCapabilities ?? []) {
    if (!capabilities.has(capability)) return false
  }
  return true
}

function searchableTextForTool(definition: KhalaToolDefinition): string {
  return [
    definition.name,
    definition.label,
    definition.description,
    definition.prompt,
    ...definition.promptGuidelines,
  ].join(" ").toLocaleLowerCase()
}

function scoreSearchableTool(searchText: string, terms: ReadonlyArray<string>): number {
  let score = 0
  for (const term of terms) {
    if (searchText.includes(term)) score += term.length
  }
  return score
}

export function executeKhalaTool(
  registry: KhalaToolRegistry,
  invocation: KhalaToolInvocation,
  services: KhalaToolServices,
  options: KhalaToolDispatcherOptions = {},
): Effect.Effect<KhalaToolResult, never> {
  return makeKhalaToolDispatcher(options).dispatch({ invocation, registry, services }).pipe(
    Effect.map(dispatched => dispatched.result),
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
  const interaction = input.interaction ?? nonInteractiveKhalaInteractionService
  return {
    browser: input.browser ?? unconfiguredKhalaBrowserService,
    interaction,
    network: input.network ?? createFetchKhalaNetworkService(),
    outputStore: inMemoryKhalaOutputStore(),
    permission: input.permission ?? makeKhalaPermissionPolicyService({ interaction }),
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
      const proc = spawnProcessGroup(input, "pipes")
      const kill = (reason: "cancelled" | "timedOut"): void => {
        if (reason === "cancelled") cancelled = true
        else timedOut = true
        killProcessGroup(proc)
      }
      const timeout = setTimeout(() => kill("timedOut"), input.timeoutMs)
      const cancel = input.cancelAfterMs === undefined
        ? undefined
        : setTimeout(() => kill("cancelled"), input.cancelAfterMs)
      const events: KhalaProcessEvent[] = []
      const exit = waitForChildExit(proc)
      const [stdout, stderr, exitResult] = await Promise.all([
        readNodeProcessStream(proc.stdout, "stdout", input.maxCaptureBytes, events, () => killProcessGroup(proc)),
        readNodeProcessStream(proc.stderr, "stderr", input.maxCaptureBytes, events, () => killProcessGroup(proc)),
        exit,
      ])
      clearTimeout(timeout)
      if (cancel !== undefined) clearTimeout(cancel)
      return {
        cancelled,
        durationMs: Date.now() - started,
        events,
        exitCode: exitResult.exitCode,
        sandbox: {
          enforced: false,
          kind: "none",
          note: "No sandbox is enforced by the default local Khala process service.",
        },
        signal: exitResult.signal,
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
      const proc = spawnProcessGroup(input, "pty")
      const sessionId = `khala.proc.${Date.now().toString(36)}.${Math.random().toString(36).slice(2)}`
      const session: DefaultKhalaProcessSession = {
        cancelled: false,
        command: input.command,
        createdAtMs: started,
        events: [],
        exitCode: null,
        khalaSessionId: input.khalaSessionId,
        maxCaptureBytes: input.maxCaptureBytes,
        pgid: proc.pid,
        proc,
        sessionId,
        signal: null,
        stderr: "",
        stderrTruncated: false,
        stdout: "",
        stdoutTruncated: false,
        timedOut: false,
      }
      defaultProcessSessions.set(sessionId, session)
      void pumpSessionStream(session, proc.stdout, "stdout")
      void pumpSessionStream(session, proc.stderr, "stderr")
      void waitForChildExit(proc).then(exit => {
        session.exitCode = exit.exitCode
        session.signal = exit.signal
      })
      setTimeout(() => {
        if (session.exitCode === null) {
          timedOut = true
          session.timedOut = true
          killProcessGroup(proc)
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
          killProcessGroup(session.proc, "SIGINT")
        } else {
          if (session.proc.stdin.destroyed) throw new Error(`process session stdin is closed: ${input.sessionId}`)
          session.proc.stdin.write(input.chars)
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
  pgid: number | undefined
  proc: ChildProcessWithoutNullStreams
  sessionId: string
  signal: string | null
  stderr: string
  stderrTruncated: boolean
  stdout: string
  stdoutTruncated: boolean
  timedOut: boolean
}

const defaultProcessSessions = new Map<string, DefaultKhalaProcessSession>()

async function pumpSessionStream(
  session: DefaultKhalaProcessSession,
  stream: NodeJS.ReadableStream,
  channel: "stdout" | "stderr",
): Promise<void> {
  for await (const value of stream) {
    const chunk = Buffer.from(value as Buffer).toString("utf8")
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
    events: [...session.events],
    exitCode: session.exitCode,
    sandbox: {
      enforced: false,
      kind: "none",
      note: "No sandbox is enforced by the default local Khala process service.",
    },
    sessionId: session.sessionId,
    signal: session.signal,
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

type ChildExitResult = Readonly<{
  exitCode: number | null
  signal: string | null
}>

type ProcessSpawnMode = "pipes" | "pty"

function spawnProcessGroup(input: KhalaProcessExecInput, mode: ProcessSpawnMode): ChildProcessWithoutNullStreams {
  const rawCommand = input.argv !== undefined && input.argv.length > 0
    ? input.argv[0]
    : input.shell ?? process.env.SHELL ?? "/bin/sh"
  const rawArgs = input.argv !== undefined && input.argv.length > 0
    ? [...input.argv.slice(1)]
    : ["-lc", input.command]
  const [command, args] = mode === "pty"
    ? ptyWrappedCommand(rawCommand, rawArgs)
    : [rawCommand, rawArgs] as const
  return spawn(command, args, {
    cwd: input.cwd,
    detached: true,
    env: { ...process.env, TERM: process.env.TERM ?? "xterm-256color" },
    stdio: "pipe",
  })
}

function ptyWrappedCommand(command: string, args: ReadonlyArray<string>): readonly [string, ReadonlyArray<string>] {
  const python = existsSync("/usr/bin/python3") ? "/usr/bin/python3" : "python3"
  return [python, ["-c", PythonPtyBridge, command, ...args]]
}

function waitForChildExit(proc: ChildProcessWithoutNullStreams): Promise<ChildExitResult> {
  return new Promise(resolve => {
    proc.once("exit", (exitCode, signal) => resolve({ exitCode, signal }))
    proc.once("error", () => resolve({ exitCode: null, signal: null }))
  })
}

function killProcessGroup(proc: ChildProcessWithoutNullStreams, signal: NodeJS.Signals = "SIGTERM"): void {
  if (proc.pid === undefined) {
    proc.kill(signal)
    return
  }
  try {
    process.kill(-proc.pid, signal)
  } catch {
    proc.kill(signal)
  }
}

const PythonPtyBridge = `
import errno
import os
import pty
import select
import signal
import sys
argv = sys.argv[1:]
if not argv:
    sys.exit(127)
child_pid, fd = pty.fork()
if child_pid == 0:
    os.execvp(argv[0], argv)

def forward_signal(signum, _frame):
    try:
        os.kill(child_pid, signum)
    except ProcessLookupError:
        pass

signal.signal(signal.SIGINT, forward_signal)
signal.signal(signal.SIGTERM, forward_signal)
stdin_open = True
exit_status = None
while True:
    try:
        done, status = os.waitpid(child_pid, os.WNOHANG)
        if done == child_pid:
            exit_status = status
            break
    except ChildProcessError:
        exit_status = 0
        break
    readers = [fd]
    if stdin_open:
        readers.append(sys.stdin.fileno())
    try:
        readable, _, _ = select.select(readers, [], [], 0.05)
    except InterruptedError:
        continue
    if sys.stdin.fileno() in readable:
        data = os.read(sys.stdin.fileno(), 4096)
        if data:
            os.write(fd, data)
        else:
            stdin_open = False
    if fd in readable:
        try:
            data = os.read(fd, 4096)
        except OSError as error:
            if error.errno != errno.EIO:
                raise
            data = b""
        if data:
            os.write(sys.stdout.fileno(), data)
        else:
            break
while True:
    try:
        data = os.read(fd, 4096)
    except OSError:
        break
    if not data:
        break
    os.write(sys.stdout.fileno(), data)
if exit_status is None:
    try:
        _, exit_status = os.waitpid(child_pid, 0)
    except ChildProcessError:
        exit_status = 0
if os.WIFEXITED(exit_status):
    sys.exit(os.WEXITSTATUS(exit_status))
if os.WIFSIGNALED(exit_status):
    sys.exit(128 + os.WTERMSIG(exit_status))
sys.exit(1)
`

async function readNodeProcessStream(
  stream: NodeJS.ReadableStream,
  channel: KhalaProcessOutputChannel,
  maxBytes: number,
  events: KhalaProcessEvent[],
  onTruncate: () => void,
): Promise<Readonly<{ text: string; truncated: boolean }>> {
  const chunks: Buffer[] = []
  let bytes = 0
  let truncated = false
  for await (const value of stream) {
    const chunk = Buffer.from(value as Buffer)
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
  createKhalaToolTurnAccounting,
  makeKhalaToolDispatcher,
  type KhalaToolDispatcher,
  type KhalaToolDispatcherOptions,
  type KhalaToolDispatchAfterHookContext,
  type KhalaToolDispatchEventContext,
  type KhalaToolDispatchHookContext,
  type KhalaToolDispatchHooks,
  type KhalaToolDispatchInput,
  type KhalaToolDispatchPhase,
  type KhalaToolDispatchResult,
  type KhalaToolTelemetryTags,
  type KhalaToolTelemetryTagValue,
  type KhalaToolTurnAccounting,
  type KhalaToolTurnAccountingSnapshot,
} from "./dispatcher.js"
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
