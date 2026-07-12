import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  decodeDiagnosticsReport,
  isExportSafe,
  redactDiagnosticsReport,
  worstLevel,
  type DiagnosticsReport,
} from "../src/diagnostics-contract.ts"
import { buildDiagnosticsReport, type DiagnosticsInputs } from "../src/diagnostics-report.ts"
import { makeDiagnosticsHost } from "../src/diagnostics-host.ts"

const dirs: string[] = []
const scratch = (): string => {
  const dir = mkdtempSync(path.join(tmpdir(), "oa-diag-"))
  dirs.push(dir)
  return dir
}
afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true })
})

const healthy = (): DiagnosticsInputs => ({
  appVersion: "0.0.1",
  generatedAt: 1_752_000_000_000,
  provider: { state: "ok", accounts: [{ ref: "codex", readiness: "ready" }, { ref: "claude-primary", readiness: "ready" }] },
  runtimeGateway: {
    state: "present",
    lifecycle: "ready",
    sessionPhase: "session_ready",
    capabilities: [
      { id: "agent-timeline", state: "available" },
      { id: "khala-sync", state: "available" },
    ],
  },
  sync: { state: "local_ready", syncPhase: "live", pendingMutationCount: 0 },
  workspace: { state: "selected", git: "clean", entryCount: 42 },
  pty: { state: "available", sessionCount: 0 },
  extensions: { state: "ok", enabledCount: 2, totalCount: 3, dropped: 0 },
})

const domainRow = (report: DiagnosticsReport, domain: string) => report.rows.find((row) => row.domain === domain)!

