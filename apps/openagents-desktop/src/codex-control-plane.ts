import { createHash, randomUUID } from "node:crypto"
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import { dirname, join } from "node:path"

import {
  codexAppServerPoolKey,
  type CodexAppServerLease,
  type CodexAppServerPoolTarget,
  type CodexAppServerSupervisor,
} from "./codex-app-server-supervisor.ts"

export const CODEX_CONTROL_RECEIPT_SCHEMA = "openagents.desktop.codex_control_receipts.v1" as const

type JsonObject = Readonly<Record<string, unknown>>

export type CodexControlModel = Readonly<{
  id: string
  model: string
  displayName: string
  hidden: boolean
  isDefault: boolean
  defaultReasoningEffort: string | null
  supportedReasoningEfforts: ReadonlyArray<string>
}>

export type CodexControlSnapshot = Readonly<{
  revision: number
  generation: number
  observedAt: string
  account: Readonly<{ signedIn: boolean; authMode: string | null; planType: string | null }>
  loginFlow: Readonly<{ type: "chatgpt" | "chatgptDeviceCode"; loginId: string; authUrl?: string; verificationUrl?: string; userCode?: string }> | null
  loginCompletion: Readonly<{ loginId: string | null; success: boolean; error: string | null }> | null
  rateLimits: unknown
  usage: unknown
  workspaceMessages: ReadonlyArray<Readonly<{ messageId: string; messageType: string }>>
  models: ReadonlyArray<CodexControlModel>
  modelCapabilities: Readonly<{ imageGeneration: boolean; namespaceTools: boolean; webSearch: boolean }>
  config: unknown
  requirements: JsonObject | null
  permissionProfiles: ReadonlyArray<Readonly<{ id: string; allowed: boolean; description: string | null }>>
  experimentalFeatures: ReadonlyArray<Readonly<{ name: string; enabled: boolean; stage: string }>>
  collaborationModes: ReadonlyArray<Readonly<{ name: string; model: string | null }>>
  warnings: ReadonlyArray<Readonly<{ summary: string; details: string | null; path: string | null }>>
  errors: ReadonlyArray<Readonly<{ method: string; detail: string }>>
}>

export type CodexControlMutationKind =
  | "login"
  | "login_cancel"
  | "logout"
  | "reset_credit"
  | "workspace_nudge"
  | "config_write"
  | "config_batch_write"
  | "mcp_reload"
  | "feature_enablement"

export type CodexOwnerIntent = Readonly<{
  token: string
  kind: CodexControlMutationKind
  expectedRevision: number
  expiresAt: string
}>

export type CodexControlMutationReceipt = Readonly<{
  intentHash: string
  kind: CodexControlMutationKind
  method: string
  expectedRevision: number
  resultRevision: number
  outcome: "accepted" | "failed" | "stale" | "expired"
  observedAt: string
}>

export type CodexControlGate = Readonly<{
  allowed: boolean
  source: "app-server-models" | "model-capabilities" | "managed-requirements" | "permission-profiles" | "experimental-features"
  reason: string
}>

export type CodexControl =
  | Readonly<{ type: "model"; value: string }>
  | Readonly<{ type: "approvalPolicy"; value: unknown }>
  | Readonly<{ type: "sandboxMode"; value: string }>
  | Readonly<{ type: "permissionProfile"; value: string }>
  | Readonly<{ type: "feature"; value: string }>
  | Readonly<{ type: "webSearch"; value: string }>
  | Readonly<{ type: "namespaceTools" | "imageGeneration" | "remoteControl"; value: boolean }>

export class CodexControlPlaneError extends Error {
  readonly _tag = "CodexControlPlaneError"
  override readonly name = "CodexControlPlaneError"
  constructor(
    readonly reason: "closed" | "owner_intent_required" | "stale_state" | "intent_expired" | "intent_reused" | "invalid_response",
    message: string,
  ) { super(message) }
}

