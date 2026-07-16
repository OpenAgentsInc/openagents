import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { decodeUpdateMigrationEvidence, evaluateNoMigrationInvariant, migrationLedgerFromEvidence } from "./update-migration-evidence.ts"

const roots: string[] = []
afterEach(() => { for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true }) })

describe("update migration evidence", () => {
  test("admits only existing, correctly typed durable stores outside the app bundle", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-migration-evidence-")); roots.push(root)
    const app = path.join(root, "Applications", "OpenAgents.app")
    const sessions = path.join(root, ".codex", "sessions")
    const vault = path.join(root, "userData", "session", "native-session.enc")
    const settings = path.join(root, "userData")
    const drafts = path.join(root, "userData", "sync", "khala-sync.sqlite")
    mkdirSync(app, { recursive: true }); mkdirSync(sessions, { recursive: true }); mkdirSync(path.dirname(vault), { recursive: true }); mkdirSync(path.dirname(drafts), { recursive: true })
    writeFileSync(vault, "encrypted"); writeFileSync(drafts, "sqlite")
    const kinds = { sessions: "directory", vaultRefs: "file", settings: "directory", drafts: "file" } as const
    const evidence = evaluateNoMigrationInvariant({ installedApplicationRoot: app, categoryRoots: { sessions, vaultRefs: vault, settings, drafts }, categoryKinds: kinds })
    expect(evidence).not.toBeNull()
    expect(decodeUpdateMigrationEvidence(JSON.parse(JSON.stringify(evidence)))).toEqual(evidence)
    expect(migrationLedgerFromEvidence(evidence!)).toEqual({ sessions: { status: "preserved" }, vaultRefs: { status: "preserved" }, settings: { status: "preserved" }, drafts: { status: "preserved" } })
    expect(evaluateNoMigrationInvariant({ installedApplicationRoot: app, categoryRoots: { sessions: path.join(app, "sessions"), vaultRefs: vault, settings, drafts }, categoryKinds: kinds })).toBeNull()
    expect(evaluateNoMigrationInvariant({ installedApplicationRoot: app, categoryRoots: { sessions: path.join(root, "missing"), vaultRefs: vault, settings, drafts }, categoryKinds: kinds })).toBeNull()
    expect(evaluateNoMigrationInvariant({ installedApplicationRoot: app, categoryRoots: { sessions, vaultRefs: vault, settings, drafts }, categoryKinds: { ...kinds, drafts: "directory" } })).toBeNull()
  })

  test("records only the two legitimate absent-store dispositions", () => {
    const root = mkdtempSync(path.join(tmpdir(), "oa-migration-absent-")); roots.push(root)
    const app = path.join(root, "Applications", "OpenAgents.app")
    const settings = path.join(root, "userData")
    const drafts = path.join(settings, "sync", "khala-sync.sqlite")
    mkdirSync(app, { recursive: true }); mkdirSync(path.dirname(drafts), { recursive: true }); writeFileSync(drafts, "sqlite")
    const categoryRoots = { sessions: path.join(root, ".codex", "sessions"), vaultRefs: path.join(settings, "session", "native-session.enc"), settings, drafts }
    const categoryKinds = { sessions: "directory", vaultRefs: "file", settings: "directory", drafts: "file" } as const
    const evidence = evaluateNoMigrationInvariant({ installedApplicationRoot: app, categoryRoots, categoryKinds, absentDispositions: { sessions: "no_sessions", vaultRefs: "signed_out" } })
    expect(evidence?.categories).toMatchObject({ sessions: { disposition: "absent", reason: "no_sessions" }, vaultRefs: { disposition: "absent", reason: "signed_out" }, settings: { disposition: "present", kind: "directory" }, drafts: { disposition: "present", kind: "file" } })
    expect(migrationLedgerFromEvidence(evidence!)).toEqual({ sessions: { status: "loss_accounted", reasonRef: "no_sessions" }, vaultRefs: { status: "loss_accounted", reasonRef: "signed_out" }, settings: { status: "preserved" }, drafts: { status: "preserved" } })
    expect(evaluateNoMigrationInvariant({ installedApplicationRoot: app, categoryRoots, categoryKinds, absentDispositions: { vaultRefs: "no_sessions" } })).toBeNull()
  })
})
