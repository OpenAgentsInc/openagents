import { describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { openPluginConfigStore } from "../src/plugin-config-host.ts"

const fixture = () => {
  const root = mkdtempSync(join(tmpdir(), "oa-plugin-config-"))
  const plugin = join(root, "fixture-plugin")
  mkdirSync(join(plugin, ".claude-plugin"), { recursive: true })
  writeFileSync(join(plugin, ".claude-plugin", "plugin.json"), JSON.stringify({ name: "fixture" }))
  mkdirSync(join(plugin, "skills", "review"), { recursive: true })
  writeFileSync(join(plugin, "skills", "review", "SKILL.md"), "# Review\n")
  return { root, plugin, store: openPluginConfigStore(join(root, "plugins.json")) }
}

describe("local plugin registry", () => {
  test("persists a private path but projects only opaque identity and lifecycle fields", () => {
    const { root, plugin, store } = fixture()
    const added = store.addPath(plugin)
    expect(added.state).toBe("ok")
    if (added.state !== "ok") return
    expect(added.plugins[0]).toMatchObject({
      name: "fixture-plugin", provider: "claude_agent", provenance: "user_local",
      scope: "app", readiness: "ready", enabled: true, restartRequired: false,
      perSessionUse: "next_turn",
      skills: ["review"],
    })
    expect(JSON.stringify(added)).not.toContain(root)
    expect(readFileSync(join(root, "plugins.json"), "utf8")).toContain(plugin)
    expect(store.enabledPaths()).toEqual([plugin])
    expect(store.resolveSkill(added.plugins[0]!.ref, "review")).toEqual({ pluginPath: plugin, name: "review" })
  })

  test("toggle/remove are ref-bound and duplicate/invalid paths fail closed", () => {
    const { root, plugin, store } = fixture()
    const added = store.addPath(plugin)
    if (added.state !== "ok") throw new Error("expected add")
    const ref = added.plugins[0]!.ref
    expect(store.addPath(plugin).state).toBe("rejected")
    expect(store.addPath(root).state).toBe("rejected")
    expect(store.toggle(ref, false)).toMatchObject({ state: "ok", plugins: [{ enabled: false }] })
    expect(store.enabledPaths()).toEqual([])
    expect(store.remove(ref)).toMatchObject({ state: "ok", plugins: [] })
    expect(store.remove(ref).state).toBe("rejected")
  })

  test("missing plugins remain visible but are never offered to a session", () => {
    const { plugin, store } = fixture()
    const added = store.addPath(plugin)
    if (added.state !== "ok") throw new Error("expected add")
    rmSync(plugin, { recursive: true })
    expect(store.list()).toMatchObject({ state: "ok", plugins: [{ readiness: "missing" }] })
    expect(store.enabledPaths()).toEqual([])
    expect(store.resolveSkill(added.plugins[0]!.ref, "review")).toBeNull()
  })
})
