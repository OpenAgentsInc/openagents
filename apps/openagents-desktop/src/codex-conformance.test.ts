import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

import { evaluateCodexReleaseGate, makeCodexConformanceReport, publicCodexDiagnostics } from "./codex-conformance.ts"

describe("Codex conformance release gate", () => {
  test("keeps committed machine-readable evidence deterministic", () => {
    const committed = JSON.parse(readFileSync(resolve(import.meta.dirname, "../../../docs/receipts/2026-07-15-codex-app-server-conformance.json"), "utf8"))
    expect(committed).toEqual(makeCodexConformanceReport())
  })

  test("reports the selected 126/1/11/72 wire target and exact bundled target separately", () => {
    const report = makeCodexConformanceReport()
    expect(report.selectedTarget.counts).toEqual({ "client-request": 126, "client-notification": 1, "server-request": 11, "server-notification": 72 })
    expect(report.bundledReleaseTarget.counts).toEqual({ "client-request": 125, "client-notification": 1, "server-request": 11, "server-notification": 69 })
    expect(report.selectedTarget.members).toHaveLength(210); expect(report.bundledReleaseTarget.members).toHaveLength(206)
    expect(report.selectedTarget.coverage).toMatchObject({ transport: 100, handlerDisposition: 100, nativeProjection: 100, productPresentation: 100, authority: 100, fixture: 100 })
    expect(report.bundledReleaseTarget.coverage).toEqual({ transport: 100, handlerDisposition: 100, nativeProjection: 100, productPresentation: 100, authority: 100, fixture: 100, realBinaryFamily: 100 })
  })

  test("gives every member a typed product surface or explicit policy disposition", () => {
    const rows = makeCodexConformanceReport().selectedTarget.members
    expect(rows.every(row => /^(?:desktop:|policy:)/u.test(row.productPresentation))).toBe(true)
    expect(rows.some(row => row.policy === "requires-binary-upgrade")).toBe(true)
    expect(rows.filter(row => row.policy === "test-only").every(row => row.member.startsWith("mock/"))).toBe(true)
    expect(rows.every(row => !row.productPresentation.includes("raw-rpc"))).toBe(true)
  })

  test("keeps composer authority equivalent across surfaces", () => {
    const report = makeCodexConformanceReport()
    expect(report.crossSurface).toEqual({ desktop: "typed-projections-and-intents", web: "policy:no-local-app-server-authority", mobile: "policy:no-local-app-server-authority", operator: "diagnostic-counts-and-state-only", composer: "shared-effect-admission-matrix" })
    expect(report.selectedTarget.members.filter(row => /^(?:turn\/(?:start|steer|interrupt)|thread\/(?:compact\/start|rollback))$/u.test(row.member)).every(row => row.authority === "composer-admission-authority")).toBe(true)
  })

  test("blocks release for drift, mismatch, overload, repair, unsettled reverse RPC, queue, or recovery corruption", () => {
    const blocked = evaluateCodexReleaseGate({ supervisorStates: [{ status: "repairing", generation: 2, attempt: 1, maxAttempts: 3 }], compatibilityReceipts: [{ _tag: "CodexCompatibilityReceipt", generation: 2, observedAt: new Date(0).toISOString(), direction: "server-notification", method: "future/event", reason: "unknown_method", detail: "private token", occurrences: 1 }], binaryManifestMatches: false, familySmokesPass: false, reverseRequestsSettled: false, queueJournalHealthy: false, recoveryHealthy: false, overloadObserved: true })
    expect(blocked.releasable).toBe(false)
    expect(blocked.blockers).toEqual(["binary_manifest_mismatch", "family_smoke_failure", "protocol_decode_drift", "supervisor_not_ready", "reverse_request_unsettled", "queue_journal_corrupt", "recovery_corrupt", "transport_overload"])
    expect(blocked.diagnosticRef).toMatch(/^diagnostic\.[a-f0-9]{24}$/u)
  })

  test("releases only with matching binary, complete smokes, ready supervisors, and healthy recovery", () => {
    expect(evaluateCodexReleaseGate({ supervisorStates: [{ status: "ready", generation: 1 }], compatibilityReceipts: [], binaryManifestMatches: true, familySmokesPass: true, reverseRequestsSettled: true, queueJournalHealthy: true, recoveryHealthy: true, overloadObserved: false })).toMatchObject({ releasable: true, blockers: [] })
  })

  test("public diagnostics redact receipt detail and expose state/count truth only", () => {
    const projection = publicCodexDiagnostics({ states: [{ status: "degraded", generation: 3, attempt: 2, reason: "private credential" }], receipts: [{ _tag: "CodexCompatibilityReceipt", generation: 3, observedAt: new Date(0).toISOString(), direction: "server-notification", method: "future/event", reason: "invalid_payload", detail: "token=secret", occurrences: 4 }] })
    expect(projection).toEqual({ connections: [{ status: "degraded", generation: 3 }], compatibility: [{ direction: "server-notification", method: "future/event", reason: "invalid_payload", occurrences: 4 }] })
    expect(JSON.stringify(projection)).not.toContain("secret")
  })
})
