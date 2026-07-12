/**
 * Unified extension lifecycle tests — CUT-23 (#8703).
 *
 * Deterministic coverage of the CUT-23 boundary classes over the PURE
 * derived projection: invalid config, revoked grants, duplicate names,
 * provider disagreement, and offline/unavailable (partial) state — plus the
 * per-item grant scoping rule that a skill's grant lives under its parent
 * plugin's grant and is revoked with it.
 */
import { describe, expect, test } from "bun:test"

import {
  decodeExtensionLifecycleAudit,
  mcpServerLifecycleEntry,
  pluginLifecycleEntry,
  skillLifecycleEntries,
  unifiedExtensionLifecycle,
  type ExtensionLifecycleEntry,
} from "./extension-lifecycle-contract.ts"
import type { McpConfigServerView } from "./mcp-config-contract.ts"
import type { PluginConfigView } from "./plugin-config-contract.ts"

const mcpServer = (over: Partial<McpConfigServerView> = {}): McpConfigServerView => ({
  name: "search",
  transport: "stdio",
  enabled: true,
  command: "search-mcp",
  argsCount: 0,
  envCount: 2,
  headersCount: 0,
  ...over,
})

const plugin = (over: Partial<PluginConfigView> = {}): PluginConfigView => ({
  ref: "plugin.local.0123456789abcdef01234567",
  name: "review-tools",
  provider: "claude_agent",
  provenance: "user_local",
  scope: "app",
  readiness: "ready",
  enabled: true,
  restartRequired: false,
  perSessionUse: "next_turn",
  capabilities: ["skills"],
  skills: ["review"],
  ...over,
})

describe("per-kind lifecycle projection", () => {
  test("enabled MCP server is granted with an active next-turn app grant", () => {
    const entry = mcpServerLifecycleEntry(mcpServer())
    expect(entry.stage).toBe("granted")
    expect(entry.grant).toEqual({ state: "active", use: "next_turn", scope: "app" })
    expect(entry.kind).toBe("mcp_server")
    expect(entry.provenance).toBe("user_local")
  })

  test("disabled MCP server is revoked — the grant is withdrawn, not deleted", () => {
    const entry = mcpServerLifecycleEntry(mcpServer({ enabled: false }))
    expect(entry.stage).toBe("revoked")
    expect(entry.grant.state).toBe("revoked")
  })

  test("invalid plugin is never grantable regardless of its enabled flag", () => {
    const entry = pluginLifecycleEntry(plugin({ readiness: "invalid", enabled: true }))
    expect(entry.stage).toBe("invalid")
    expect(entry.grant.state).toBe("blocked")
  })

  test("missing plugin directory is declared (registered, not runnable)", () => {
    const entry = pluginLifecycleEntry(plugin({ readiness: "missing" }))
    expect(entry.stage).toBe("declared")
    expect(entry.grant.state).toBe("blocked")
  })

  test("skill grants are scoped under the parent plugin grant", () => {
    const granted = skillLifecycleEntries(plugin())
    expect(granted).toHaveLength(1)
    expect(granted[0]?.stage).toBe("granted")
    // Even active skill grants require the explicit /skill invocation.
    expect(granted[0]?.grant).toEqual({
      state: "active",
      use: "explicit_invocation",
      scope: "app",
    })
    expect(granted[0]?.id).toBe("plugin.local.0123456789abcdef01234567/review")
    expect(granted[0]?.label).toBe("review-tools/review")
  })

  test("revoking the plugin revokes every skill under it in the same projection", () => {
    const revoked = skillLifecycleEntries(plugin({ enabled: false }))
    expect(revoked[0]?.stage).toBe("revoked")
    expect(revoked[0]?.grant.state).toBe("revoked")
    const blocked = skillLifecycleEntries(plugin({ readiness: "invalid" }))
    expect(blocked[0]?.stage).toBe("invalid")
    expect(blocked[0]?.grant.state).toBe("blocked")
  })

  test("provider disagreement is explicit: every kind is Claude-supported, Codex-unsupported", () => {
    const entries: ReadonlyArray<ExtensionLifecycleEntry> = [
      mcpServerLifecycleEntry(mcpServer()),
      pluginLifecycleEntry(plugin()),
      ...skillLifecycleEntries(plugin()),
    ]
    for (const entry of entries) {
      expect(entry.providerSupport).toEqual({ claude_agent: "supported", codex: "unsupported" })
    }
  })
})

