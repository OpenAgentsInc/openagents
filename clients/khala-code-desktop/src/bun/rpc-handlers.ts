import { randomUUID } from "node:crypto"

import type {
  KhalaCodexRateLimitProviderStatus,
  KhalaCodexRateLimitResetOutcome,
} from "../shared/codex-rate-limits.js"
import type { CodexAppServerHost } from "./codex-app-server-client.js"
import type { CodexAppServerNotification } from "./codex-app-server-client.js"
import {
  createCodexAppServerChatRuntime,
  type CodexAppServerChatRuntime,
} from "./codex-app-server-chat-runtime.js"
import type { KhalaAppleFmReadiness } from "../shared/apple-fm-readiness.js"
import type { OnDeviceDeciderSelection } from "../shared/on-device-decider.js"
import {
  type KhalaCodeDesktopAppInfo,
  type KhalaCodeDesktopCodexAppServerActionResult,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopChatTurnResponse,
  type KhalaCodeDesktopCodexConfigValueWriteRequest,
  type KhalaCodeDesktopCodexEcosystemReadRequest,
  type KhalaCodeDesktopCodexEcosystemReadResult,
  type KhalaCodeDesktopCodexSettingsReadRequest,
  type KhalaCodeDesktopCodexSettingsReadResult,
  type KhalaCodeDesktopSlashCommandDispatchRequest,
  type KhalaCodeDesktopSlashCommandDispatchResult,
  type KhalaCodeDesktopCodexAccountsStatus,
  type KhalaCodeDesktopCodexHarnessStatus,
  type KhalaCodeDesktopCodexRateLimitResetResult,
  type KhalaCodeDesktopFleetAccount,
  type KhalaCodeDesktopFleetAssignment,
  type KhalaCodeDesktopFleetHomeRole,
  type KhalaCodeDesktopFleetPromotionRequest,
  type KhalaCodeDesktopFleetPromotionResult,
  type KhalaCodeDesktopFleetQueuePolicy,
  type KhalaCodeDesktopFleetSessionRole,
  type KhalaCodeDesktopFleetWorkerSession,
  type KhalaCodeDesktopFleetStatus,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopRuntimeStatus,
} from "../shared/rpc.js"
import {
  khalaCodeDesktopCodexApprovalResponsePayload,
  type KhalaCodeDesktopJsonRpcId,
} from "../shared/codex-approval-decisions.js"
import { projectKhalaCodeDesktopCodexSettings } from "../shared/codex-settings.js"
import { projectKhalaCodeDesktopCodexEcosystem } from "../shared/codex-ecosystem.js"
import {
  evaluateKhalaCodeDesktopSlashCommandAvailability,
  khalaCodeDesktopSlashCommandsWithAvailability,
  parseKhalaCodeDesktopSlashCommand,
} from "../shared/codex-slash-commands.js"
import { inspectCodexHarnessStatus } from "./codex-harness-status.js"
import {
  consumeKhalaCodexRateLimitResetCredit,
  fetchKhalaCodexRateLimitStatus,
} from "./codex-rate-limits.js"
import {
  khalaCodeDesktopToolCatalog,
  runKhalaCodeDesktopChatTurn,
} from "./khala-chat-runtime.js"
import {
  beginCodexConnect,
  collectCodexAccountEmails,
  ensureLocalPylon,
  inspectCodexFleet,
  spawnCodexInstances,
  type KhalaCodexFleetToolOptions,
  openExternalUrl,
  removeCodexAccount,
} from "./khala-codex-fleet-tools.js"

type ChatEnv = Readonly<Record<string, string | undefined>>
type MaybePromise<T> = T | Promise<T>

export type KhalaCodeDesktopRpcHandlersInput = {
  readonly appleFmReadiness: () => MaybePromise<KhalaAppleFmReadiness>
  readonly codexAppServerHost?: CodexAppServerHost
  readonly codexChatRuntime?: CodexAppServerChatRuntime
  readonly codexRateLimitStatus?: () => MaybePromise<KhalaCodexRateLimitProviderStatus>
  readonly codexHarnessStatus?: () => MaybePromise<KhalaCodeDesktopCodexHarnessStatus>
  readonly consumeCodexRateLimitResetCredit?: (input: {
    readonly idempotencyKey: string
  }) => MaybePromise<KhalaCodexRateLimitResetOutcome>
  readonly codexFleetToolOptions?: KhalaCodexFleetToolOptions
  readonly env: ChatEnv
  readonly emitChatTurnEvent?: (event: KhalaCodeDesktopChatTurnEvent) => void
  readonly legacyChatTurn?: typeof runKhalaCodeDesktopChatTurn
  readonly onDeviceDeciderStatus: () => MaybePromise<OnDeviceDeciderSelection>
  readonly workingDirectory: string
}

const appInfo = (): KhalaCodeDesktopAppInfo => ({
  ok: true,
  app: "Khala Code Desktop",
  observedAt: new Date().toISOString(),
})

const runtimeStatus = (input: {
  readonly available: boolean
  readonly capability: KhalaCodeDesktopRuntimeStatus["capability"]
  readonly reason: string
  readonly status: KhalaCodeDesktopRuntimeStatus["status"]
}): KhalaCodeDesktopRuntimeStatus => ({
  ok: true,
  app: "Khala Code Desktop",
  available: input.available,
  capability: input.capability,
  observedAt: new Date().toISOString(),
  reason: input.reason,
  status: input.status,
})

