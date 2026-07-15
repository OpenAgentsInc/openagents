import { describe, expect, test } from "vite-plus/test"

import type { CodexAppServerRequest } from "./codex-app-server-client.ts"
import type { CodexAppServerLease, CodexAppServerNotification } from "./codex-app-server-supervisor.ts"
import { CODEX_ECOSYSTEM_METHODS, CodexEcosystemError, makeCodexEcosystem } from "./codex-ecosystem.ts"

const fixture = () => {
  const requests: Array<Readonly<{ method: string; params: unknown }>> = []
  const notifications = new Set<(value: CodexAppServerNotification) => void>()
  const reverseHandlers = new Set<(request: CodexAppServerRequest) => Promise<unknown> | unknown>()
  let installed = false
  let skillEnabled = true
  const respond = (method: string): unknown => {
    if (method === "skills/list") return { data: [{ cwd: "/private/project", errors: [], skills: [{ name: "ship", description: "Ship safely", enabled: skillEnabled, path: "/private/project/.agents/ship/SKILL.md", scope: "repo", dependencies: { tools: [{ type: "mcp", value: "safe-tool", url: "https://token@example.test" }] } }] }] }
    if (method === "hooks/list") return { data: [{ cwd: "/private/project", errors: [], warnings: [], hooks: [{ key: "verify", eventName: "postToolUse", handlerType: "command", enabled: true, trustStatus: "trusted", source: "project", sourcePath: "/private/project/hook", currentHash: "secret", displayOrder: 1, isManaged: false, timeoutSec: 10 }] }] }
    if (method === "plugin/list") return { marketplaces: [{ name: "official", path: "/private/market", interface: { displayName: "Official" }, plugins: [{ id: "plugin-1", name: "connector", installed, enabled: installed, version: "1.0.0", authPolicy: "ON_USE", installPolicy: "AVAILABLE", source: { type: "local", path: "/private/plugin" }, interface: { displayName: "Connector", capabilities: ["apps", "mcp"], logo: "/private/logo" } }] }] }
    if (method === "app/list") return { data: installed ? [{ id: "app-1", name: "Drive", description: "Files", isEnabled: true, isAccessible: true }] : [] }
    if (method === "mcpServerStatus/list") return { data: [{ name: "files", authStatus: installed ? "oAuth" : "notLoggedIn", serverInfo: { title: "Files", version: "1" }, resources: [{ name: "home", title: "Home", uri: "file:///private/home" }], resourceTemplates: [], tools: { search: { name: "search", inputSchema: {}, description: "Search" } } }] }
    if (method === "plugin/install") { installed = true; return {} }
    if (method === "plugin/uninstall") { installed = false; return {} }
    if (method === "skills/config/write") { skillEnabled = false; return {} }
    if (method === "mcpServer/resource/read") return { contents: [{ text: "private payload" }] }
    if (method === "mcpServer/tool/call") return { content: [{ type: "text", text: "ok" }] }
    if (method === "mcpServer/oauth/login") return { authorizationUrl: "https://auth.example.test" }
    return {}
  }
  const lease = {
    state: () => ({ status: "ready" as const, generation: 1 }),
    request: async (method: string, params: unknown) => { requests.push({ method, params }); return respond(method) },
    subscribe: (listener: (value: CodexAppServerNotification) => void) => { notifications.add(listener); return () => notifications.delete(listener) },
    registerReverseHandler: (handler: (request: CodexAppServerRequest) => Promise<unknown> | unknown) => { reverseHandlers.add(handler); return () => reverseHandlers.delete(handler) },
    release: () => undefined,
  } as unknown as CodexAppServerLease
  return {
    lease,
    requests,
    notify: (method: string, params: unknown) => { for (const listener of notifications) listener({ generation: 1, message: { method, params } }) },
    reverse: async (request: CodexAppServerRequest) => {
      const handler = [...reverseHandlers][0]
      if (handler === undefined) throw new Error("missing reverse handler")
      return handler(request)
    },
  }
}