describe("unified audit", () => {
  test("composes all kinds with deterministic ordering and grant tallies", () => {
    const audit = unifiedExtensionLifecycle({
      mcpServers: [mcpServer({ name: "zeta", enabled: false }), mcpServer({ name: "alpha" })],
      mcpDropped: 1,
      plugins: [plugin()],
      pluginsDropped: 0,
    })
    expect(audit.entries.map((entry) => `${entry.kind}:${entry.label}`)).toEqual([
      "mcp_server:alpha",
      "mcp_server:zeta",
      "plugin:review-tools",
      "skill:review-tools/review",
    ])
    expect(audit.granted).toBe(3)
    expect(audit.revoked).toBe(1)
    expect(audit.blocked).toBe(0)
    expect(audit.droppedInvalid).toEqual({ mcpServers: 1, plugins: 0 })
    expect(audit.partial).toBe(false)
    // The audit round-trips its own schema (the typed contract holds).
    expect(decodeExtensionLifecycleAudit(JSON.parse(JSON.stringify(audit)))).toEqual(audit)
  })

  test("flags cross-kind duplicate labels without hiding either entry", () => {
    const audit = unifiedExtensionLifecycle({
      mcpServers: [mcpServer({ name: "review-tools" })],
      mcpDropped: 0,
      plugins: [plugin()],
      pluginsDropped: 0,
    })
    const dupes = audit.entries.filter((entry) => entry.duplicateLabel)
    expect(dupes.map((entry) => entry.kind).sort()).toEqual(["mcp_server", "plugin"])
    // The skill's compound label does not collide.
    expect(
      audit.entries.find((entry) => entry.kind === "skill")?.duplicateLabel,
    ).toBe(false)
  })

  test("an unavailable registry yields an honest partial audit, never an empty-complete one", () => {
    const audit = unifiedExtensionLifecycle({
      mcpServers: null,
      mcpDropped: 0,
      plugins: [plugin({ enabled: false })],
      pluginsDropped: 2,
    })
    expect(audit.partial).toBe(true)
    expect(audit.entries.map((entry) => entry.kind)).toEqual(["plugin", "skill"])
    expect(audit.revoked).toBe(2)
    expect(audit.droppedInvalid.plugins).toBe(2)
    const bothDown = unifiedExtensionLifecycle({
      mcpServers: null,
      mcpDropped: 0,
      plugins: null,
      pluginsDropped: 0,
    })
    expect(bothDown.partial).toBe(true)
    expect(bothDown.entries).toEqual([])
  })

  test("host-dropped invalid rows surface as counts (invalid-config class)", () => {
    const audit = unifiedExtensionLifecycle({
      mcpServers: [],
      mcpDropped: 3,
      plugins: [],
      pluginsDropped: 1,
    })
    expect(audit.droppedInvalid).toEqual({ mcpServers: 3, plugins: 1 })
    // Negative counts can never sneak in from a corrupted source.
    const clamped = unifiedExtensionLifecycle({
      mcpServers: [],
      mcpDropped: -2,
      plugins: [],
      pluginsDropped: -1,
    })
    expect(clamped.droppedInvalid).toEqual({ mcpServers: 0, plugins: 0 })
  })

  test("no secret-bearing fields exist anywhere in the audit projection", () => {
    const audit = unifiedExtensionLifecycle({
      mcpServers: [mcpServer({ envCount: 4, headersCount: 2 })],
      mcpDropped: 0,
      plugins: [plugin()],
      pluginsDropped: 0,
    })
    const serialized = JSON.stringify(audit)
    // The projection carries names/refs/stages only — never env, headers,
    // args, commands, urls, or absolute paths from the source views.
    expect(serialized).not.toContain("env")
    expect(serialized).not.toContain("header")
    expect(serialized).not.toContain("command")
    expect(serialized).not.toContain("url")
    expect(serialized).not.toContain("/Users/")
  })
})