const isDisplayOnlyDefaultAccountRef = (accountRef: string): boolean =>
  /^(?:\(default\)|default)$/iu.test(accountRef.trim())

const accountSessionRole = (
  accountRef: string,
): KhalaCodeDesktopFleetSessionRole =>
  isDisplayOnlyDefaultAccountRef(accountRef)
    ? "main_local_codex_session"
    : "swarm_worker_codex_session"

const accountHomeRole = (
  accountRef: string,
): KhalaCodeDesktopFleetHomeRole =>
  isDisplayOnlyDefaultAccountRef(accountRef)
    ? "main_user_codex_home_display_only"
    : "pylon_isolated_worker_codex_home"

const fleetQueuePolicy = (
  capacity: KhalaCodeDesktopFleetAccount["capacity"],
  readiness: string,
): KhalaCodeDesktopFleetQueuePolicy => {
  const value = readiness.toLowerCase()
  const cooldown =
    value.includes("cooldown") || value.includes("cooling")
      ? "cooling_down"
      : value === "ready" || value === "available"
        ? "ready"
        : value === "unknown"
          ? "unknown"
          : "none_reported"
  return {
    admission: "pylon_capacity_gate",
    cooldown,
    refill: "pylon_presence_heartbeat",
    queued: capacity?.queued ?? null,
  }
}

const sessionLayers = (): NonNullable<KhalaCodeDesktopFleetStatus["sessionLayers"]> => ({
  main: {
    homeRole: "main_user_codex_home_display_only",
    label: "Main local Codex session",
    mutationPolicy: "codex_app_server_owned",
    role: "main_local_codex_session",
    runtime: "codex_harness",
    transcriptSurface: "chat",
  },
  workers: {
    homeRole: "pylon_isolated_worker_codex_home",
    label: "Khala swarm worker Codex sessions",
    mutationPolicy: "pylon_isolated_home_only",
    role: "swarm_worker_codex_session",
    runtime: "codex_harness",
    transcriptSurface: "fleet",
  },
})

const workerSessionForAssignment = (
  marker: {
    readonly assignmentRef: string | null
    readonly blockerRefs: readonly string[]
    readonly closeoutStatus: string | null
    readonly tokenRate: KhalaCodeDesktopFleetAssignment["tokenRate"]
    readonly transcriptRef: string | null
  },
): KhalaCodeDesktopFleetWorkerSession => {
  const hasBlocker = marker.blockerRefs.length > 0
  const hasCloseout = marker.closeoutStatus !== null
  const approvalRequired = marker.blockerRefs.some(ref => /approval|permission/iu.test(ref))
  return {
    approvalState: approvalRequired
      ? "approval_required"
      : hasBlocker
        ? "blocked"
        : hasCloseout
          ? "ready_for_review"
          : "none",
    blockerRefs: marker.blockerRefs,
    closeoutStatus: marker.closeoutStatus,
    executionRuntime: "codex_harness",
    homeRole: "pylon_isolated_worker_codex_home",
    queuePolicy: {
      admission: "pylon_capacity_gate",
      cooldown: hasBlocker ? "unknown" : "ready",
      refill: "pylon_presence_heartbeat",
      queued: null,
    },
    reviewState: hasBlocker
      ? "blocked"
      : hasCloseout || marker.tokenRate.status === "exact"
        ? "ready_for_review"
        : "active",
    role: "swarm_worker_codex_session",
    transcriptRef: marker.transcriptRef ?? marker.assignmentRef,
  }
}

const renderFleetPromotionObjective = (
  request: KhalaCodeDesktopFleetPromotionRequest,
): string => {
  const allowedRefs = request.contextBoundary.allowedRefs.length === 0
    ? "none"
    : request.contextBoundary.allowedRefs.join(", ")
  return [
    "Khala swarm delegation from a main local Codex thread.",
    `Origin thread: ${request.threadId}`,
    `Context boundary: ${request.contextBoundary.mode}; transcript included: false; allowed refs: ${allowedRefs}.`,
    request.contextBoundary.summary === null
      ? null
      : `User summary: ${request.contextBoundary.summary}`,
    `Objective: ${request.objective.trim()}`,
  ].filter((line): line is string => line !== null).join("\n")
}

const promoteThreadResult = (
  request: KhalaCodeDesktopFleetPromotionRequest,
  spawn: Awaited<ReturnType<typeof spawnCodexInstances>>,
): KhalaCodeDesktopFleetPromotionResult => ({
  acceptedCount: spawn.acceptedCount,
  contextBoundary: request.contextBoundary,
  ok: spawn.acceptedCount === spawn.requestedCount,
  origin: {
    role: "main_local_codex_session",
    sessionId: request.sessionId,
    threadId: request.threadId,
  },
  pylonRef: spawn.pylonRef,
  requestedCount: spawn.requestedCount,
  results: spawn.results.map(slot => ({
    accountRef: slot.accountRef,
    assignmentRef: slot.assignmentRef,
    closeoutStatus: slot.closeoutStatus,
    status: slot.status,
    summary: slot.summary,
    tokensVerified: slot.tokensVerified,
    transcriptRef: slot.transcriptRef,
  })),
  workerRuntime: {
    assignmentTool: "codex_spawn",
    homeRole: "pylon_isolated_worker_codex_home",
    role: "swarm_worker_codex_session",
    runtime: "codex_harness",
  },
})

