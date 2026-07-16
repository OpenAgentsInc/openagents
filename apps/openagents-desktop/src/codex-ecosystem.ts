import { createHash, randomUUID } from "node:crypto"
import { isAbsolute } from "node:path"

import { bundledCodex01441ProtocolManifest } from "@openagentsinc/codex-app-server-protocol/parity"

import type { CodexAppServerRequest } from "./codex-app-server-client.ts"
import { codexAppServerPoolKey, type CodexAppServerLease, type CodexAppServerNotification, type CodexAppServerPoolTarget, type CodexAppServerSupervisor } from "./codex-app-server-supervisor.ts"

type ObjectValue = Readonly<Record<string, unknown>>
const object = (value: unknown): ObjectValue | null => typeof value === "object" && value !== null && !Array.isArray(value) ? value as ObjectValue : null
const array = (value: unknown): ReadonlyArray<unknown> => Array.isArray(value) ? value : []
const string = (value: unknown): string | null => typeof value === "string" ? value : null
const bool = (value: unknown): boolean => value === true
const hash = (value: string): string => createHash("sha256").update(value).digest("hex")

export const CODEX_ECOSYSTEM_METHODS = Object.freeze(bundledCodex01441ProtocolManifest.members
  .filter(member =>
    /^(skills\/|hooks\/|hook\/|marketplace\/|plugin\/|app\/|mcpServer|config\/mcpServer)/u.test(member.method))
  .map(member => ({ method: member.method, direction: member.direction, stability: member.stability })))

export type CodexEcosystemPolicy = "available" | "blocked" | "under_development"
export type CodexEcosystemSnapshot = Readonly<{
  revision: number
  generation: number
  observedAt: string
  skills: ReadonlyArray<Readonly<{ id: string; name: string; description: string; scope: string; enabled: boolean; dependencies: ReadonlyArray<string> }>>
  hooks: ReadonlyArray<Readonly<{ key: string; event: string; type: string; enabled: boolean; trust: string; source: string }>>
  hookRuns: ReadonlyArray<Readonly<{ runRef: string; hookKey: string; state: "running" | "completed"; threadId: string; turnId: string | null; outcome: string | null }>>
  marketplaces: ReadonlyArray<Readonly<{ name: string; displayName: string | null }>>
  plugins: ReadonlyArray<Readonly<{ id: string; name: string; displayName: string | null; installed: boolean; enabled: boolean; version: string | null; capabilities: ReadonlyArray<string>; policy: CodexEcosystemPolicy }>>
  apps: ReadonlyArray<Readonly<{ id: string; name: string; description: string | null; enabled: boolean; accessible: boolean; auth: "ready" | "required" | "blocked" }>>
  mcpServers: ReadonlyArray<Readonly<{ name: string; title: string | null; version: string | null; auth: string; startup: string; tools: ReadonlyArray<string>; resources: ReadonlyArray<Readonly<{ resourceRef: string; name: string; title: string | null }>> }>>
  oauth: ReadonlyArray<Readonly<{ causalRef: string; name: string; threadId: string | null; state: "pending" | "completed" | "failed"; error: string | null }>>
  policies: Readonly<Record<string, CodexEcosystemPolicy>>
  errors: ReadonlyArray<Readonly<{ method: string; detail: string }>>
}>

export type CodexEcosystemMutation = "skill_config" | "skill_roots" | "marketplace_add" | "marketplace_remove" | "marketplace_upgrade" | "plugin_install" | "plugin_uninstall" | "plugin_share" | "mcp_reload" | "mcp_oauth"
export type CodexEcosystemAuthority = Readonly<{ token: string; kind: CodexEcosystemMutation; expectedRevision: number; workContextRef: string; expiresAt: string }>
export type CodexDynamicTool = Readonly<{ name: string; namespace: string; invoke: (argumentsValue: unknown, causal: Readonly<{ threadId: string | null; turnId: string | null; callId: string | null }>) => Promise<unknown> | unknown }>

export class CodexEcosystemError extends Error {
  readonly _tag = "CodexEcosystemError"
  override readonly name = "CodexEcosystemError"
  constructor(readonly reason: "closed" | "authority_required" | "stale" | "expired" | "reused" | "blocked" | "unknown_extension" | "invalid_response", message: string) { super(message) }
}

