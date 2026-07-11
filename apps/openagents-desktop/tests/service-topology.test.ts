import { describe, expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import path from "node:path"

import {
  assertValidDesktopServiceSourceCoupling,
  assertValidDesktopServiceTopology,
  desktopServiceTopology,
  validateDesktopServiceSourceCoupling,
  validateDesktopServiceTopology,
  type DesktopServiceSourceSet,
  type DesktopServiceTopologyEntry,
} from "../src/service-topology.ts"

const repoRoot = path.resolve(import.meta.dir, "../../..")

const service = (
  override: Partial<DesktopServiceTopologyEntry> & Pick<DesktopServiceTopologyEntry, "id" | "scope">,
): DesktopServiceTopologyEntry => ({
  label: override.id,
  owner: "electron-main",
  installedAt: override.scope,
  modules: [`${override.id}.ts`],
  sourceEvidence: [{
    module: `${override.id}.ts`,
    compositionModule: `${override.id}.ts`,
    constructions: [`make${override.id}`],
  }],
  dependsOn: [],
  authority: [],
  cacheKey: { scope: override.scope, parts: [`${override.id}:cache-key`] },
  freshness: {
    source: "static_manifest",
    maxAge: "process_lifetime",
    invalidatesOn: [`${override.id}:invalidate`],
  },
  disposal: { disposesWith: override.scope, invalidatesOn: [`${override.id}:dispose`] },
  ...override,
})

const codes = (entries: ReadonlyArray<DesktopServiceTopologyEntry>) =>
  validateDesktopServiceTopology(entries).map(violation => violation.code)

const sourceCodes = (
  entries: ReadonlyArray<DesktopServiceTopologyEntry>,
  sources: DesktopServiceSourceSet,
) => validateDesktopServiceSourceCoupling(entries, sources).map(violation => violation.code)

const productionSources = (): DesktopServiceSourceSet => {
  const modules = new Set(desktopServiceTopology.flatMap(entry => [
    ...entry.modules,
    ...entry.sourceEvidence.map(evidence => evidence.compositionModule),
  ]))
  return Object.fromEntries([...modules].map(module => [
    module,
    readFileSync(path.join(repoRoot, module), "utf8"),
  ]))
}

const without = (
  entry: DesktopServiceTopologyEntry,
  key: keyof DesktopServiceTopologyEntry,
): DesktopServiceTopologyEntry => {
  const next = { ...entry } as Record<string, unknown>
  delete next[key]
  return next as DesktopServiceTopologyEntry
}

describe("Desktop service topology oracle (#8678)", () => {
  test("current Desktop services satisfy the source-coupled topology", () => {
    expect(() => assertValidDesktopServiceTopology()).not.toThrow()
    expect(() => assertValidDesktopServiceSourceCoupling(productionSources())).not.toThrow()

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
      "desktop-host-lifecycle",
      "desktop-operation-correlation",
      "preload-bridge",
      "effect-native-renderer",
    ]) {
      expect(ids.has(id)).toBe(true)
    }
    for (const entry of desktopServiceTopology) {
      expect(entry.cacheKey?.scope === "none" || entry.cacheKey?.scope === entry.scope).toBe(true)
      if (entry.cacheKey?.scope === "none") {
        expect(entry.cacheKey.parts).toEqual([])
      } else {
        expect(entry.cacheKey?.parts.length).toBeGreaterThan(0)
      }
      expect(entry.freshness?.invalidatesOn.length).toBeGreaterThan(0)
      expect(entry.disposal).toBeDefined()
      expect(entry.installedAt).toBe(entry.scope)
      expect(entry.sourceEvidence.length).toBeGreaterThan(0)
    }
  })

  test("fails when a bound construction is removed or no longer composed", () => {
    const sources = productionSources()
    const gateway = desktopServiceTopology.find(entry => entry.id === "desktop-runtime-gateway")!
    const main = "apps/openagents-desktop/src/main.ts"
    expect(sourceCodes([gateway], {
      ...sources,
      [main]: sources[main]!.replaceAll("createDesktopRuntimeGateway", "removedGatewayFactory"),
    })).toContain("missing_construction_symbol")
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

  test("fails session and project services installed at wider process scope", () => {
    expect(codes([
      service({ id: "session", scope: "conversation_or_run", installedAt: "process" }),
    ])).toContain("wrong_installation_scope")
    expect(codes([
      service({ id: "project", scope: "work_context", installedAt: "process" }),
    ])).toContain("wrong_installation_scope")
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

  test("derives and rejects renderer filesystem, process, network, and secret authority from source", () => {
    const renderer = service({
      id: "renderer",
      scope: "renderer_view",
      owner: "renderer",
      authority: ["view_projection"],
    })
    const source = [
      'import "node:fs"',
      'import "node:child_process"',
      'import "node:http"',
      'const token = process.env.OPENAGENTS_API_TOKEN',
      'export const makerenderer = () => fetch("https://example.invalid")',
    ].join("\n")
    const violations = validateDesktopServiceSourceCoupling([renderer], { "renderer.ts": source })
    expect(violations.filter(item => item.code === "forbidden_renderer_source_authority").map(item => item.detail)).toEqual([
      "Renderer construction contains filesystem authority.",
      "Renderer construction contains network authority.",
      "Renderer construction contains process authority.",
      "Renderer construction contains secret authority.",
    ])
  })

  test("derives ambient cwd, AsyncLocalStorage, and runtime exits from implementation source", () => {
    const internal = service({ id: "internal", scope: "work_context" })
    const source = [
      "export const makeinternal = () => process.cwd()",
      "const local = new AsyncLocalStorage()",
      "Effect.runPromise(program)",
    ].join("\n")
    const observed = sourceCodes([internal], { "internal.ts": source })
    expect(observed).toContain("source_ambient_authority")
    expect(observed).toContain("internal_run_promise_escape")
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
      service({ id: "false-no-cache", scope: "process", cacheKey: { scope: "none", parts: ["impossible"] } }),
    ])).toContain("invalid_cache_key_scope")
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
    expect(codes([
      service({ id: "empty-disposal", scope: "process", disposal: { disposesWith: "process", invalidatesOn: [] } }),
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
        disposal: { disposesWith: "process", invalidatesOn: ["wrong-owner"] },
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