const codexStatusFromRateLimits = (
  rateLimits: KhalaCodexRateLimitProviderStatus,
  harness: KhalaCodeDesktopCodexHarnessStatus,
  env: ChatEnv,
): KhalaCodeDesktopCodexAccountsStatus => {
  const available = harness.available && rateLimits.status === "ok"
  const credentialSource = env.CODEX_HOME?.trim()
    ? "CODEX_HOME" as const
    : "default_home" as const
  const readinessState =
    harness.auth.state !== "ready"
      ? harness.auth.state === "invalid"
        ? "invalid" as const
        : harness.auth.state === "error"
          ? "error" as const
          : "credentials_missing" as const
      : rateLimits.status === "ok"
      ? "ready" as const
      : rateLimits.status === "unavailable"
        ? "credentials_missing" as const
        : "error" as const
  const blockerRefs =
    readinessState === "ready"
      ? []
      : harness.auth.blockerRefs.length > 0
        ? harness.auth.blockerRefs
        : readinessState === "credentials_missing"
          ? ["blocker.codex.credentials_missing"]
          : ["blocker.codex.rate_limit_status_error"]
  const status = available
    ? "ready" as const
    : harness.status === "unavailable" || rateLimits.status === "unavailable"
      ? "unavailable" as const
      : "error" as const

  return {
    ok: true,
    app: "Khala Code Desktop",
    available,
    capability: "codex_accounts",
    observedAt: new Date().toISOString(),
    reason: available
      ? "Codex CLI account is signed in, the harness is ready, and rate-limit windows are available."
      : harness.available
        ? rateLimits.error ?? "Codex account status is unavailable."
        : harness.reason,
    status,
    accounts: [
      {
        provider: "codex",
        accountRef: "default",
        credentialSource,
        homeRef: credentialSource === "CODEX_HOME" ? "env:CODEX_HOME" : "default:~/.codex",
        homeRole: "main_user_codex_home",
        readiness: {
          state: readinessState,
          blockerRefs,
        },
        rateLimits,
      },
    ],
    harness,
    rateLimits,
  }
}

const unavailableRateLimits = (
  error: string,
): KhalaCodexRateLimitProviderStatus => ({
  provider: "codex",
  session: null,
  weekly: null,
  rateLimitResetCredits: null,
  updatedAtIso: new Date().toISOString(),
  error,
  status: "unavailable",
})