export type CodexEcosystem = Readonly<{
  initialize: (cwds?: ReadonlyArray<string>) => Promise<CodexEcosystemSnapshot>
  snapshot: () => CodexEcosystemSnapshot
  subscribe: (listener: (snapshot: CodexEcosystemSnapshot) => void) => () => void
  authorize: (kind: CodexEcosystemMutation, workContextRef: string, expectedRevision: number) => CodexEcosystemAuthority
  configureSkill: (input: Readonly<{ id: string; enabled: boolean }>, authority: CodexEcosystemAuthority) => Promise<void>
  setExtraSkillRoots: (roots: ReadonlyArray<string>, authority: CodexEcosystemAuthority) => Promise<void>
  addMarketplace: (input: unknown, authority: CodexEcosystemAuthority) => Promise<void>
  removeMarketplace: (name: string, authority: CodexEcosystemAuthority) => Promise<void>
  upgradeMarketplace: (name: string | null, authority: CodexEcosystemAuthority) => Promise<void>
  installPlugin: (input: unknown, authority: CodexEcosystemAuthority) => Promise<void>
  uninstallPlugin: (id: string, authority: CodexEcosystemAuthority) => Promise<void>
  listInstalledPlugins: (input?: unknown) => Promise<unknown>
  readPlugin: (input: unknown) => Promise<unknown>
  readPluginSkill: (input: unknown) => Promise<unknown>
  savePluginShare: (input: unknown, authority: CodexEcosystemAuthority) => Promise<unknown>
  updatePluginShareTargets: (input: unknown, authority: CodexEcosystemAuthority) => Promise<unknown>
  listPluginShares: (input?: unknown) => Promise<unknown>
  checkoutPluginShare: (input: unknown, authority: CodexEcosystemAuthority) => Promise<unknown>
  deletePluginShare: (input: unknown, authority: CodexEcosystemAuthority) => Promise<unknown>
  reloadMcp: (authority: CodexEcosystemAuthority) => Promise<void>
  startMcpOauth: (name: string, threadId: string | null, authority: CodexEcosystemAuthority) => Promise<unknown>
  readMcpResource: (server: string, resourceRef: string, threadId?: string | null) => Promise<unknown>
  callMcpTool: (server: string, tool: string, threadId: string, argumentsValue: unknown) => Promise<unknown>
  admitTurnExtensions: (input: Readonly<{ skillIds?: ReadonlyArray<string>; appIds?: ReadonlyArray<string>; pluginIds?: ReadonlyArray<string> }>) => void
  close: () => void
}>

export type CodexEcosystemRegistry = Readonly<{
  forTarget: (target: CodexAppServerPoolTarget) => Promise<CodexEcosystem>
  close: () => void
}>

const initialPolicies = (): Readonly<Record<string, CodexEcosystemPolicy>> => Object.fromEntries(
  CODEX_ECOSYSTEM_METHODS.map(member => [
    member.method,
    "available",
  ]),
)
const initial = (): CodexEcosystemSnapshot => ({ revision: 0, generation: 0, observedAt: new Date(0).toISOString(), skills: [], hooks: [], hookRuns: [], marketplaces: [], plugins: [], apps: [], mcpServers: [], oauth: [], policies: initialPolicies(), errors: [] })

