import { createHash } from "node:crypto"
import { chmodSync, existsSync, mkdirSync, readFileSync, readdirSync, renameSync, statSync, writeFileSync } from "node:fs"
import path from "node:path"

import type { PluginConfigResult, PluginConfigView, PluginRef } from "./plugin-config-contract.ts"

type StoredPlugin = Readonly<{ ref: PluginRef; path: string; name: string; enabled: boolean }>
const OWNER_ONLY = 0o600
const LIMIT = 24

const pluginRef = (pluginPath: string): PluginRef =>
  `plugin.local.${createHash("sha256").update(path.resolve(pluginPath)).digest("hex").slice(0, 24)}` as PluginRef

const safeName = (pluginPath: string): string => path.basename(path.resolve(pluginPath)).slice(0, 80) || "Local plugin"

const readiness = (pluginPath: string): PluginConfigView["readiness"] => {
  try {
    if (!existsSync(pluginPath) || !statSync(pluginPath).isDirectory()) return "missing"
    return existsSync(path.join(pluginPath, ".claude-plugin", "plugin.json")) ? "ready" : "invalid"
  } catch {
    return "missing"
  }
}

const skillNames = (pluginPath: string): ReadonlyArray<string> => {
  try {
    return readdirSync(path.join(pluginPath, "skills"), { withFileTypes: true })
      .filter(entry => entry.isDirectory() && /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/.test(entry.name) && existsSync(path.join(pluginPath, "skills", entry.name, "SKILL.md")))
      .map(entry => entry.name).sort().slice(0, 64)
  } catch { return [] }
}
const view = (plugin: StoredPlugin): PluginConfigView => ({
  ref: plugin.ref,
  name: plugin.name,
  provider: "claude_agent",
  provenance: "user_local",
  scope: "app",
  readiness: readiness(plugin.path),
  enabled: plugin.enabled,
  restartRequired: false,
  perSessionUse: "next_turn",
  capabilities: ["commands", "agents", "skills", "hooks", "mcp"],
  skills: skillNames(plugin.path),
})

export type PluginConfigStore = Readonly<{
  list: () => PluginConfigResult
  addPath: (pluginPath: string) => PluginConfigResult
  toggle: (ref: PluginRef, enabled: boolean) => PluginConfigResult
  remove: (ref: PluginRef) => PluginConfigResult
  enabledPaths: () => ReadonlyArray<string>
  resolveSkill: (pluginRef: PluginRef, name: string) => Readonly<{ pluginPath: string; name: string }> | null
}>

export const openPluginConfigStore = (filePath: string): PluginConfigStore => {
  const read = (): { plugins: StoredPlugin[]; dropped: number } => {
    let raw: unknown
    try { raw = JSON.parse(readFileSync(filePath, "utf8")) } catch { return { plugins: [], dropped: 0 } }
    const rows = typeof raw === "object" && raw !== null && Array.isArray((raw as { plugins?: unknown }).plugins)
      ? (raw as { plugins: unknown[] }).plugins : []
    const plugins: StoredPlugin[] = []
    const seen = new Set<string>()
    let dropped = 0
    for (const row of rows.slice(0, LIMIT)) {
      if (typeof row !== "object" || row === null) { dropped += 1; continue }
      const value = row as Record<string, unknown>
      if (typeof value.path !== "string" || typeof value.name !== "string" || typeof value.enabled !== "boolean") {
        dropped += 1; continue
      }
      const ref = pluginRef(value.path)
      if (seen.has(ref)) { dropped += 1; continue }
      seen.add(ref)
      plugins.push({ ref, path: path.resolve(value.path), name: value.name.slice(0, 80), enabled: value.enabled })
    }
    dropped += Math.max(0, rows.length - LIMIT)
    return { plugins, dropped }
  }
  const write = (plugins: ReadonlyArray<StoredPlugin>): void => {
    mkdirSync(path.dirname(filePath), { recursive: true })
    const temporary = `${filePath}.tmp`
    writeFileSync(temporary, JSON.stringify({ version: 1, plugins }), { encoding: "utf8", mode: OWNER_ONLY })
    renameSync(temporary, filePath)
    try { chmodSync(filePath, OWNER_ONLY) } catch {}
  }
  const result = (plugins: ReadonlyArray<StoredPlugin>, dropped = 0): PluginConfigResult =>
    ({ state: "ok", plugins: plugins.map(view), dropped })
  return {
    list: () => { const current = read(); return result(current.plugins, current.dropped) },
    addPath: pluginPath => {
      const current = read()
      const resolved = path.resolve(pluginPath)
      if (readiness(resolved) !== "ready") return { state: "rejected", reason: "That directory is not a valid Claude plugin." }
      const ref = pluginRef(resolved)
      if (current.plugins.some(plugin => plugin.ref === ref)) return { state: "rejected", reason: "That plugin is already registered." }
      const name = safeName(resolved)
      if (current.plugins.some(plugin => plugin.name === name)) return { state: "rejected", reason: "A plugin with that name is already registered." }
      if (current.plugins.length >= LIMIT) return { state: "rejected", reason: `At most ${LIMIT} plugins can be registered.` }
      const next = [...current.plugins, { ref, path: resolved, name, enabled: true }]
      write(next)
      return result(next, current.dropped)
    },
    toggle: (ref, enabled) => {
      const current = read()
      if (!current.plugins.some(plugin => plugin.ref === ref)) return { state: "rejected", reason: "Unknown plugin ref." }
      const next = current.plugins.map(plugin => plugin.ref === ref ? { ...plugin, enabled } : plugin)
      write(next); return result(next, current.dropped)
    },
    remove: ref => {
      const current = read(); const next = current.plugins.filter(plugin => plugin.ref !== ref)
      if (next.length === current.plugins.length) return { state: "rejected", reason: "Unknown plugin ref." }
      write(next); return result(next, current.dropped)
    },
    enabledPaths: () => read().plugins.filter(plugin => plugin.enabled && readiness(plugin.path) === "ready").map(plugin => plugin.path),
    resolveSkill: (ref, name) => {
      if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/.test(name)) return null
      const plugin = read().plugins.find(candidate => candidate.ref === ref && candidate.enabled && readiness(candidate.path) === "ready")
      if (plugin === undefined) return null
      const skillFile = path.join(plugin.path, "skills", name, "SKILL.md")
      return existsSync(skillFile) ? { pluginPath: plugin.path, name } : null
    },
  }
}