describe("diagnostics report builder", () => {
  test("a fully healthy fleet reports ok across every domain and rolls up to ok", () => {
    const report = buildDiagnosticsReport(healthy())
    expect(report.rows.map((row) => row.domain)).toEqual([
      "provider",
      "runtimeGateway",
      "sync",
      "workspace",
      "pty",
      "extensions",
    ])
    for (const domain of ["provider", "runtimeGateway", "sync", "workspace", "pty", "extensions"]) {
      expect(domainRow(report, domain).level).toBe("ok")
    }
    expect(report.overall).toBe("ok")
  })

  test("PTY reflects the workspace-bounded terminal host (CUT-20): ready with a session count, or honestly unavailable", () => {
    const ready = buildDiagnosticsReport({ ...healthy(), pty: { state: "available", sessionCount: 2 } })
    expect(domainRow(ready, "pty").level).toBe("ok")
    expect(domainRow(ready, "pty").summary).toContain("2 active sessions")
    const down = buildDiagnosticsReport({ ...healthy(), pty: { state: "unavailable", reason: "Terminal not configured" } })
    expect(domainRow(down, "pty").level).toBe("unavailable")
    expect(down.overall).toBe("unavailable")
  })

  test("fault injection: provider outage degrades provider and overall", () => {
    const report = buildDiagnosticsReport({ ...healthy(), provider: { state: "unavailable", reason: "pylon offline" } })
    expect(domainRow(report, "provider").level).toBe("unavailable")
    expect(domainRow(report, "provider").actions).toContain("reprobe_providers")
  })

  test("fault injection: half-ready providers report degraded, not ok", () => {
    const report = buildDiagnosticsReport({
      ...healthy(),
      provider: { state: "ok", accounts: [{ ref: "codex", readiness: "ready" }, { ref: "codex-2", readiness: "credentials-missing" }] },
    })
    expect(domainRow(report, "provider").level).toBe("degraded")
    expect(domainRow(report, "provider").summary).toContain("1 of 2")
  })

  test("fault injection: closed sync is unavailable; unobserved is unknown", () => {
    expect(domainRow(buildDiagnosticsReport({ ...healthy(), sync: { state: "closed", syncPhase: "closed", pendingMutationCount: 0 } }), "sync").level).toBe("unavailable")
    expect(domainRow(buildDiagnosticsReport({ ...healthy(), sync: { state: "unobserved" } }), "sync").level).toBe("unknown")
  })

  test("fault injection: gateway lifecycle and capability degradation", () => {
    expect(domainRow(buildDiagnosticsReport({ ...healthy(), runtimeGateway: { state: "absent" } }), "runtimeGateway").level).toBe("unavailable")
    const starting = buildDiagnosticsReport({
      ...healthy(),
      runtimeGateway: { state: "present", lifecycle: "starting", sessionPhase: "unverified", capabilities: [] },
    })
    expect(domainRow(starting, "runtimeGateway").level).toBe("degraded")
    const degradedCap = buildDiagnosticsReport({
      ...healthy(),
      runtimeGateway: {
        state: "present",
        lifecycle: "ready",
        sessionPhase: "session_ready",
        capabilities: [{ id: "khala-sync", state: "available" }, { id: "provider-accounts", state: "unavailable" }],
      },
    })
    expect(domainRow(degradedCap, "runtimeGateway").level).toBe("degraded")
    expect(domainRow(degradedCap, "runtimeGateway").refs).toContain("provider-accounts")
  })

  test("workspace: none is unknown, git-unavailable is degraded, and no path leaks", () => {
    expect(domainRow(buildDiagnosticsReport({ ...healthy(), workspace: { state: "none" } }), "workspace").level).toBe("unknown")
    const gitDown = buildDiagnosticsReport({ ...healthy(), workspace: { state: "selected", git: "unavailable", entryCount: 3 } })
    expect(domainRow(gitDown, "workspace").level).toBe("degraded")
    // The builder never receives or emits the workspace root path.
    expect(domainRow(gitDown, "workspace").summary).not.toMatch(/\//)
  })

  test("extensions: dropped invalid rows degrade the domain", () => {
    const report = buildDiagnosticsReport({ ...healthy(), extensions: { state: "ok", enabledCount: 1, totalCount: 2, dropped: 1 } })
    expect(domainRow(report, "extensions").level).toBe("degraded")
    expect(domainRow(report, "extensions").summary).toContain("1 invalid")
  })

  test("the built report is schema-valid and export-safe by construction", () => {
    const report = buildDiagnosticsReport(healthy())
    expect(decodeDiagnosticsReport(report)).not.toBeNull()
    expect(isExportSafe(report)).toBe(true)
    // No row summary or ref carries a path, url, or token-like blob.
    for (const row of report.rows) {
      expect(row.summary).not.toMatch(/\/|:\/\/|Bearer|sk-/)
      for (const ref of row.refs) expect(ref).not.toMatch(/\/|@/)
    }
  })
})

describe("diagnostics redaction (privacy scan of the export artifact)", () => {
  const leaky = (): DiagnosticsReport => ({
    schema: "openagents.desktop.diagnostics.v1",
    generatedAt: 1,
    appVersion: "0.0.1",
    overall: "degraded",
    rows: [
      // A regressed builder that leaked a path + a token into the summary/refs.
      { domain: "workspace", level: "degraded", summary: "failed at /Users/alice/secret-project/.env", refs: ["ok"], actions: ["refresh"] },
      { domain: "provider", level: "degraded", summary: "auth Bearer sk-abc123DEADBEEFabc123DEADBEEFabc123DEADBEEF failed", refs: ["codex"], actions: ["refresh"] },
    ],
  })

  test("redaction removes secret-like summaries and keeps the report schema-valid", () => {
    const before = leaky()
    // The leaky report is (correctly) flagged unsafe before redaction.
    expect(isExportSafe(before)).toBe(false)
    const redacted = redactDiagnosticsReport(before)
    expect(isExportSafe(redacted)).toBe(true)
    expect(decodeDiagnosticsReport(redacted)).not.toBeNull()
    // The secret content is gone; a generic placeholder remains.
    const serialized = JSON.stringify(redacted)
    expect(serialized).not.toContain("secret-project")
    expect(serialized).not.toContain("sk-abc123")
    expect(serialized).not.toContain("Bearer")
    expect(redacted.rows[0].summary).toContain("redacted")
  })

  test("redaction is idempotent on an already-clean report", () => {
    const clean = buildDiagnosticsReport(healthy())
    expect(redactDiagnosticsReport(clean)).toEqual(clean)
  })

  test("worstLevel rolls up severity correctly", () => {
    expect(worstLevel([{ level: "ok" }, { level: "unknown" }, { level: "degraded" }])).toBe("degraded")
    expect(worstLevel([{ level: "ok" }, { level: "unavailable" }])).toBe("unavailable")
    expect(worstLevel([])).toBe("ok")
    expect(worstLevel([{ level: "ok" }, { level: "unknown" }])).toBe("unknown")
  })
})

describe("diagnostics host (main-side gather + redacted export + recovery)", () => {
  const inputs = (): DiagnosticsInputs => ({
    appVersion: "0.0.1",
    generatedAt: 1,
    provider: { state: "ok", accounts: [{ ref: "codex", readiness: "ready" }] },
    runtimeGateway: { state: "present", lifecycle: "ready", sessionPhase: "session_ready", capabilities: [] },
    sync: { state: "local_ready", syncPhase: "live", pendingMutationCount: 0 },
    workspace: { state: "selected", git: "clean", entryCount: 1 },
    pty: { state: "available", sessionCount: 0 },
    extensions: { state: "ok", enabledCount: 0, totalCount: 0, dropped: 0 },
  })

  test("export writes a redacted, owner-only bundle and returns no path in the notice", async () => {
    const dir = scratch()
    // Inject a builder that would leak a path, to prove the host redacts before writing.
    const leaky = (): DiagnosticsInputs => ({ ...inputs(), pty: { state: "unavailable", reason: "failed at /Users/alice/.env" } })
    const host = makeDiagnosticsHost({ collectInputs: leaky, exportDir: path.join(dir, "diagnostics"), now: () => 1_752_000_000_000 })
    const result = await host.exportRedacted()
    expect(result.ok).toBe(true)
    // The public-safe notice must not carry the saved path.
    expect(result.notice).not.toMatch(/\//)
    const files = readdirSync(path.join(dir, "diagnostics"))
    expect(files.length).toBe(1)
    const file = path.join(dir, "diagnostics", files[0])
    expect(statSync(file).mode & 0o777).toBe(0o600)
    const bundle = JSON.parse(readFileSync(file, "utf8"))
    expect(decodeDiagnosticsReport(bundle)).not.toBeNull()
    // The leaked path never reaches disk.
    expect(readFileSync(file, "utf8")).not.toContain("/Users/alice")
  })

  test("runAction dispatches a mapped recovery callback and reports its notice", async () => {
    let ran = 0
    const host = makeDiagnosticsHost({
      collectInputs: inputs,
      exportDir: path.join(scratch(), "diagnostics"),
      recovery: { reprobe_providers: async () => { ran += 1; return { ok: true, notice: "Providers re-checked" } } },
    })
    const result = await host.runAction("reprobe_providers")
    expect(ran).toBe(1)
    expect(result).toEqual({ ok: true, notice: "Providers re-checked" })
  })

  test("an unmapped action is a safe no-op, not a throw", async () => {
    const host = makeDiagnosticsHost({ collectInputs: inputs, exportDir: path.join(scratch(), "diagnostics") })
    expect(await host.runAction("restart_runtime")).toEqual({ ok: false, notice: "No recovery action available" })
  })

  test("gather produces a schema-valid, export-safe report", async () => {
    const host = makeDiagnosticsHost({ collectInputs: inputs, exportDir: path.join(scratch(), "diagnostics") })
    expect(isExportSafe(await host.gather())).toBe(true)
  })
})