export const makeCodexEcosystem = (options: Readonly<{
  lease: CodexAppServerLease
  now?: () => Date
  authorityTtlMs?: number
  authorizeWorkContext: (workContextRef: string, kind: CodexEcosystemMutation) => boolean
  authorizeRoot?: (root: string, workContextRef: string) => boolean
  authorizeNamespace?: (namespace: string, threadId: string | null) => boolean
  dynamicTools?: ReadonlyArray<CodexDynamicTool>
  onElicitation?: (input: Readonly<{ causalRef: string; server: string | null; threadId: string | null; turnId: string | null; request: unknown }>) => Promise<unknown> | unknown
}>): CodexEcosystem => {
  let state = initial()
  let closed = false
  let cwds: ReadonlyArray<string> = []
  const listeners = new Set<(snapshot: CodexEcosystemSnapshot) => void>()
  const authorities = new Map<string, CodexEcosystemAuthority>()
  const resourceUris = new Map<string, Readonly<{ server: string; uri: string }>>()
  const dynamicTools = new Map((options.dynamicTools ?? []).map(tool => [`${tool.namespace}/${tool.name}`, tool]))
  const now = () => options.now?.() ?? new Date()
  const publish = (patch: Partial<CodexEcosystemSnapshot>, generation = options.lease.state().generation): CodexEcosystemSnapshot => {
    if (generation < state.generation) return state
    state = { ...state, ...patch, revision: state.revision + 1, generation, observedAt: now().toISOString() }
    for (const listener of listeners) listener(state)
    return state
  }
  const assertOpen = () => { if (closed) throw new CodexEcosystemError("closed", "Codex ecosystem is closed") }
  const request = async (method: string, params: unknown): Promise<unknown> => {
    assertOpen()
    try { return await options.lease.request(method, params) }
    catch (error) {
      publish({ errors: [...state.errors, { method, detail: "The app-server ecosystem request failed; private diagnostics retained the original error." }].slice(-128) })
      throw error
    }
  }
  const consume = (kind: CodexEcosystemMutation, authority: CodexEcosystemAuthority) => {
    assertOpen()
    const stored = authorities.get(authority.token)
    if (stored === undefined) throw new CodexEcosystemError("authority_required", "Owner WorkContext authority is required")
    authorities.delete(authority.token)
    if (stored.kind !== kind) throw new CodexEcosystemError("authority_required", "Authority kind mismatch")
    if (Date.parse(stored.expiresAt) < now().getTime()) throw new CodexEcosystemError("expired", "Authority expired")
    if (stored.expectedRevision !== state.revision) throw new CodexEcosystemError("stale", "Ecosystem state changed")
    if (!options.authorizeWorkContext(stored.workContextRef, kind)) throw new CodexEcosystemError("blocked", "WorkContext policy denied this mutation")
  }
  const assertMutationPaths = (input: unknown, authority: CodexEcosystemAuthority): void => {
    const visit = (value: unknown, key = "") => {
      if (typeof value === "string" && /(?:path|root|source)$/iu.test(key) && isAbsolute(value) && options.authorizeRoot?.(value, authority.workContextRef) !== true) throw new CodexEcosystemError("blocked", "Extension path is outside WorkContext authority")
      if (Array.isArray(value)) for (const item of value) visit(item, key)
      else if (object(value) !== null) for (const [childKey, child] of Object.entries(object(value)!)) visit(child, childKey)
    }
    visit(input)
  }
  const projectSkills = (raw: unknown) => array(object(raw)?.data).flatMap(group => array(object(group)?.skills).flatMap(value => {
    const row = object(value); const name = string(row?.name); if (row === null || name === null) return []
    return [{ id: hash(`${string(row.path) ?? name}:${name}`).slice(0, 24), name, description: string(row.description) ?? "", scope: string(row.scope) ?? "unknown", enabled: bool(row.enabled), dependencies: array(object(row.dependencies)?.tools).flatMap(dep => string(object(dep)?.value) ?? []).slice(0, 64) }]
  }))
  const projectHooks = (raw: unknown) => array(object(raw)?.data).flatMap(group => array(object(group)?.hooks).flatMap(value => {
    const row = object(value); const key = string(row?.key); if (row === null || key === null) return []
    return [{ key, event: string(row.eventName) ?? "unknown", type: string(row.handlerType) ?? "unknown", enabled: bool(row.enabled), trust: string(row.trustStatus) ?? "unknown", source: string(row.source) ?? "unknown" }]
  }))
  const projectPlugins = (raw: unknown) => {
    const marketplaces = array(object(raw)?.marketplaces)
    return {
      marketplaces: marketplaces.flatMap(value => { const row = object(value); const name = string(row?.name); return row === null || name === null ? [] : [{ name, displayName: string(object(row.interface)?.displayName) }] }),
      plugins: marketplaces.flatMap(marketplace => array(object(marketplace)?.plugins).flatMap(value => {
        const row = object(value); const id = string(row?.id); const name = string(row?.name); if (row === null || id === null || name === null) return []
        const availability = string(row.availability); const installPolicy = string(row.installPolicy)
        return [{ id, name, displayName: string(object(row.interface)?.displayName), installed: bool(row.installed), enabled: bool(row.enabled), version: string(row.version) ?? string(row.localVersion), capabilities: array(object(row.interface)?.capabilities).flatMap(stringValue => string(stringValue) ?? []), policy: availability === "DISABLED_BY_ADMIN" ? "blocked" as const : installPolicy === "NOT_AVAILABLE" ? "under_development" as const : "available" as const }]
      })),
    }
  }
  const projectApps = (raw: unknown) => array(object(raw)?.data).flatMap(value => {
    const row = object(value); const id = string(row?.id); const name = string(row?.name); if (row === null || id === null || name === null) return []
    const accessible = row.isAccessible !== false; const enabled = row.isEnabled !== false
    return [{ id, name, description: string(row.description), enabled, accessible, auth: !accessible ? "blocked" as const : enabled ? "ready" as const : "required" as const }]
  })
  const projectMcp = (raw: unknown) => array(object(raw)?.data).flatMap(value => {
    const row = object(value); const name = string(row?.name); if (row === null || name === null) return []
    const resources = array(row.resources).flatMap(resource => {
      const item = object(resource); const uri = string(item?.uri); const resourceName = string(item?.name); if (item === null || uri === null || resourceName === null) return []
      const resourceRef = hash(`${name}:${uri}`).slice(0, 32); resourceUris.set(resourceRef, { server: name, uri })
      return [{ resourceRef, name: resourceName, title: string(item.title) }]
    })
    return [{ name, title: string(object(row.serverInfo)?.title), version: string(object(row.serverInfo)?.version), auth: string(row.authStatus) ?? "unknown", startup: "ready", tools: Object.keys(object(row.tools) ?? {}).slice(0, 256), resources }]
  })
  const refreshSkills = async (forceReload = false) => publish({ skills: projectSkills(await request("skills/list", { cwds, forceReload })) })
  const refreshHooks = async () => publish({ hooks: projectHooks(await request("hooks/list", { cwds })) })
  const refreshPlugins = async () => { const next = projectPlugins(await request("plugin/list", { cwds })); publish(next) }
  const refreshApps = async (forceRefetch = false) => publish({ apps: projectApps(await request("app/list", { forceRefetch })) })
  const refreshMcp = async () => publish({ mcpServers: projectMcp(await request("mcpServerStatus/list", { detail: "full" })) })
  const reconcileInstall = async () => { await refreshPlugins(); await refreshApps(true); await refreshMcp() }
  const onNotification = ({ generation, message }: CodexAppServerNotification) => {
    const params = object(message.params) ?? {}
    if (message.method === "skills/changed") { void refreshSkills(true).catch(() => undefined); return }
    if (message.method === "app/list/updated") { publish({ apps: projectApps(params) }, generation); return }
    if (message.method === "mcpServer/startupStatus/updated") {
      const name = string(params.name); if (name === null) return
      publish({ mcpServers: state.mcpServers.map(server => server.name === name ? { ...server, startup: string(params.status) ?? "unknown" } : server) }, generation); return
    }
    if (message.method === "mcpServer/oauthLogin/completed") {
      const name = string(params.name); const threadId = string(params.threadId)
      publish({ oauth: state.oauth.map(flow => flow.name === name && flow.threadId === threadId && flow.state === "pending" ? { ...flow, state: params.success === true ? "completed" as const : "failed" as const, error: params.success === true ? null : "Connector authentication failed" } : flow) }, generation); void refreshMcp().catch(() => undefined); return
    }
    if (message.method === "hook/started" || message.method === "hook/completed") {
      const run = object(params.run); const runRef = string(run?.id) ?? hash(JSON.stringify(params)).slice(0, 24); const hookKey = string(run?.hookName) ?? string(run?.hookKey) ?? "unknown"
      const next = { runRef, hookKey, state: message.method === "hook/started" ? "running" as const : "completed" as const, threadId: string(params.threadId) ?? "unknown", turnId: string(params.turnId), outcome: message.method === "hook/completed" ? string(run?.status) ?? "completed" : null }
      publish({ hookRuns: [...state.hookRuns.filter(value => value.runRef !== runRef), next].slice(-256) }, generation)
    }
  }
  const removeNotifications = options.lease.subscribe(onNotification)
  const removeReverse = options.lease.registerReverseHandler(async (requestValue: CodexAppServerRequest) => {
    const params = object(requestValue.params) ?? {}
    const threadId = string(params.threadId); const turnId = string(params.turnId); const callId = string(params.callId)
    if (requestValue.method === "mcpServer/elicitation/request") {
      const causalRef = hash(JSON.stringify([requestValue.id, threadId, turnId, callId, string(params.serverName)])).slice(0, 32)
      if (options.onElicitation === undefined) return { action: "decline", content: null, _meta: null }
      return options.onElicitation({ causalRef, server: string(params.serverName) ?? string(params.server), threadId, turnId, request: params.request ?? params })
    }
    if (requestValue.method === "item/tool/call") {
      const namespace = string(params.namespace) ?? string(params.tool)?.split("/")[0] ?? ""
      const name = string(params.name) ?? string(params.tool)?.split("/").slice(1).join("/") ?? ""
      const tool = dynamicTools.get(`${namespace}/${name}`)
      if (tool === undefined || options.authorizeNamespace?.(namespace, threadId) !== true) return { contentItems: [], success: false }
      return { contentItems: [{ type: "text", text: JSON.stringify(await tool.invoke(params.arguments, { threadId, turnId, callId })).slice(0, 20_000) }], success: true }
    }
    throw new CodexEcosystemError("blocked", "Ecosystem handler does not own this reverse request")
  })
  return {
    initialize: async roots => {
      assertOpen(); cwds = [...new Set((roots ?? []).filter(value => value.length > 0))]
      await refreshSkills(true); await refreshHooks(); await refreshPlugins(); await refreshApps(true); await refreshMcp()
      return state
    },
    snapshot: () => state,
    subscribe: listener => { listeners.add(listener); return () => listeners.delete(listener) },
    authorize: (kind, workContextRef, expectedRevision) => {
      assertOpen(); if (!options.authorizeWorkContext(workContextRef, kind)) throw new CodexEcosystemError("blocked", "WorkContext policy denied authority")
      const authority = { token: randomUUID(), kind, expectedRevision, workContextRef, expiresAt: new Date(now().getTime() + Math.max(1, options.authorityTtlMs ?? 60_000)).toISOString() }
      authorities.set(authority.token, authority); return authority
    },
    configureSkill: async (input, authority) => { consume("skill_config", authority); const skill = state.skills.find(value => value.id === input.id); if (skill === undefined) throw new CodexEcosystemError("unknown_extension", "Unknown skill"); await request("skills/config/write", { name: skill.name, enabled: input.enabled }); await refreshSkills(true) },
    setExtraSkillRoots: async (roots, authority) => { consume("skill_roots", authority); if (roots.some(root => options.authorizeRoot?.(root, authority.workContextRef) !== true)) throw new CodexEcosystemError("blocked", "An extra skill root is outside WorkContext authority"); await request("skills/extraRoots/set", { extraRoots: roots }); await refreshSkills(true) },
    addMarketplace: async (input, authority) => { consume("marketplace_add", authority); assertMutationPaths(input, authority); await request("marketplace/add", input); await refreshPlugins() },
    removeMarketplace: async (name, authority) => { consume("marketplace_remove", authority); await request("marketplace/remove", { marketplaceName: name }); await refreshPlugins() },
    upgradeMarketplace: async (name, authority) => { consume("marketplace_upgrade", authority); await request("marketplace/upgrade", { marketplaceName: name }); await refreshPlugins() },
    installPlugin: async (input, authority) => { consume("plugin_install", authority); assertMutationPaths(input, authority); await request("plugin/install", input); await reconcileInstall() },
    uninstallPlugin: async (id, authority) => { consume("plugin_uninstall", authority); await request("plugin/uninstall", { pluginId: id }); await reconcileInstall() },
    listInstalledPlugins: input => request("plugin/installed", input ?? { cwds }),
    readPlugin: input => request("plugin/read", input),
    readPluginSkill: input => request("plugin/skill/read", input),
    savePluginShare: async (input, authority) => { consume("plugin_share", authority); assertMutationPaths(input, authority); const result = await request("plugin/share/save", input); await refreshPlugins(); return result },
    updatePluginShareTargets: async (input, authority) => { consume("plugin_share", authority); const result = await request("plugin/share/updateTargets", input); await refreshPlugins(); return result },
    listPluginShares: input => request("plugin/share/list", input ?? {}),
    checkoutPluginShare: async (input, authority) => { consume("plugin_share", authority); const result = await request("plugin/share/checkout", input); await reconcileInstall(); return result },
    deletePluginShare: async (input, authority) => { consume("plugin_share", authority); const result = await request("plugin/share/delete", input); await refreshPlugins(); return result },
    reloadMcp: async authority => { consume("mcp_reload", authority); await request("config/mcpServer/reload", undefined); await refreshMcp() },
    startMcpOauth: async (name, threadId, authority) => { consume("mcp_oauth", authority); const causalRef = `oauth.${randomUUID()}`; publish({ oauth: [...state.oauth, { causalRef, name, threadId, state: "pending" as const, error: null }].slice(-128) }); return request("mcpServer/oauth/login", { name, threadId }) },
    readMcpResource: async (server, resourceRef, threadId = null) => { const resource = resourceUris.get(resourceRef); if (resource === undefined || resource.server !== server) throw new CodexEcosystemError("unknown_extension", "Unknown MCP resource"); return request("mcpServer/resource/read", { server, uri: resource.uri, threadId }) },
    callMcpTool: async (server, tool, threadId, argumentsValue) => { const catalog = state.mcpServers.find(value => value.name === server); if (catalog === undefined || !catalog.tools.includes(tool)) throw new CodexEcosystemError("unknown_extension", "Undeclared MCP tool"); return request("mcpServer/tool/call", { server, tool, threadId, arguments: argumentsValue }) },
    admitTurnExtensions: input => {
      for (const id of input.skillIds ?? []) if (!state.skills.some(value => value.id === id && value.enabled)) throw new CodexEcosystemError("unknown_extension", `Skill ${id} is not reconciled and enabled`)
      for (const id of input.appIds ?? []) if (!state.apps.some(value => value.id === id && value.enabled && value.accessible)) throw new CodexEcosystemError("unknown_extension", `App ${id} is not authorized`)
      for (const id of input.pluginIds ?? []) if (!state.plugins.some(value => value.id === id && value.installed && value.enabled && value.policy === "available")) throw new CodexEcosystemError("unknown_extension", `Plugin ${id} is not authorized`)
    },
    close: () => { if (closed) return; closed = true; removeNotifications(); removeReverse(); authorities.clear(); listeners.clear(); options.lease.release() },
  }
}