export type CodexControlPlane = Readonly<{
  initialize: () => Promise<CodexControlSnapshot>
  refresh: () => Promise<CodexControlSnapshot>
  snapshot: () => CodexControlSnapshot
  subscribe: (listener: (snapshot: CodexControlSnapshot) => void) => () => void
  authorizeOwnerIntent: (kind: CodexControlMutationKind, expectedRevision: number) => CodexOwnerIntent
  startLogin: (params: unknown, intent: CodexOwnerIntent) => Promise<unknown>
  cancelLogin: (loginId: string, intent: CodexOwnerIntent) => Promise<unknown>
  logout: (intent: CodexOwnerIntent) => Promise<unknown>
  consumeResetCredit: (params: Readonly<{ idempotencyKey: string; creditId?: string | null }>, intent: CodexOwnerIntent) => Promise<unknown>
  sendWorkspaceNudge: (creditType: "credits" | "usage_limit", intent: CodexOwnerIntent) => Promise<unknown>
  writeConfig: (params: unknown, intent: CodexOwnerIntent) => Promise<unknown>
  batchWriteConfig: (params: unknown, intent: CodexOwnerIntent) => Promise<unknown>
  reloadMcp: (intent: CodexOwnerIntent) => Promise<unknown>
  setFeatureEnablement: (enablement: Readonly<Record<string, boolean>>, intent: CodexOwnerIntent) => Promise<unknown>
  gate: (control: CodexControl) => CodexControlGate
  receipts: () => ReadonlyArray<CodexControlMutationReceipt>
  close: () => void
}>

const asObject = (value: unknown): JsonObject | null =>
  typeof value === "object" && value !== null && !Array.isArray(value) ? value as JsonObject : null

const string = (value: unknown): string | null => typeof value === "string" ? value : null
const boolean = (value: unknown): boolean => value === true
const array = (value: unknown): ReadonlyArray<unknown> => Array.isArray(value) ? value : []
const same = (left: unknown, right: unknown): boolean => JSON.stringify(left) === JSON.stringify(right)

const initialSnapshot = (): CodexControlSnapshot => ({
  revision: 0,
  generation: 0,
  observedAt: new Date(0).toISOString(),
  account: { signedIn: false, authMode: null, planType: null },
  loginFlow: null,
  loginCompletion: null,
  rateLimits: null,
  usage: null,
  workspaceMessages: [],
  models: [],
  modelCapabilities: { imageGeneration: false, namespaceTools: false, webSearch: false },
  config: null,
  requirements: null,
  permissionProfiles: [],
  experimentalFeatures: [],
  collaborationModes: [],
  warnings: [],
  errors: [],
})

const readReceipts = (path: string | undefined): CodexControlMutationReceipt[] => {
  if (path === undefined) return []
  try {
    const parsed = JSON.parse(readFileSync(path, "utf8")) as { schema?: unknown; receipts?: unknown }
    return parsed.schema === CODEX_CONTROL_RECEIPT_SCHEMA && Array.isArray(parsed.receipts)
      ? parsed.receipts.filter(receipt => receipt !== null && typeof receipt === "object") as CodexControlMutationReceipt[]
      : []
  } catch {
    return []
  }
}

