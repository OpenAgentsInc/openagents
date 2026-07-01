import { existsSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"

import {
  KHALA_CODE_CODEX_APP_SERVER_GAP_DOC_PATH,
  KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX,
  KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_ISSUE,
  KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_REFERENCE_COMMIT,
} from "../src/bun/codex-app-server-gap-matrix"
import {
  extractAppServerMethodsFromGeneratedType,
  findCodexReferenceRoot,
  KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
  readCodexSchemaFile,
} from "../src/bun/codex-parity-contract"
import {
  KHALA_CODE_DESKTOP_SLASH_COMMANDS,
  khalaCodeDesktopSlashCommandDispatchCoverage,
} from "../src/shared/codex-slash-commands"

const repoRoot = new URL("../../..", import.meta.url).pathname

const sorted = (values: readonly string[]): readonly string[] => [...values].sort()

describe("Khala Code Codex app-server gap matrix", () => {
  test("pins the same Codex reference as the parity contract", () => {
    expect(KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_REFERENCE_COMMIT).toBe(
      KHALA_CODE_CODEX_PARITY_REFERENCE_COMMIT,
    )
  })

  test("has complete, unique row metadata with source refs and issue links", () => {
    const rowIds = KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX.map(row => row.id)
    expect(new Set(rowIds).size).toBe(rowIds.length)

    const codexRoot = findCodexReferenceRoot()
    const incompleteRows = KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX.filter(row =>
      row.area.length === 0 ||
      row.rationale.length === 0 ||
      row.updateTrigger.length === 0 ||
      row.codexSourceRefs.length === 0 ||
      row.testRefs.length === 0 ||
      !row.linkedIssues.includes(KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_ISSUE)
    ).map(row => row.id)
    expect(incompleteRows).toEqual([])

    const missingCodexRefs = KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX
      .flatMap(row => row.codexSourceRefs.map(ref => ({ row: row.id, ref })))
      .filter(({ ref }) => !existsSync(join(codexRoot, ref)))
    expect(missingCodexRefs).toEqual([])

    const invalidDecisions = KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX.filter(row => {
      if (row.decision === "covered_by_app_server") return row.appServerMethods.length === 0
      if (row.decision === "khala_adapter_with_test") return row.khalaAdapter === undefined
      return row.upstreamGapId?.startsWith("codex.app_server.gap.") !== true
    }).map(row => row.id)
    expect(invalidDecisions).toEqual([])
  })

  test("maps every slash command exactly once", () => {
    const matrixCommands = KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX
      .flatMap(row => row.slashCommands)
    const duplicateMatrixCommands = matrixCommands.filter((command, index) =>
      matrixCommands.indexOf(command) !== index
    )

    expect(duplicateMatrixCommands).toEqual([])
    expect(sorted(matrixCommands)).toEqual(sorted(
      KHALA_CODE_DESKTOP_SLASH_COMMANDS.map(command => command.command),
    ))
  })

  test("uses real generated methods for stable app-server rows", async () => {
    const root = findCodexReferenceRoot()
    const generatedClientMethods = new Set(extractAppServerMethodsFromGeneratedType(
      await readCodexSchemaFile(root, "ClientRequest.ts"),
    ))
    const missingStableMethods = KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX
      .flatMap(row => row.appServerMethods.map(method => ({ row: row.id, method })))
      .filter(({ method }) => !generatedClientMethods.has(method))

    expect(missingStableMethods).toEqual([])
  })

  test("keeps registry dispatch status linked to the matrix decision", () => {
    const matrixByCommand = new Map<string, typeof KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX[number]>()
    for (const row of KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX) {
      for (const command of row.slashCommands) matrixByCommand.set(command, row)
    }

    const dispatchCoverage = khalaCodeDesktopSlashCommandDispatchCoverage()
    const missingRows = dispatchCoverage
      .filter(entry => !matrixByCommand.has(entry.command))
      .map(entry => entry.command)
    expect(missingRows).toEqual([])

    const invalidAppServerMappings = dispatchCoverage
      .filter(entry => entry.dispatchKind === "app_server")
      .filter(entry => {
        if (entry.method === undefined) return true
        const row = matrixByCommand.get(entry.command)
        const stable = row?.appServerMethods.includes(entry.method) === true
        const experimental = row?.experimentalAppServerMethods?.includes(entry.method) === true
        return !(stable || (entry.experimental === true && experimental))
      })
      .map(entry => entry.command)
    expect(invalidAppServerMappings).toEqual([])

    const gapCommandsWithoutUpstreamDecision = dispatchCoverage
      .filter(entry => entry.dispatchKind === "gap")
      .filter(entry => {
        const row = matrixByCommand.get(entry.command)
        return row?.decision !== "upstream_app_server_gap"
      })
      .map(entry => entry.command)
    expect(gapCommandsWithoutUpstreamDecision).toEqual([])

    const clientCommandsWithoutAdapterDecision = dispatchCoverage
      .filter(entry => entry.dispatchKind === "client")
      .filter(entry => {
        const row = matrixByCommand.get(entry.command)
        return row?.decision !== "khala_adapter_with_test"
      })
      .map(entry => entry.command)
    expect(clientCommandsWithoutAdapterDecision).toEqual([])
  })

  test("tracks background terminal parity as an experimental Khala adapter", () => {
    const row = KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX.find(row =>
      row.id === "background-terminal-management"
    )
    expect(row).toMatchObject({
      decision: "khala_adapter_with_test",
      experimentalAppServerMethods: [
        "thread/backgroundTerminals/list",
        "thread/backgroundTerminals/clean",
        "thread/backgroundTerminals/terminate",
      ],
    })
    expect(row?.testRefs).toContain("clients/khala-code-desktop/tests/rpc-handlers.test.ts")
  })

  test("keeps the human-readable matrix document in sync with checked rows", async () => {
    const docPath = join(repoRoot, KHALA_CODE_CODEX_APP_SERVER_GAP_DOC_PATH)
    expect(existsSync(docPath)).toBe(true)

    const doc = await readFile(docPath, "utf8")
    expect(doc).toContain(KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX_REFERENCE_COMMIT)
    for (const row of KHALA_CODE_CODEX_APP_SERVER_GAP_MATRIX) {
      expect(doc).toContain(row.id)
      if (row.upstreamGapId !== undefined) expect(doc).toContain(row.upstreamGapId)
    }
  })
})