export const makeCodexEcosystemRegistry = (options: Readonly<{
  supervisor: CodexAppServerSupervisor
  roots: () => ReadonlyArray<string>
  authorizeWorkContext: (workContextRef: string, kind: CodexEcosystemMutation) => boolean
  authorizeRoot?: (root: string, workContextRef: string) => boolean
  authorizeNamespace?: (namespace: string, threadId: string | null) => boolean
  dynamicTools?: ReadonlyArray<CodexDynamicTool>
  onElicitation?: (input: Readonly<{ causalRef: string; server: string | null; threadId: string | null; turnId: string | null; request: unknown }>) => Promise<unknown> | unknown
}>): CodexEcosystemRegistry => {
  const entries = new Map<string, Promise<CodexEcosystem>>()
  let closed = false
  return {
    forTarget: target => {
      if (closed) return Promise.reject(new CodexEcosystemError("closed", "Codex ecosystem registry is closed"))
      const key = codexAppServerPoolKey(target); const existing = entries.get(key); if (existing !== undefined) return existing
      const created = options.supervisor.acquire(target).then(async lease => {
        const ecosystem = makeCodexEcosystem({
          lease,
          authorizeWorkContext: options.authorizeWorkContext,
          ...(options.authorizeRoot === undefined ? {} : { authorizeRoot: options.authorizeRoot }),
          ...(options.authorizeNamespace === undefined ? {} : { authorizeNamespace: options.authorizeNamespace }),
          ...(options.dynamicTools === undefined ? {} : { dynamicTools: options.dynamicTools }),
          ...(options.onElicitation === undefined ? {} : { onElicitation: options.onElicitation }),
        })
        try { await ecosystem.initialize(options.roots()); return ecosystem }
        catch (error) { ecosystem.close(); entries.delete(key); throw error }
      })
      entries.set(key, created); return created
    },
    close: () => { if (closed) return; closed = true; for (const entry of entries.values()) void entry.then(value => value.close(), () => undefined); entries.clear() },
  }
}
