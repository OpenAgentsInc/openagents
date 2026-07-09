/**
 * Oracle tests for contract sarah.split_screen_blueprint_map.v1 (BM-3, #8629).
 *
 * These source checks intentionally cover only the shell/cut-list layer. The
 * EN view-tree oracle lives in src/ui/surface.test.ts, and BM-5 will add the
 * browser screenshot smoke gate against this same contract id.
 */

import { describe, expect, test } from "bun:test"

import {
  checkBehaviorContractCoverageFromFiles,
  renderBehaviorContractMarkdown,
  validateBehaviorContractRegistry,
} from "@openagentsinc/behavior-contracts"
import {
  SARAH_SPLIT_LAYOUT_CONTRACTS_DOC_PATH,
  sarahSplitLayoutContractRegistry,
} from "./split-layout-contracts.ts"

const repoPath = (ref: string): string =>
  new URL(`../../../../${ref}`, import.meta.url).pathname

describe("sarah split-layout contract registry", () => {
  test("registry passes mechanical validation", () => {
    const validation = validateBehaviorContractRegistry(
      sarahSplitLayoutContractRegistry,
    )
    expect(validation.issues).toEqual([])
    expect(validation.ok).toBe(true)
  })

  test("every enforced bun-test oracle exists and references its contract", async () => {
    const report = await checkBehaviorContractCoverageFromFiles(
      sarahSplitLayoutContractRegistry,
      (path) => Bun.file(path).text(),
      repoPath,
    )
    expect(
      report.results.filter(
        (result) =>
          result.status !== "covered" &&
          result.status !== "skipped_kind" &&
          result.status !== "skipped_state",
      ),
    ).toEqual([])
    expect(report.ok).toBe(true)
  })

  test("the human contract doc records the split-screen directive", async () => {
    const doc = await Bun.file(repoPath(SARAH_SPLIT_LAYOUT_CONTRACTS_DOC_PATH)).text()
    expect(doc).toContain(
      `Registry version: \`${sarahSplitLayoutContractRegistry.version}\``,
    )
    for (const contract of sarahSplitLayoutContractRegistry.contracts) {
      expect(doc).toContain(contract.contractId)
      expect(doc).toContain(contract.statement)
    }
    expect(doc).toContain(
      renderBehaviorContractMarkdown(sarahSplitLayoutContractRegistry).split(
        "\n",
      )[0] ?? "",
    )
  })
})

describe("contract sarah.split_screen_blueprint_map.v1 — source cut-list oracle", () => {
  test("the host shell is a 50/50 split with compact disclosure in the right pane", async () => {
    const html = await Bun.file(repoPath("apps/sarah/src/ui/index.html")).text()
    const css = await Bun.file(repoPath("apps/sarah/src/ui/sarah.css")).text()

    expect(html).toContain('class="sarah-right-shell"')
    expect(html.indexOf('id="sarah-avatar"')).toBeLessThan(
      html.indexOf('class="sarah-right-shell"'),
    )
    expect(html).toContain('class="sarah-disclosure"')

    expect(css).toContain("grid-template-columns: minmax(0, 1fr) minmax(0, 1fr)")
    expect(css).toContain("height: 100vh")
    expect(css).toContain('[data-en-key="avatar-overlay"]')
    expect(css).not.toContain("480px")
    expect(css).not.toContain("720px")
  })

  test("the audited caption and controls row are gone from the EN source", async () => {
    const source = await Bun.file(repoPath("apps/sarah/src/ui/main.ts")).text()
    expect(source).toContain("Tabs")
    expect(source).toContain("Blueprint map")
    expect(source).toContain("avatar-overlay")
    expect(source).not.toContain("OpenAgents sales · openagents.com/sarah")
    expect(source).not.toContain("avatar-controls")
    expect(source).not.toContain('text("title", "Sarah"')
  })
})