export const makeCodexControlPlane = (options: Readonly<{
  lease: CodexAppServerLease
  receiptPath?: string
  now?: () => Date
  intentTtlMs?: number
  maxReceipts?: number
}>): CodexControlPlane => {
  const listeners = new Set<(snapshot: CodexControlSnapshot) => void>()
  const intents = new Map<string, CodexOwnerIntent>()
  const receipts = readReceipts(options.receiptPath)
  const maxReceipts = Math.max(1, Math.floor(options.maxReceipts ?? 2_048))
  const intentTtlMs = Math.max(1, Math.floor(options.intentTtlMs ?? 60_000))
  let state = initialSnapshot()
  let closed = false
  let removeNotifications: (() => void) | null = null

  const now = (): Date => options.now?.() ?? new Date()
  const assertOpen = (): void => {
    if (closed) throw new CodexControlPlaneError("closed", "Codex control plane is closed")
  }
  const publish = (patch: Partial<CodexControlSnapshot>, generation = state.generation): CodexControlSnapshot => {
    if (generation < state.generation) return state
    state = {
      ...state,
      ...patch,
      revision: state.revision + 1,
      generation,
      observedAt: now().toISOString(),
    }
    for (const listener of listeners) {
      try { listener(state) } catch { /* isolate control-plane observers */ }
    }
    return state
  }
  const persistReceipts = (): void => {
    if (options.receiptPath === undefined) return
    mkdirSync(dirname(options.receiptPath), { recursive: true })
    const temporary = `${options.receiptPath}.tmp`
    writeFileSync(temporary, `${JSON.stringify({
      schema: CODEX_CONTROL_RECEIPT_SCHEMA,
      receipts: receipts.slice(-maxReceipts),
    }, null, 2)}\n`, { mode: 0o600 })
    renameSync(temporary, options.receiptPath)
  }
  const record = (
    intent: CodexOwnerIntent,
    method: string,
    outcome: CodexControlMutationReceipt["outcome"],
  ): void => {
    receipts.push({
      intentHash: createHash("sha256").update(intent.token).digest("hex"),
      kind: intent.kind,
      method,
      expectedRevision: intent.expectedRevision,
      resultRevision: state.revision,
      outcome,
      observedAt: now().toISOString(),
    })
    if (receipts.length > maxReceipts) receipts.splice(0, receipts.length - maxReceipts)
    persistReceipts()
  }
  const consumeIntent = (expectedKind: CodexControlMutationKind, intent: CodexOwnerIntent, method: string): void => {
    assertOpen()
    const owned = intents.get(intent.token)
    if (owned === undefined) throw new CodexControlPlaneError("intent_reused", "Owner intent is unknown or already consumed")
    intents.delete(intent.token)
    if (owned.kind !== expectedKind || owned.kind !== intent.kind) {
      record(intent, method, "failed")
      throw new CodexControlPlaneError("owner_intent_required", `Owner intent does not authorize ${method}`)
    }
    if (Date.parse(owned.expiresAt) <= now().getTime()) {
      record(intent, method, "expired")
      throw new CodexControlPlaneError("intent_expired", `Owner intent expired before ${method}`)
    }
    if (owned.expectedRevision !== state.revision || intent.expectedRevision !== state.revision) {
      record(intent, method, "stale")
      throw new CodexControlPlaneError("stale_state", `Codex control state changed before ${method}`)
    }
  }

  const parseModels = (response: unknown): ReadonlyArray<CodexControlModel> => array(asObject(response)?.data).flatMap(raw => {
    const model = asObject(raw)
    const id = string(model?.id)
    const providerModel = string(model?.model)
    if (id === null || providerModel === null) return []
    return [{
      id,
      model: providerModel,
      displayName: string(model?.displayName) ?? providerModel,
      hidden: boolean(model?.hidden),
      isDefault: boolean(model?.isDefault),
      defaultReasoningEffort: string(model?.defaultReasoningEffort),
      supportedReasoningEfforts: array(model?.supportedReasoningEfforts)
        .flatMap(option => string(asObject(option)?.reasoningEffort) ?? string(option) ?? []),
    }]
  })
  const parseRequirements = (response: unknown): JsonObject | null => asObject(asObject(response)?.requirements)

  const refresh = async (): Promise<CodexControlSnapshot> => {
    assertOpen()
    const generation = options.lease.state().generation
    const optional = async (method: string, params: unknown): Promise<Readonly<{
      method: string
      value: unknown
      error: string | null
    }>> => {
      try { return { method, value: await options.lease.request(method, params), error: null } }
      catch (error) { return { method, value: null, error: error instanceof Error ? error.message.slice(0, 1_000) : "request failed" } }
    }
    const [accountResponse, limitsResult, usageResult, messagesResult, models, capabilities, config, requirements, profiles, features, modesResult] = await Promise.all([
      options.lease.request("account/read", { refreshToken: false }),
      optional("account/rateLimits/read", {}),
      optional("account/usage/read", {}),
      optional("account/workspaceMessages/read", {}),
      options.lease.request("model/list", { includeHidden: true }),
      options.lease.request("modelProvider/capabilities/read", {}),
      options.lease.request("config/read", { includeLayers: true }),
      options.lease.request("configRequirements/read", {}),
      options.lease.request("permissionProfile/list", {}),
      options.lease.request("experimentalFeature/list", {}),
      optional("collaborationMode/list", {}),
    ])
    const accountObject = asObject(accountResponse)
    const account = asObject(accountObject?.account)
    const capabilityObject = asObject(capabilities)
    return publish({
      account: {
        signedIn: account !== null,
        authMode: string(account?.type),
        planType: string(account?.planType),
      },
      rateLimits: limitsResult.value,
      usage: usageResult.value,
      workspaceMessages: array(asObject(messagesResult.value)?.messages).flatMap(raw => {
        const message = asObject(raw)
        const messageId = string(message?.messageId)
        const messageType = string(message?.messageType)
        return messageId === null || messageType === null ? [] : [{ messageId, messageType }]
      }),
      models: parseModels(models),
      modelCapabilities: {
        imageGeneration: boolean(capabilityObject?.imageGeneration),
        namespaceTools: boolean(capabilityObject?.namespaceTools),
        webSearch: boolean(capabilityObject?.webSearch),
      },
      config,
      requirements: parseRequirements(requirements),
      permissionProfiles: array(asObject(profiles)?.data).flatMap(raw => {
        const profile = asObject(raw)
        const id = string(profile?.id)
        return id === null ? [] : [{ id, allowed: boolean(profile?.allowed), description: string(profile?.description) }]
      }),
      experimentalFeatures: array(asObject(features)?.data).flatMap(raw => {
        const feature = asObject(raw)
        const name = string(feature?.name)
        return name === null ? [] : [{ name, enabled: boolean(feature?.enabled), stage: string(feature?.stage) ?? "underDevelopment" }]
      }),
      collaborationModes: array(asObject(modesResult.value)?.data).flatMap(raw => {
        const mode = asObject(raw)
        const name = string(mode?.name)
        return name === null ? [] : [{ name, model: string(mode?.model) }]
      }),
      errors: [limitsResult, usageResult, messagesResult, modesResult].flatMap(result =>
        result.error === null ? [] : [{ method: result.method, detail: result.error }]),
    }, generation)
  }

  const reconcileFor = async (method: string): Promise<void> => {
    if (method.startsWith("config/") || method === "experimentalFeature/enablement/set") {
      const [config, requirements, profiles, features] = await Promise.all([
        options.lease.request("config/read", { includeLayers: true }),
        options.lease.request("configRequirements/read", {}),
        options.lease.request("permissionProfile/list", {}),
        options.lease.request("experimentalFeature/list", {}),
      ])
      publish({
        config,
        requirements: parseRequirements(requirements),
        permissionProfiles: array(asObject(profiles)?.data).flatMap(raw => {
          const value = asObject(raw); const id = string(value?.id)
          return id === null ? [] : [{ id, allowed: boolean(value?.allowed), description: string(value?.description) }]
        }),
        experimentalFeatures: array(asObject(features)?.data).flatMap(raw => {
          const value = asObject(raw); const name = string(value?.name)
          return name === null ? [] : [{ name, enabled: boolean(value?.enabled), stage: string(value?.stage) ?? "underDevelopment" }]
        }),
      }, options.lease.state().generation)
    } else if (method.startsWith("account/")) {
      const accountResponse = await options.lease.request("account/read", { refreshToken: false })
      const account = asObject(asObject(accountResponse)?.account)
      publish({ account: {
        signedIn: account !== null,
        authMode: string(account?.type),
        planType: string(account?.planType),
      } }, options.lease.state().generation)
    }
  }

  const mutate = async (
    kind: CodexControlMutationKind,
    method: string,
    params: unknown,
    intent: CodexOwnerIntent,
  ): Promise<unknown> => {
    consumeIntent(kind, intent, method)
    try {
      const response = await options.lease.request(method, params)
      if (method === "account/login/start") {
        const flow = asObject(response)
        const type = string(flow?.type)
        const loginId = string(flow?.loginId)
        if ((type === "chatgpt" || type === "chatgptDeviceCode") && loginId !== null) {
          publish({ loginFlow: {
            type,
            loginId,
            ...(string(flow?.authUrl) === null ? {} : { authUrl: string(flow?.authUrl)! }),
            ...(string(flow?.verificationUrl) === null ? {} : { verificationUrl: string(flow?.verificationUrl)! }),
            ...(string(flow?.userCode) === null ? {} : { userCode: string(flow?.userCode)! }),
          } }, options.lease.state().generation)
        }
      }
      await reconcileFor(method)
      record(intent, method, "accepted")
      return response
    } catch (error) {
      record(intent, method, "failed")
      throw error
    }
  }

  const gate = (control: CodexControl): CodexControlGate => {
    const requirements = state.requirements
    if (control.type === "model") {
      const available = state.models.some(model => !model.hidden && (model.id === control.value || model.model === control.value))
      return { allowed: available, source: "app-server-models", reason: available ? "model is advertised" : "model is absent or hidden" }
    }
    if (control.type === "namespaceTools" || control.type === "imageGeneration") {
      const allowed = control.value === false || state.modelCapabilities[control.type]
      return { allowed, source: "model-capabilities", reason: allowed ? "provider capability permits control" : "provider capability is unavailable" }
    }
    if (control.type === "remoteControl") {
      const allowed = control.value === false || requirements?.allowRemoteControl !== false
      return { allowed, source: "managed-requirements", reason: allowed ? "managed requirements permit remote control" : "managed requirements prohibit remote control" }
    }
    if (control.type === "approvalPolicy" || control.type === "sandboxMode" || control.type === "webSearch") {
      const key = control.type === "approvalPolicy" ? "allowedApprovalPolicies"
        : control.type === "sandboxMode" ? "allowedSandboxModes" : "allowedWebSearchModes"
      const allowedValues = array(requirements?.[key])
      const capabilityAllowed = control.type !== "webSearch" || control.value === "disabled" || state.modelCapabilities.webSearch
      const allowed = capabilityAllowed && (allowedValues.length === 0 || allowedValues.some(value => same(value, control.value)))
      return { allowed, source: capabilityAllowed ? "managed-requirements" : "model-capabilities", reason: allowed ? "control is permitted" : "control is prohibited" }
    }
    if (control.type === "permissionProfile") {
      const managed = asObject(requirements?.allowedPermissionProfiles)
      const profile = state.permissionProfiles.find(candidate => candidate.id === control.value)
      const allowed = profile?.allowed === true && managed?.[control.value] !== false
      return { allowed, source: "permission-profiles", reason: allowed ? "permission profile is allowed" : "permission profile is prohibited" }
    }
    if (control.type === "feature") {
      const managed = asObject(requirements?.featureRequirements)
      const feature = state.experimentalFeatures.find(candidate => candidate.name === control.value)
      const allowed = feature?.enabled === true && managed?.[control.value] !== false
      return { allowed, source: "experimental-features", reason: allowed ? "feature is enabled and permitted" : "feature is disabled or prohibited" }
    }
    return { allowed: false, source: "managed-requirements", reason: "unknown control is prohibited" }
  }

  return {
    initialize: async () => {
      assertOpen()
      if (removeNotifications === null) {
        removeNotifications = options.lease.subscribe(notification => {
          if (notification.generation < state.generation) return
          const method = notification.message.method
          const params = asObject(notification.message.params)
          if (method === "account/login/completed") {
            publish({
              loginFlow: null,
              loginCompletion: {
                loginId: string(params?.loginId),
                success: boolean(params?.success),
                error: string(params?.error),
              },
            }, notification.generation)
            void reconcileFor("account/login/completed").catch(() => undefined)
          } else if (method === "account/updated") {
            publish({ account: {
              signedIn: true,
              authMode: string(params?.authMode),
              planType: string(params?.planType),
            } }, notification.generation)
          } else if (method === "configWarning") {
            publish({ warnings: [...state.warnings, {
              summary: (string(params?.summary) ?? "Codex config warning").slice(0, 400),
              details: string(params?.details)?.slice(0, 1_000) ?? null,
              path: string(params?.path),
            }].slice(-100) }, notification.generation)
          }
        })
      }
      return refresh()
    },
    refresh,
    snapshot: () => state,
    subscribe: listener => {
      assertOpen()
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    authorizeOwnerIntent: (kind, expectedRevision) => {
      assertOpen()
      if (expectedRevision !== state.revision) throw new CodexControlPlaneError("stale_state", "Cannot authorize intent from stale control state")
      const token = randomUUID()
      const intent = { token, kind, expectedRevision, expiresAt: new Date(now().getTime() + intentTtlMs).toISOString() }
      intents.set(token, intent)
      return intent
    },
    startLogin: (params, intent) => mutate("login", "account/login/start", params, intent),
    cancelLogin: (loginId, intent) => mutate("login_cancel", "account/login/cancel", { loginId }, intent),
    logout: intent => mutate("logout", "account/logout", {}, intent),
    consumeResetCredit: (params, intent) => mutate("reset_credit", "account/rateLimitResetCredit/consume", params, intent),
    sendWorkspaceNudge: (creditType, intent) => mutate("workspace_nudge", "account/sendAddCreditsNudgeEmail", { creditType }, intent),
    writeConfig: (params, intent) => mutate("config_write", "config/value/write", params, intent),
    batchWriteConfig: (params, intent) => mutate("config_batch_write", "config/batchWrite", params, intent),
    reloadMcp: intent => mutate("mcp_reload", "config/mcpServer/reload", {}, intent),
    setFeatureEnablement: (enablement, intent) => mutate("feature_enablement", "experimentalFeature/enablement/set", { enablement }, intent),
    gate,
    receipts: () => [...receipts],
    close: () => {
      if (closed) return
      closed = true
      removeNotifications?.()
      removeNotifications = null
      intents.clear()
      listeners.clear()
      options.lease.release()
    },
  }
}

export type CodexControlPlaneRegistry = Readonly<{
  forTarget: (target: CodexAppServerPoolTarget) => Promise<CodexControlPlane>
  close: () => void
}>

/** Main-process owner for one control plane per supervisor pool identity. */
export const makeCodexControlPlaneRegistry = (options: Readonly<{
  supervisor: CodexAppServerSupervisor
  receiptRoot: string
}>): CodexControlPlaneRegistry => {
  const entries = new Map<string, Promise<CodexControlPlane>>()
  let closed = false
  return {
    forTarget: target => {
      if (closed) return Promise.reject(new CodexControlPlaneError("closed", "Codex control-plane registry is closed"))
      const key = codexAppServerPoolKey(target)
      const existing = entries.get(key)
      if (existing !== undefined) return existing
      const created = options.supervisor.acquire(target).then(async lease => {
        const plane = makeCodexControlPlane({
          lease,
          receiptPath: join(options.receiptRoot, `${createHash("sha256").update(key).digest("hex")}.json`),
        })
        try {
          await plane.initialize()
          return plane
        } catch (error) {
          plane.close()
          entries.delete(key)
          throw error
        }
      })
      entries.set(key, created)
      return created
    },
    close: () => {
      if (closed) return
      closed = true
      for (const entry of entries.values()) void entry.then(plane => plane.close(), () => undefined)
      entries.clear()
    },
  }
}