export function createKhalaCodeDesktopRpcRequestHandlers(
  input: KhalaCodeDesktopRpcHandlersInput,
): KhalaCodeDesktopRPCSchema["requests"] {
  const codexChatRuntime =
    input.codexChatRuntime ??
    (input.codexAppServerHost === undefined
      ? null
      : createCodexAppServerChatRuntime({
        env: input.env,
        host: input.codexAppServerHost,
        ...(input.emitChatTurnEvent === undefined ? {} : { onEvent: input.emitChatTurnEvent }),
        workingDirectory: input.workingDirectory,
      }))
  const legacyChatTurn = input.legacyChatTurn ?? runKhalaCodeDesktopChatTurn
  const requireCodexChatRuntime = (): CodexAppServerChatRuntime => {
    if (codexChatRuntime === null) {
      throw new Error("Codex app-server chat runtime is not configured.")
    }
    return codexChatRuntime
  }
  const useLegacyKhalaNativeRuntime = (): boolean =>
    input.env.KHALA_CODE_DESKTOP_RUNTIME === "khala_native_runtime" ||
    input.env.KHALA_CODE_DESKTOP_LEGACY_KHALA_NATIVE_RUNTIME === "1"

  const labelLegacyRuntimeResponse = (
    response: KhalaCodeDesktopChatTurnResponse,
  ): KhalaCodeDesktopChatTurnResponse => ({
    ...response,
    backend: {
      ...response.backend,
      runtimeMode: "khala_native_runtime",
      toolCatalogKind: "khala_native_legacy",
    },
    messages: [
      {
        id: `legacy-runtime-${Date.now().toString(36)}`,
        role: "system",
        body: "Legacy Khala native runtime handled this turn. The default Khala Code path wraps the local Codex harness.",
      },
      ...response.messages,
    ],
  })

  const labelCodexHarnessResponse = (
    response: KhalaCodeDesktopChatTurnResponse,
  ): KhalaCodeDesktopChatTurnResponse => ({
    ...response,
    backend: {
      ...response.backend,
      runtimeMode: "codex_harness",
      toolCatalogKind: response.backend.toolCatalogKind ?? "codex_app_server",
    },
  })

  const ecosystemNotifications: CodexAppServerNotification[] = []
  const ecosystemNotificationMethods = new Set([
    "app/list/updated",
    "mcpServer/oauthLogin/completed",
    "mcpServer/startupStatus/updated",
    "skills/changed",
  ])
  input.codexAppServerHost?.subscribe(notification => {
    if (!ecosystemNotificationMethods.has(notification.method)) return
    ecosystemNotifications.push(notification)
    if (ecosystemNotifications.length > 50) {
      ecosystemNotifications.splice(0, ecosystemNotifications.length - 50)
    }
  })

  const codexHarnessStatus = async (): Promise<KhalaCodeDesktopCodexHarnessStatus> =>
    input.codexHarnessStatus?.() ??
    inspectCodexHarnessStatus({ env: input.env as NodeJS.ProcessEnv })

  const codexAccountsStatus = async (): Promise<KhalaCodeDesktopCodexAccountsStatus> => {
    const harness = await codexHarnessStatus()
    if (!harness.available) {
      return codexStatusFromRateLimits(unavailableRateLimits(harness.reason), harness, input.env)
    }
    const rateLimits = await (input.codexRateLimitStatus?.() ??
      fetchKhalaCodexRateLimitStatus({ env: input.env as NodeJS.ProcessEnv }))
    return codexStatusFromRateLimits(rateLimits, harness, input.env)
  }

  const threadIdForSlashCommand = async (
    request: KhalaCodeDesktopSlashCommandDispatchRequest,
  ): Promise<string | null> => {
    const explicit = request.threadId?.trim()
    if (explicit !== undefined && explicit.length > 0) return explicit
    return await codexChatRuntime?.threadIdForSession(request.sessionId) ?? null
  }

  const blockedSlashCommand = (
    request: {
      readonly command?: string
      readonly message: string
      readonly method?: string
      readonly threadId?: string
    },
  ): KhalaCodeDesktopSlashCommandDispatchResult => ({
    ok: false,
    status: "blocked",
    ...request,
  })

  const dispatchedSlashCommand = (
    request: {
      readonly command: string
      readonly message: string
      readonly method: string
      readonly response?: unknown
      readonly threadId?: string
    },
  ): KhalaCodeDesktopSlashCommandDispatchResult => ({
    ok: true,
    status: "dispatched",
    ...request,
  })

  const requestCodexAppServer = async (
    method: string,
    params?: unknown,
  ): Promise<unknown> => {
    if (input.codexAppServerHost === undefined) {
      throw new Error("Codex app-server host is not configured.")
    }
    return input.codexAppServerHost.request(method, params)
  }

  const codexAppServerAction = async (
    method: string,
    params?: unknown,
  ): Promise<KhalaCodeDesktopCodexAppServerActionResult> => {
    try {
      return {
        ok: true,
        method,
        response: await requestCodexAppServer(method, params),
      }
    } catch (error) {
      return {
        ok: false,
        method,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const readCodexEcosystem = async (
    request: KhalaCodeDesktopCodexEcosystemReadRequest = {},
  ): Promise<KhalaCodeDesktopCodexEcosystemReadResult> => {
    const cwd = request.cwd ?? input.workingDirectory
    if (input.codexAppServerHost === undefined) {
      return projectKhalaCodeDesktopCodexEcosystem({
        cwd,
        errors: ["Codex app-server host is not configured."],
      })
    }

    const errors: string[] = []
    const capture = async <Result>(
      label: string,
      method: string,
      params?: unknown,
    ): Promise<Result | undefined> => {
      try {
        return await input.codexAppServerHost!.request<Result>(method, params)
      } catch (error) {
        errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`)
        return undefined
      }
    }

    const [
      skillsList,
      hooksList,
      pluginList,
      pluginInstalled,
      appsList,
      mcpServerStatusList,
    ] = await Promise.all([
      capture("skills/list", "skills/list", {
        cwds: [cwd],
        ...(request.forceReloadSkills === undefined ? {} : { forceReload: request.forceReloadSkills }),
      }),
      capture("hooks/list", "hooks/list", { cwds: [cwd] }),
      capture("plugin/list", "plugin/list", { cwds: [cwd] }),
      capture("plugin/installed", "plugin/installed", { cwds: [cwd] }),
      capture("app/list", "app/list", {
        ...(request.threadId === undefined ? {} : { threadId: request.threadId }),
        ...(request.forceRefetchApps === undefined ? {} : { forceRefetch: request.forceRefetchApps }),
      }),
      capture("mcpServerStatus/list", "mcpServerStatus/list", {
        detail: "full",
        ...(request.threadId === undefined ? {} : { threadId: request.threadId }),
      }),
    ])

    return projectKhalaCodeDesktopCodexEcosystem({
      cwd,
      errors,
      skillsList,
      hooksList,
      pluginList,
      pluginInstalled,
      appsList,
      mcpServerStatusList,
      notifications: ecosystemNotifications.slice(-25),
    })
  }

  const readCodexSettings = async (
    request: KhalaCodeDesktopCodexSettingsReadRequest = {},
  ): Promise<KhalaCodeDesktopCodexSettingsReadResult> => {
    const cwd = request.cwd ?? input.workingDirectory
    if (input.codexAppServerHost === undefined) {
      return projectKhalaCodeDesktopCodexSettings({
        cwd,
        errors: ["Codex app-server host is not configured."],
      })
    }

    const errors: string[] = []
    const capture = async <Result>(
      label: string,
      method: string,
      params?: unknown,
    ): Promise<Result | undefined> => {
      try {
        return await input.codexAppServerHost!.request<Result>(method, params)
      } catch (error) {
        errors.push(`${label}: ${error instanceof Error ? error.message : String(error)}`)
        return undefined
      }
    }

    const [
      configRead,
      modelList,
      providerCapabilities,
      permissionProfileList,
      requirementsRead,
      usageRead,
      collaborationModeList,
    ] = await Promise.all([
      capture("config/read", "config/read", { cwd, includeLayers: true }),
      capture("model/list", "model/list", {
        includeHidden: request.includeHiddenModels === true,
      }),
      capture("modelProvider/capabilities/read", "modelProvider/capabilities/read", {}),
      capture("permissionProfile/list", "permissionProfile/list", { cwd }),
      capture("configRequirements/read", "configRequirements/read"),
      capture("account/usage/read", "account/usage/read"),
      capture("collaborationMode/list", "collaborationMode/list", {}),
    ])

    return projectKhalaCodeDesktopCodexSettings({
      cwd,
      errors,
      configRead,
      modelList,
      providerCapabilities,
      permissionProfileList,
      requirementsRead,
      usageRead,
      collaborationModeList,
    })
  }

  const writeCodexConfigValue = async (
    request: KhalaCodeDesktopCodexConfigValueWriteRequest,
  ) => {
    if (input.codexAppServerHost === undefined) {
      return {
        ok: false,
        keyPath: request.keyPath,
        error: "Codex app-server host is not configured.",
      }
    }
    try {
      const response = await input.codexAppServerHost.request("config/value/write", {
        keyPath: request.keyPath,
        value: request.value,
        mergeStrategy: request.mergeStrategy ?? "replace",
        ...(request.filePath === undefined ? {} : { filePath: request.filePath }),
        ...(request.expectedVersion === undefined ? {} : { expectedVersion: request.expectedVersion }),
      })
      return {
        ok: true,
        keyPath: request.keyPath,
        response,
        settings: await readCodexSettings({
          includeHiddenModels: true,
          ...(request.cwd === undefined ? {} : { cwd: request.cwd }),
        }),
      }
    } catch (error) {
      return {
        ok: false,
        keyPath: request.keyPath,
        error: error instanceof Error ? error.message : String(error),
      }
    }
  }

  const dispatchSlashAppServerCommand = async (
    request: KhalaCodeDesktopSlashCommandDispatchRequest,
  ): Promise<KhalaCodeDesktopSlashCommandDispatchResult> => {
    const parsed = parseKhalaCodeDesktopSlashCommand(request.raw, {
      ...(request.debug === undefined ? {} : { debug: request.debug }),
      ...(request.platform === undefined ? {} : { platform: request.platform }),
    })
    if (parsed === null) {
      return {
        ok: false,
        status: "not_found",
        message: "Unknown Codex slash command.",
      }
    }
    const command = parsed.command
    const availability = evaluateKhalaCodeDesktopSlashCommandAvailability(command, request)
    if (!availability.available) {
      return blockedSlashCommand({
        command: command.command,
        message: availability.reason ?? `/${command.command} is not available here.`,
      })
    }
    const dispatch = command.dispatch
    if (dispatch.kind === "gap") {
      return {
        ok: false,
        status: "gap",
        command: command.command,
        message: dispatch.dependency,
      }
    }
    if (dispatch.kind === "client") {
      return {
        ok: true,
        status: "client_action",
        action: dispatch.action,
        command: command.command,
        message: `/${command.command} is handled by the Khala Code desktop shell.`,
      }
    }

    const args = parsed.args
    if (dispatch.requiresArgs === true && args.length === 0) {
      return blockedSlashCommand({
        command: command.command,
        message: `/${command.command} requires inline arguments.`,
        method: dispatch.method,
      })
    }

    const threadId = await threadIdForSlashCommand(request)
    if (dispatch.requiresThread === true && threadId === null) {
      return blockedSlashCommand({
        command: command.command,
        message: `/${command.command} requires an active Codex thread.`,
        method: dispatch.method,
      })
    }

    try {
      switch (command.command) {
        case "new": {
          const response = await requireCodexChatRuntime().startThread({
            cwd: request.cwd ?? input.workingDirectory,
            sessionId: request.sessionId,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Started a new Codex thread.",
            response,
            threadId: response.threadId,
          })
        }
        case "resume": {
          const resumeThreadId = args.split(/\s+/)[0]?.trim()
          if (resumeThreadId === undefined || resumeThreadId.length === 0) {
            return blockedSlashCommand({
              command: command.command,
              message: "/resume requires a Codex thread id until the desktop picker lands.",
              method: dispatch.method,
            })
          }
          const response = await requireCodexChatRuntime().resumeThread({
            cwd: request.cwd ?? input.workingDirectory,
            sessionId: request.sessionId,
            threadId: resumeThreadId,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `Resumed Codex thread ${resumeThreadId}.`,
            response,
            threadId: response.threadId,
          })
        }
        case "compact": {
          const response = await requireCodexChatRuntime().compactThread({
            sessionId: request.sessionId,
            ...(threadId === null ? {} : { threadId }),
          })
          return {
            ok: response.ok,
            status: response.ok ? "dispatched" : "blocked",
            command: command.command,
            method: dispatch.method,
            message: response.ok
              ? "Requested Codex context compaction."
              : response.error ?? "Codex context compaction could not start.",
            response,
            ...(response.threadId === undefined ? {} : { threadId: response.threadId }),
          }
        }
        case "archive":
        case "delete":
        case "fork": {
          const response = await requestCodexAppServer(dispatch.method, { threadId })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `/${command.command} was sent to Codex.`,
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "rename": {
          const response = await requestCodexAppServer(dispatch.method, {
            threadId,
            name: args,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `Renamed the Codex thread to "${args}".`,
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "goal": {
          const method =
            args.length === 0
              ? "thread/goal/get"
              : args.toLowerCase() === "clear"
                ? "thread/goal/clear"
                : dispatch.method
          const params = args.length === 0 || args.toLowerCase() === "clear"
            ? { threadId }
            : { threadId, objective: args, status: "active" }
          const response = await requestCodexAppServer(method, params)
          return dispatchedSlashCommand({
            command: command.command,
            method,
            message: args.length === 0
              ? "Loaded the current Codex goal."
              : args.toLowerCase() === "clear"
                ? "Cleared the current Codex goal."
                : "Updated the current Codex goal.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "review": {
          const response = await requestCodexAppServer(dispatch.method, {
            threadId,
            target: args.length === 0
              ? { type: "uncommittedChanges" }
              : { type: "custom", instructions: args },
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Started a Codex review turn.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "ps":
        case "stop": {
          const response = await requestCodexAppServer(dispatch.method, { threadId })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: command.command === "ps"
              ? "Loaded Codex background terminal commands."
              : "Requested cleanup of Codex background terminal commands.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "mcp": {
          const response = await requestCodexAppServer(dispatch.method, {
            ...(threadId === null ? {} : { threadId }),
            detail: "full",
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex MCP server status.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "app":
        case "apps": {
          const response = await requestCodexAppServer(dispatch.method, {
            ...(threadId === null ? {} : { threadId }),
            forceRefetch: args.toLowerCase() === "refresh",
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex app integrations.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "plugins": {
          const response = await requestCodexAppServer(dispatch.method, {
            cwds: [request.cwd ?? input.workingDirectory],
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex plugins.",
            response,
          })
        }
        case "model": {
          const response = await requestCodexAppServer(dispatch.method, {
            includeHidden: args.toLowerCase() === "all",
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex models.",
            response,
          })
        }
        case "permissions": {
          const response = await requestCodexAppServer(dispatch.method, {
            cwd: request.cwd ?? input.workingDirectory,
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex permission profiles.",
            response,
          })
        }
        case "experimental": {
          const response = await requestCodexAppServer(dispatch.method, {
            ...(threadId === null ? {} : { threadId }),
          })
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: "Loaded Codex experimental features.",
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
        case "usage":
        case "logout": {
          const response = await requestCodexAppServer(dispatch.method)
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: command.command === "usage"
              ? "Loaded Codex token usage."
              : "Requested Codex sign-out.",
            response,
          })
        }
        default: {
          const response = await requestCodexAppServer(dispatch.method)
          return dispatchedSlashCommand({
            command: command.command,
            method: dispatch.method,
            message: `/${command.command} was sent to Codex.`,
            response,
            ...(threadId === null ? {} : { threadId }),
          })
        }
      }
    } catch (error) {
      return blockedSlashCommand({
        command: command.command,
        message: error instanceof Error ? error.message : String(error),
        method: dispatch.method,
        ...(threadId === null ? {} : { threadId }),
      })
    }
  }

  return {
    async appInfo() {
      return appInfo()
    },
    async appleFmReadiness() {
      return input.appleFmReadiness()
    },
    async codexAppServerRestart() {
      return input.codexAppServerHost?.restart() ?? {
        ok: false,
        action: "restart",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "errored",
          transport: "stdio",
        },
        error: "Codex app-server host is not configured.",
      }
    },
    async codexAppServerStart() {
      return input.codexAppServerHost?.start() ?? {
        ok: false,
        action: "start",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "errored",
          transport: "stdio",
        },
        error: "Codex app-server host is not configured.",
      }
    },
    async codexAppServerStatus() {
      return input.codexAppServerHost?.status() ?? {
        ok: true,
        app: "Khala Code Desktop",
        adapterVersion: "unconfigured",
        codexCommand: "codex",
        codexHome: "",
        diagnostics: [],
        initialized: false,
        initializeResult: null,
        lastError: "Codex app-server host is not configured.",
        pendingRequestCount: 0,
        pid: null,
        state: "errored",
        transport: "stdio",
      }
    },
    async codexAppServerStop() {
      return input.codexAppServerHost?.stop() ?? {
        ok: true,
        action: "stop",
        changed: false,
        status: {
          ok: true,
          app: "Khala Code Desktop",
          adapterVersion: "unconfigured",
          codexCommand: "codex",
          codexHome: "",
          diagnostics: [],
          initialized: false,
          initializeResult: null,
          lastError: "Codex app-server host is not configured.",
          pendingRequestCount: 0,
          pid: null,
          state: "stopped",
          transport: "stdio",
        },
      }
    },
    async codexAccountsStatus() {
      return codexAccountsStatus()
    },
    async codexFleetStatus(): Promise<KhalaCodeDesktopFleetStatus> {
      const fleet = await inspectCodexFleet(
        { includeProcesses: true, startPylon: false },
        { ...input.codexFleetToolOptions, env: input.env as NodeJS.ProcessEnv },
      )
      const emails = await collectCodexAccountEmails(
        fleet.accounts.map(account => account.accountRef),
        { env: input.env },
      )
      return {
        ok: fleet.ensure.ok,
        observedAt: fleet.observedAt,
        sessionLayers: sessionLayers(),
        pylon: {
          status: fleet.ensure.status,
          pylonRef: fleet.ensure.pylonRef,
          message: fleet.ensure.message,
        },
        availableCodexAssignments: fleet.availableCodexAssignments,
        maxCodexAssignments: fleet.maxCodexAssignments,
        accounts: fleet.accounts.map(account => ({
          accountRef: account.accountRef,
          provider: account.provider,
          readiness: account.readiness,
          quotaState: account.quotaState,
          accountKey: account.accountKey,
          capacity: account.capacity,
          homeRole: accountHomeRole(account.accountRef),
          queuePolicy: fleetQueuePolicy(account.capacity, account.readiness),
          sessionRole: accountSessionRole(account.accountRef),
          email: emails[account.accountRef] ?? null,
        })),
        activeAssignments: fleet.activeAssignments.map(marker => ({
          assignmentRef: marker.assignmentRef,
          blockerRefs: marker.blockerRefs,
          closeoutStatus: marker.closeoutStatus,
          elapsedMs: marker.elapsedMs,
          issueRef: marker.issueRef,
          tokenRate: marker.tokenRate,
          workerSession: workerSessionForAssignment(marker),
          updatedAt: marker.updatedAt,
        })),
        tokenRate: fleet.tokenRate,
        processes: fleet.processes.map(process => ({
          pid: process.pid,
          parentPid: process.parentPid,
          elapsed: process.elapsed,
        })),
      }
    },
    async codexFleetPromoteThread(request): Promise<KhalaCodeDesktopFleetPromotionResult> {
      if (request.sessionId.trim().length === 0) {
        throw new Error("codexFleetPromoteThread requires a sessionId")
      }
      if (request.threadId.trim().length === 0) {
        throw new Error("codexFleetPromoteThread requires a threadId")
      }
      if (request.objective.trim().length === 0) {
        throw new Error("codexFleetPromoteThread requires an explicit objective")
      }
      if (request.contextBoundary.includeTranscript !== false) {
        throw new Error("codexFleetPromoteThread requires includeTranscript: false")
      }
      const spawn = await spawnCodexInstances({
        accountRef: request.accountRef,
        branch: request.branch,
        commit: request.commit,
        count: request.count,
        fixture: request.fixture,
        noRun: request.noRun,
        prompt: renderFleetPromotionObjective(request),
        repo: request.repo,
        timeoutMs: request.timeoutMs,
        verify: request.verify,
      }, {
        ...input.codexFleetToolOptions,
        env: input.env,
      })
      return promoteThreadResult(request, spawn)
    },
    async codexHarnessStatus() {
      return codexHarnessStatus()
    },
    async codexApprovalRespond(request) {
      const payload = khalaCodeDesktopCodexApprovalResponsePayload(request)
      try {
        input.codexAppServerHost?.respondToServerRequest(
          request.requestId as KhalaCodeDesktopJsonRpcId,
          payload,
        )
        if (input.codexAppServerHost === undefined) {
          throw new Error("Codex app-server host is not configured.")
        }
        return {
          ok: true,
          method: request.method,
          payload,
          requestId: request.requestId,
        }
      } catch (error) {
        return {
          ok: false,
          method: request.method,
          payload,
          requestId: request.requestId,
          error: error instanceof Error ? error.message : String(error),
        }
      }
    },
    async codexConfigValueWrite(request) {
      return writeCodexConfigValue(request)
    },
    async codexEcosystemRead(request = {}) {
      return readCodexEcosystem(request)
    },
    async codexMarketplaceAdd(request) {
      return codexAppServerAction("marketplace/add", {
        source: request.source,
        ...(request.refName === undefined ? {} : { refName: request.refName }),
        ...(request.sparsePaths === undefined ? {} : { sparsePaths: request.sparsePaths }),
      })
    },
    async codexMarketplaceRemove(request) {
      return codexAppServerAction("marketplace/remove", request)
    },
    async codexMarketplaceUpgrade(request = {}) {
      return codexAppServerAction("marketplace/upgrade", request)
    },
    async codexMcpOauthLogin(request) {
      return codexAppServerAction("mcpServer/oauth/login", {
        name: request.server,
        ...(request.threadId === undefined ? {} : { threadId: request.threadId }),
        ...(request.scopes === undefined ? {} : { scopes: request.scopes }),
        ...(request.timeoutSecs === undefined ? {} : { timeoutSecs: request.timeoutSecs }),
      })
    },
    async codexMcpResourceRead(request) {
      return codexAppServerAction("mcpServer/resource/read", request)
    },
    async codexMcpServerReload() {
      return codexAppServerAction("config/mcpServer/reload")
    },
    async codexMcpToolCall(request) {
      return codexAppServerAction("mcpServer/tool/call", {
        threadId: request.threadId,
        server: request.server,
        tool: request.tool,
        ...(request.arguments === undefined ? {} : { arguments: request.arguments }),
        ...(request.meta === undefined ? {} : { _meta: request.meta }),
      })
    },
    async codexPluginInstall(request) {
      return codexAppServerAction("plugin/install", request)
    },
    async codexPluginUninstall(request) {
      return codexAppServerAction("plugin/uninstall", request)
    },
    async codexSettingsRead(request = {}) {
      return readCodexSettings(request)
    },
    async codexSkillsConfigWrite(request) {
      return codexAppServerAction("skills/config/write", request)
    },
    async codexSkillsExtraRootsSet(request) {
      return codexAppServerAction("skills/extraRoots/set", {
        extraRoots: request.extraRoots,
      })
    },
    async codexThreadArchive(request) {
      return requireCodexChatRuntime().archiveThread(request)
    },
    async codexThreadCompact(request) {
      return requireCodexChatRuntime().compactThread(request)
    },
    async codexThreadDelete(request) {
      return requireCodexChatRuntime().deleteThread(request)
    },
    async codexThreadFork(request) {
      return requireCodexChatRuntime().forkThread(request)
    },
    async codexThreadList(request) {
      return requireCodexChatRuntime().listThreads(request)
    },
    async codexThreadRead(request) {
      return requireCodexChatRuntime().readThread(request)
    },
    async codexThreadRename(request) {
      return requireCodexChatRuntime().renameThread(request)
    },
    async codexThreadResume(request) {
      return requireCodexChatRuntime().resumeThread(request)
    },
    async codexThreadStart(request = {}) {
      return requireCodexChatRuntime().startThread({
        cwd: input.workingDirectory,
        ...request,
      })
    },
    async codexThreadUnarchive(request) {
      return requireCodexChatRuntime().unarchiveThread(request)
    },
    async codexTurnInterrupt(request) {
      return requireCodexChatRuntime().interruptTurn(request)
    },
    async codexTurnStart(request) {
      return requireCodexChatRuntime().startTurn({
        ...request,
        cwd: request.cwd ?? input.workingDirectory,
      })
    },
    async codexTurnSteer(request) {
      return requireCodexChatRuntime().steerTurn(request)
    },
    async codingStatus() {
      const harness = await codexHarnessStatus()
      if (!harness.available) {
        return runtimeStatus({
          available: false,
          capability: "coding",
          reason: harness.reason,
          status: harness.status,
        })
      }
      return runtimeStatus({
        available: true,
        capability: "coding",
        reason: "Khala Code coding is gated on the local Codex harness.",
        status: "ready",
      })
    },
    async onDeviceDeciderStatus() {
      return input.onDeviceDeciderStatus()
    },
    async connectCodexAccount(accountRef: string) {
      const result = await beginCodexConnect(accountRef, { env: input.env })
      if (result.verificationUrl !== null) openExternalUrl(result.verificationUrl)
      return result
    },
    async openExternalUrl(url: string) {
      return openExternalUrl(url)
    },
    async removeCodexAccount(accountRef: string) {
      return removeCodexAccount(accountRef, { env: input.env })
    },
    async pylonStatus() {
      const status = await ensureLocalPylon({
        start: false,
        timeoutMs: 10_000,
        waitMs: 0,
      }, {
        env: input.env,
      })
      return runtimeStatus({
        available: status.ok,
        capability: "pylon",
        reason: status.ok
          ? status.message
          : `${status.message}${status.unavailableReason ? ` ${status.unavailableReason}` : ""}`,
        status: status.ok ? "ready" : "unavailable",
      })
    },
    async slashCommandDispatch(request) {
      return dispatchSlashAppServerCommand(request)
    },
    async slashCommandList(request = {}) {
      return {
        ok: true,
        commands: khalaCodeDesktopSlashCommandsWithAvailability({
          ...(request.activeTurn === undefined ? {} : { activeTurn: request.activeTurn }),
          ...(request.debug === undefined ? {} : { debug: request.debug }),
          ...(request.platform === undefined ? {} : { platform: request.platform }),
          ...(request.sideConversation === undefined ? {} : { sideConversation: request.sideConversation }),
        }),
      }
    },
    async submitChatMessage(request) {
      if (!useLegacyKhalaNativeRuntime()) {
        return labelCodexHarnessResponse(await requireCodexChatRuntime().startTurn({
          ...request,
          cwd: input.workingDirectory,
        }))
      }
      return labelLegacyRuntimeResponse(await legacyChatTurn({
        env: input.env,
        ...(input.emitChatTurnEvent === undefined ? {} : { onEvent: input.emitChatTurnEvent }),
        request,
        workingDirectory: input.workingDirectory,
      }))
    },
    async consumeCodexRateLimitResetCredit(): Promise<KhalaCodeDesktopCodexRateLimitResetResult> {
      const observedAt = new Date().toISOString()
      const idempotencyKey = randomUUID()
      try {
        const outcome = await (input.consumeCodexRateLimitResetCredit?.({
          idempotencyKey,
        }) ??
          consumeKhalaCodexRateLimitResetCredit({
            env: input.env as NodeJS.ProcessEnv,
            idempotencyKey,
          }))
        return {
          ok: true,
          observedAt,
          outcome,
          status: await codexAccountsStatus(),
        }
      } catch (error) {
        return {
          ok: false,
          observedAt,
          outcome: null,
          status: await codexAccountsStatus(),
          error: error instanceof Error ? error.message : String(error),
        }
      }
    },
    async tokenAccountingStatus() {
      return runtimeStatus({
        available: false,
        capability: "token_accounting",
        reason: "Token accounting is handled by hosted OpenAgents when a cloud credential is configured.",
        status: "not_configured",
      })
    },
    async toolCatalog() {
      return khalaCodeDesktopToolCatalog({
        runtimeMode: useLegacyKhalaNativeRuntime()
          ? "khala_native_runtime"
          : "codex_harness",
      })
    },
  }
}
