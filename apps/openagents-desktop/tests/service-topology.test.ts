import { describe, expect, test } from "bun:test"

import {
  assertValidDesktopServiceTopology,
  desktopServiceTopology,
  validateDesktopServiceTopology,
  type DesktopServiceTopologyEntry,
} from "../src/service-topology.ts"

const service = (
  override: Partial<DesktopServiceTopologyEntry> & Pick<DesktopServiceTopologyEntry, "id" | "scope">,
): DesktopServiceTopologyEntry => ({
  label: override.id,
  owner: "electron-main",
  modules: [`${override.id}.ts`],
  dependsOn: [],
  authority: [],
  cacheKey: { scope: override.scope, parts: [`${override.id}:cache-key`] },
  freshness: {
    source: "static_manifest",
    maxAge: "process_lifetime",
    invalidatesOn: [`${override.id}:invalidate`],
  },
  disposal: { disposesWith: override.scope, closes: [] },
  ...override,
})

const codes = (entries: ReadonlyArray<DesktopServiceTopologyEntry>) =>
  validateDesktopServiceTopology(entries).map(violation => violation.code)

const without = (
  entry: DesktopServiceTopologyEntry,
  key: keyof DesktopServiceTopologyEntry,
): DesktopServiceTopologyEntry => {
  const next = { ...entry } as Record<string, unknown>
  delete next[key]
  return next as DesktopServiceTopologyEntry
}

describe("Desktop service topology oracle (#8678)", () => {
  test("current Desktop services satisfy the checked topology manifest", () => {
    expect(() => assertValidDesktopServiceTopology()).not.toThrow()

    const ids: ReadonlySet<string> = new Set(desktopServiceTopology.map(entry => entry.id))
    for (const id of [
      "electron-main-composition",
      "desktop-runtime-gateway",
      "desktop-session-custody",
      "desktop-sync-host",
      "workspace-root",
      "codex-history-reader",
      "legacy-thread-store",
      "fleet-stage-control",
      "codex-connect-host",
      "preload-bridge",
      "effect-native-renderer",
    ]) {
      expect(ids.has(id)).toBe(true)
    }
    for (const entry of desktopServiceTopology) {
      expect(entry.cacheKey?.scope).toBe(entry.scope)
      expect(entry.cacheKey?.parts.length).toBeGreaterThan(0)
      expect(entry.freshness?.invalidatesOn.length).toBeGreaterThan(0)
      expect(entry.disposal).toBeDefined()
    }
  })

  test("fails an ordinary process service that captures WorkContext authority", () => {
    expect(codes([
      service({ id: "workspace", scope: "work_context" }),
      service({ id: "process-capture", scope: "process", dependsOn: ["workspace"] }),
    ])).toContain("wrong_scope_dependency")
  })

  test("allows a named perimeter composition root to wire narrower services", () => {
    expect(codes([
      service({ id: "workspace", scope: "work_context" }),
      service({ id: "composition-root", scope: "process", dependsOn: ["workspace"], perimeter: true }),
    ])).not.toContain("wrong_scope_dependency")
  })

  test("fails WorkContext services that capture view or run scopes", () => {
    expect(codes([
      service({ id: "view", scope: "foreign_host_or_view" }),
      service({ id: "workspace", scope: "work_context", dependsOn: ["view"] }),
    ])).toContain("wrong_scope_dependency")
  })

  test("fails renderer-owned runtime authority", () => {
    expect(codes([
      service({ id: "renderer", scope: "renderer_view", owner: "renderer", authority: ["runtime"] }),
    ])).toContain("renderer_runtime_authority")
  })

  test("fails duplicate public Schema identities unless explicitly legacy", () => {
    const duplicate: DesktopServiceTopologyEntry[] = [
      service({
        id: "gateway-a",
        scope: "process",
        publicSchemas: [{ name: "Command", module: "a.ts" }],
      }),
      service({
        id: "gateway-b",
        scope: "process",
        publicSchemas: [{ name: "Command", module: "b.ts" }],
      }),
    ]
    expect(codes(duplicate)).toContain("duplicate_public_schema_identity")

    const legacy: DesktopServiceTopologyEntry[] = [
      service({
        id: "gateway-a",
        scope: "process",
        publicSchemas: [{ name: "Command", module: "a.ts" }],
      }),
      service({
        id: "legacy-chat",
        scope: "conversation_or_run",
        publicSchemas: [{ name: "Command", module: "legacy.ts", legacy: true }],
      }),
    ]
    expect(codes(legacy)).not.toContain("duplicate_public_schema_identity")
  })

  test("fails ambient path or AsyncLocalStorage authority", () => {
    expect(codes([
      service({ id: "ambient", scope: "work_context", ambientAuthority: ["cwd", "async_local_storage"] }),
    ])).toContain("ambient_authority")
  })

  test("fails resources that are owned by a wider scope than their service", () => {
    expect(codes([
      service({
        id: "request-listener",
        scope: "request_or_command",
        ownedResources: [{ kind: "subscription", disposesWith: "process" }],
      }),
    ])).toContain("unowned_resource")
  })

  test("fails missing cache, freshness, or disposal declarations", () => {
    expect(codes([
      without(service({ id: "no-cache", scope: "process" }), "cacheKey"),
    ])).toContain("missing_cache_key")
    expect(codes([
      service({ id: "empty-cache", scope: "process", cacheKey: { scope: "process", parts: [] } }),
    ])).toContain("missing_cache_key")
    expect(codes([
      without(service({ id: "no-freshness", scope: "process" }), "freshness"),
    ])).toContain("missing_freshness")
    expect(codes([
      service({
        id: "empty-freshness",
        scope: "process",
        freshness: { source: "static_manifest", maxAge: "process_lifetime", invalidatesOn: [] },
      }),
    ])).toContain("missing_freshness")
    expect(codes([
      without(service({ id: "no-disposal", scope: "process" }), "disposal"),
    ])).toContain("missing_disposal")
  })

  test("fails cache keys and service disposal that escape their owning scope", () => {
    expect(codes([
      service({
        id: "workspace-cache-mismatch",
        scope: "work_context",
        cacheKey: { scope: "process", parts: ["shared-process-cache"] },
      }),
    ])).toContain("invalid_cache_key_scope")
    expect(codes([
      service({
        id: "request-disposal-escape",
        scope: "request_or_command",
        disposal: { disposesWith: "process", closes: ["subscription"] },
      }),
    ])).toContain("invalid_disposal_scope")
  })

  test("fails internal runPromise escapes outside named perimeter modules", () => {
    expect(codes([
      service({ id: "internal-runtime", scope: "conversation_or_run", internalRunPromise: true }),
    ])).toContain("internal_run_promise_escape")
    expect(codes([
      service({ id: "runtime-perimeter", scope: "process", internalRunPromise: true, perimeter: true }),
    ])).not.toContain("internal_run_promise_escape")
  })

  test("fails cycles and unknown dependencies", () => {
    expect(codes([
      service({ id: "a", scope: "process", dependsOn: ["b"] }),
      service({ id: "b", scope: "process", dependsOn: ["a"] }),
    ])).toContain("cycle")
    expect(codes([
      service({ id: "a", scope: "process", dependsOn: ["missing"] }),
    ])).toContain("unknown_dependency")
  })
})
