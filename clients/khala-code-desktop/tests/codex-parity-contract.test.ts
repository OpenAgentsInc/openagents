import { existsSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import {
  codexSchemaPath,
  extractAppServerMethodsFromGeneratedType,
  extractThreadItemTypesFromGeneratedType,
  inspectCodexReferenceRoot,
  KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF,
  KHALA_CODE_CODEX_PARITY_COVERAGE,
  KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
  KHALA_CODE_CODEX_PARITY_REQUIRED_CLIENT_METHODS,
  KHALA_CODE_CODEX_PARITY_REQUIRED_NOTIFICATIONS,
  KHALA_CODE_CODEX_PARITY_REQUIRED_SCHEMA_FILES,
  KHALA_CODE_CODEX_PARITY_REQUIRED_SERVER_REQUESTS,
  KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES,
  readCodexReferenceCommit,
  readCodexSchemaFile,
} from "../src/bun/codex-parity-contract"
import {
  KHALA_CODE_DESKTOP_SLASH_COMMANDS,
  khalaCodeDesktopSlashCommandDispatchCoverage,
} from "../src/shared/codex-slash-commands"

const expectCodexReferenceRootOrBlocker = (): string | null => {
  const status = inspectCodexReferenceRoot()
  if (status.ok) return status.root

  expect(status).toMatchObject({
    blockerRef: KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF,
    ok: false,
    status: "blocked",
  })
  expect(status.reason.length).toBeGreaterThan(0)
  return null
}

describe("Khala Code Codex parity contract", () => {
  test("reports blocker.codex_reference_checkout_missing when the reference checkout is absent", () => {
    const status = inspectCodexReferenceRoot("/private/tmp/khala-code-missing-codex-reference", {})

    expect(status).toMatchObject({
      blockerRef: KHALA_CODE_CODEX_REFERENCE_CHECKOUT_MISSING_BLOCKER_REF,
      ok: false,
      status: "blocked",
    })
    if (status.ok) return

    expect(status.reason).toContain("projects/repos/codex")
  })

  test("pins the Codex reference checkout used for fixture parity", async () => {
    const root = expectCodexReferenceRootOrBlocker()
    if (root === null) return

    expect(await readCodexReferenceCommit(root)).toBe(KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT)
    expect(existsSync(`${root}/codex-rs/tui/src/slash_command.rs`)).toBe(true)
    expect(existsSync(`${root}/codex-rs/app-server-protocol/schema/typescript/ClientRequest.ts`))
      .toBe(true)
  })

  test("requires generated app-server schema files and parity-critical methods", async () => {
    const root = expectCodexReferenceRootOrBlocker()
    if (root === null) return

    const missingFiles = KHALA_CODE_CODEX_PARITY_REQUIRED_SCHEMA_FILES
      .filter(relative => !existsSync(codexSchemaPath(root, relative)))

    expect(missingFiles).toEqual([])

    const clientMethods = extractAppServerMethodsFromGeneratedType(
      await readCodexSchemaFile(root, "ClientRequest.ts"),
    )
    const serverRequests = extractAppServerMethodsFromGeneratedType(
      await readCodexSchemaFile(root, "ServerRequest.ts"),
    )
    const notifications = extractAppServerMethodsFromGeneratedType(
      await readCodexSchemaFile(root, "ServerNotification.ts"),
    )
    const missingClientMethods = KHALA_CODE_CODEX_PARITY_REQUIRED_CLIENT_METHODS
      .filter(method => !clientMethods.includes(method))
    const missingServerRequests = KHALA_CODE_CODEX_PARITY_REQUIRED_SERVER_REQUESTS
      .filter(method => !serverRequests.includes(method))
    const missingNotifications = KHALA_CODE_CODEX_PARITY_REQUIRED_NOTIFICATIONS
      .filter(method => !notifications.includes(method))

    expect(missingClientMethods).toEqual([])
    expect(missingServerRequests).toEqual([])
    expect(missingNotifications).toEqual([])
  })

  test("tracks upstream ThreadItem variants with fixture coverage", async () => {
    const root = expectCodexReferenceRootOrBlocker()
    if (root === null) return

    const upstreamTypes = extractThreadItemTypesFromGeneratedType(
      await readCodexSchemaFile(root, "v2/ThreadItem.ts"),
    )

    expect(upstreamTypes).toEqual([...KHALA_CODE_CODEX_PARITY_REQUIRED_THREAD_ITEM_TYPES])
  })

  test("distinguishes Codex-wrapper fixture coverage from legacy fallback guards", () => {
    const ids = KHALA_CODE_CODEX_PARITY_COVERAGE.map(row => row.id)
    expect(new Set(ids).size).toBe(ids.length)
    expect(new Set(KHALA_CODE_CODEX_PARITY_COVERAGE.map(row => row.harness))).toEqual(new Set([
      "codex_wrapper_fixture",
      "codex_wrapper_live",
      "legacy_fallback_guard",
    ]))
    const repoRoot = new URL("../../..", import.meta.url).pathname
    for (const row of KHALA_CODE_CODEX_PARITY_COVERAGE) {
      expect(existsSync(join(repoRoot, row.testFile))).toBe(true)
      if (row.harness !== "legacy_fallback_guard") {
        expect(row.testFile).not.toContain("khala-chat-runtime")
      }
      expect(row.covers.length).toBeGreaterThan(0)
    }
  })

  test("keeps slash-command parity rows mapped to app-server methods or explicit gaps", () => {
    expect(khalaCodeDesktopSlashCommandDispatchCoverage()).toHaveLength(
      KHALA_CODE_DESKTOP_SLASH_COMMANDS.length,
    )
    for (const entry of khalaCodeDesktopSlashCommandDispatchCoverage()) {
      if (entry.dispatchKind === "app_server") {
        expect(entry.method).toBeDefined()
        if (entry.method === undefined) continue
        const generatedMethod = (KHALA_CODE_CODEX_PARITY_REQUIRED_CLIENT_METHODS as readonly string[])
          .includes(entry.method)
        expect(generatedMethod || entry.experimental === true).toBe(true)
      }
      if (entry.dispatchKind === "gap") {
        expect(entry.dependency).toContain("Codex")
      }
    }
  })
})