describe("Codex ecosystem authority", () => {
  test("covers every generated ecosystem member with explicit policy", async () => {
    const h = fixture(); const ecosystem = makeCodexEcosystem({ lease: h.lease, authorizeWorkContext: () => true })
    const snapshot = await ecosystem.initialize(["/private/project"])
    expect(Object.keys(snapshot.policies).sort()).toEqual(CODEX_ECOSYSTEM_METHODS.map(value => value.method).sort())
    expect(snapshot.policies["plugin/share/save"]).toBe("available")
    expect(snapshot.skills[0]).toMatchObject({ name: "ship", enabled: true, dependencies: ["safe-tool"] })
    expect(snapshot.hooks[0]).toMatchObject({ key: "verify", trust: "trusted" })
    expect(JSON.stringify(snapshot)).not.toContain("/private/")
    expect(JSON.stringify(snapshot)).not.toContain("token@example")
    ecosystem.close()
  })

  test("consumes WorkContext authority and reconciles plugin-created app and MCP catalogs", async () => {
    const h = fixture(); const ecosystem = makeCodexEcosystem({ lease: h.lease, authorizeWorkContext: ref => ref === "work-1" })
    await ecosystem.initialize(["/private/project"])
    const authority = ecosystem.authorize("plugin_install", "work-1", ecosystem.snapshot().revision)
    await ecosystem.installPlugin({ pluginName: "connector" }, authority)
    expect(ecosystem.snapshot().plugins[0]).toMatchObject({ installed: true, enabled: true })
    expect(ecosystem.snapshot().apps).toMatchObject([{ id: "app-1", auth: "ready" }])
    expect(ecosystem.snapshot().mcpServers[0]).toMatchObject({ name: "files", auth: "oAuth", tools: ["search"] })
    expect(h.requests.slice(-4).map(value => value.method)).toEqual(["plugin/install", "plugin/list", "app/list", "mcpServerStatus/list"])
    await expect(ecosystem.installPlugin({ pluginName: "again" }, authority)).rejects.toMatchObject({ reason: "authority_required" })
    ecosystem.admitTurnExtensions({ appIds: ["app-1"], pluginIds: ["plugin-1"] })
    expect(() => ecosystem.admitTurnExtensions({ appIds: ["phantom"] })).toThrow(CodexEcosystemError)
    ecosystem.close()
    const restarted = makeCodexEcosystem({ lease: h.lease, authorizeWorkContext: () => true })
    await restarted.initialize(["/private/project"])
    expect(restarted.snapshot().plugins[0]).toMatchObject({ installed: true, enabled: true })
    expect(restarted.snapshot().apps.map(value => value.id)).toEqual(["app-1"])
    restarted.close()
  })

  test("covers extra roots, plugin read/share operations, and hook/app invalidations", async () => {
    const h = fixture(); const ecosystem = makeCodexEcosystem({ lease: h.lease, authorizeWorkContext: () => true, authorizeRoot: root => root.startsWith("/private/project/") })
    await ecosystem.initialize(["/private/project"])
    const roots = ecosystem.authorize("skill_roots", "work", ecosystem.snapshot().revision)
    await ecosystem.setExtraSkillRoots(["/private/project/skills"], roots)
    expect(h.requests.find(value => value.method === "skills/extraRoots/set")?.params).toEqual({ extraRoots: ["/private/project/skills"] })
    const blocked = ecosystem.authorize("skill_roots", "work", ecosystem.snapshot().revision)
    await expect(ecosystem.setExtraSkillRoots(["/outside"], blocked)).rejects.toMatchObject({ reason: "blocked" })
    await ecosystem.listInstalledPlugins(); await ecosystem.readPlugin({ pluginName: "connector" }); await ecosystem.readPluginSkill({ pluginName: "connector", skillName: "ship" }); await ecosystem.listPluginShares()
    for (const [method, invoke] of [
      ["plugin/share/save", (authority: ReturnType<typeof ecosystem.authorize>) => ecosystem.savePluginShare({ pluginPath: "/private/project/plugin" }, authority)],
      ["plugin/share/updateTargets", (authority: ReturnType<typeof ecosystem.authorize>) => ecosystem.updatePluginShareTargets({ remotePluginId: "remote-1", discoverability: "PRIVATE", shareTargets: [] }, authority)],
      ["plugin/share/checkout", (authority: ReturnType<typeof ecosystem.authorize>) => ecosystem.checkoutPluginShare({ remotePluginId: "remote-1" }, authority)],
      ["plugin/share/delete", (authority: ReturnType<typeof ecosystem.authorize>) => ecosystem.deletePluginShare({ remotePluginId: "remote-1" }, authority)],
    ] as const) {
      await invoke(ecosystem.authorize("plugin_share", "work", ecosystem.snapshot().revision))
      expect(h.requests.some(value => value.method === method)).toBe(true)
    }
    h.notify("hook/started", { threadId: "thread-1", turnId: "turn-1", run: { id: "run-1", hookName: "verify" } })
    h.notify("hook/completed", { threadId: "thread-1", turnId: "turn-1", run: { id: "run-1", hookName: "verify", status: "success" } })
    expect(ecosystem.snapshot().hookRuns).toMatchObject([{ runRef: "run-1", state: "completed", outcome: "success" }])
    h.notify("app/list/updated", { data: [{ id: "app-notified", name: "Calendar", isEnabled: false, isAccessible: true }] })
    expect(ecosystem.snapshot().apps).toMatchObject([{ id: "app-notified", auth: "required" }])
    ecosystem.close()
  })

  test("reconciles changes, resumes exact OAuth cause, and gates MCP resources/tools", async () => {
    const h = fixture(); const ecosystem = makeCodexEcosystem({ lease: h.lease, authorizeWorkContext: () => true })
    await ecosystem.initialize(["/private/project"])
    const oauth = ecosystem.authorize("mcp_oauth", "work", ecosystem.snapshot().revision)
    await ecosystem.startMcpOauth("files", "thread-1", oauth)
    h.notify("mcpServer/oauthLogin/completed", { name: "other", threadId: "thread-1", success: true })
    expect(ecosystem.snapshot().oauth[0]?.state).toBe("pending")
    h.notify("mcpServer/oauthLogin/completed", { name: "files", threadId: "thread-1", success: true })
    await new Promise(resolve => setTimeout(resolve, 0))
    expect(ecosystem.snapshot().oauth[0]?.state).toBe("completed")
    const resource = ecosystem.snapshot().mcpServers[0]!.resources[0]!
    await expect(ecosystem.readMcpResource("files", resource.resourceRef, "thread-1")).resolves.toMatchObject({ contents: expect.any(Array) })
    await expect(ecosystem.callMcpTool("files", "unknown", "thread-1", {})).rejects.toMatchObject({ reason: "unknown_extension" })
    await expect(ecosystem.callMcpTool("files", "search", "thread-1", { q: "x" })).resolves.toMatchObject({ content: expect.any(Array) })
    ecosystem.close()
  })

  test("dynamic tools require declared namespace grants and elicitations retain causal identity", async () => {
    const h = fixture(); const seen: string[] = []
    const ecosystem = makeCodexEcosystem({
      lease: h.lease,
      authorizeWorkContext: () => true,
      authorizeNamespace: namespace => namespace === "declared",
      dynamicTools: [{ namespace: "declared", name: "echo", invoke: value => ({ value }) }],
      onElicitation: input => { seen.push(input.causalRef); return { action: "accept", content: { answer: "yes" }, _meta: null } },
    })
    expect(await h.reverse({ id: 1, method: "item/tool/call", params: { threadId: "thread-1", turnId: "turn-1", callId: "call-1", namespace: "declared", name: "echo", arguments: { x: 1 } } })).toMatchObject({ success: true })
    expect(await h.reverse({ id: 2, method: "item/tool/call", params: { threadId: "thread-1", namespace: "undeclared", name: "echo" } })).toEqual({ contentItems: [], success: false })
    expect(await h.reverse({ id: 3, method: "mcpServer/elicitation/request", params: { threadId: "thread-1", turnId: "turn-1", server: "files", request: { message: "Continue?" } } })).toMatchObject({ action: "accept" })
    expect(seen[0]).toMatch(/^[a-f0-9]{32}$/u)
    ecosystem.close()
  })
})
