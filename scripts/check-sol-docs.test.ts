import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { dirname, join, resolve } from "node:path"
import {
  collectSolDocErrors,
  compareLiveIssues,
  type RoadmapIssue,
  type RoadmapIssueSnapshot,
  validateArchiveManifest,
  validateIssueIndex,
  validateMarkdownLinks,
  validateMaster,
  validatePolicy,
  validateQueueOwnership,
  validateReceiptIndex,
  validateRevisionPins,
  validateSnapshot,
} from "./check-sol-docs"

const repositoryRoot = resolve(import.meta.dir, "..")
const generatedAt = "2026-07-12T20:45:00.000Z"

function issue(number = 1001): RoadmapIssue {
  return {
    number,
    title: `Issue ${number}`,
    url: `https://github.com/OpenAgentsInc/openagents/issues/${number}`,
    state: "OPEN",
    labels: ["roadmap:sol"],
  }
}

function snapshot(number = 1001): RoadmapIssueSnapshot {
  return {
    schemaVersion: 1,
    generatedAt,
    repository: "OpenAgentsInc/openagents",
    label: "roadmap:sol",
    excludedLabels: ["area:docs"],
    maxAgeHours: 168,
    issues: [issue(number)],
  }
}

describe("positive repository fixture", () => {
  test("the checked-in Sol documentation passes offline", async () => {
    const errors = await collectSolDocErrors({
      root: repositoryRoot,
      now: new Date(),
    })
    expect(errors).toEqual([])
  })
})

describe("snapshot freshness and connected equality", () => {
  test("accepts a fresh, schema-versioned artifact", () => {
    expect(validateSnapshot(snapshot(), new Date(generatedAt))).toEqual([])
  })

  test("rejects malformed, future, and expired artifacts", () => {
    const malformed = { ...snapshot(), schemaVersion: 2, excludedLabels: [] }
    expect(validateSnapshot(malformed, new Date(generatedAt)).join("\n")).toContain("schemaVersion")
    expect(validateSnapshot(snapshot(), new Date("2026-07-10T00:00:00.000Z")).join("\n")).toContain("future")
    expect(validateSnapshot(snapshot(), new Date("2026-07-25T00:00:00.000Z")).join("\n")).toContain("older")
  })

  test("rejects a live GitHub divergence", () => {
    expect(compareLiveIssues(snapshot(), [issue(2)]).join("\n")).toContain("differs")
  })
})

describe("authority, queue, and issue classification regressions", () => {
  const master = "# Master\n\n- Revision: 1\n\n### Canonical open issue projection\n\n| Issue | Role |\n| --- | --- |\n| [#1001](https://github.com/OpenAgentsInc/openagents/issues/1001) | current |\n"

  test("rejects master projection drift and size overflow", () => {
    expect(validateMaster(master.replaceAll("issues/1001", "issues/1002"), snapshot()).join("\n")).toContain("differs")
    expect(validateMaster(`${master}${"extra\n".repeat(801)}`, snapshot()).join("\n")).toContain("budget")
  })

  test("rejects a hard-coded revision in an active document", () => {
    expect(validateRevisionPins({ "active.md": "- Status: current under Master Roadmap Revision 9" }).join("\n")).toContain("hard-codes")
  })

  test("rejects duplicate queue owners", () => {
    const documents = {
      "docs/sol/MASTER_ROADMAP.md": master,
      "docs/sol/README.md": "## Dispatch-safe reading order\n\n1. [Master](./MASTER_ROADMAP.md)",
      "docs/sol/other.md": "### Canonical open issue projection",
    }
    expect(validateQueueOwnership(documents).join("\n")).toContain("owned only")
  })

  test("rejects historical Start-here declarations and dispatch targets", () => {
    const documents = {
      "docs/sol/MASTER_ROADMAP.md": master,
      "docs/sol/README.md": "## Dispatch-safe reading order\n\n1. [Old](./old.md)",
      "docs/sol/old.md": "- Class: historical\n- Status: retired\n\n## Start here",
    }
    const result = validateQueueOwnership(documents).join("\n")
    expect(result).toContain("declares a Start here")
    expect(result).toContain("dispatch-safe reading order lists")
  })

  test("rejects missing classification, live-set drift, and open-as-closed", () => {
    const index = "# Issues\n\n## Live issue sources\n\n| Source | Live issue |\n| --- | --- |\n| [source](./source.md) | #1001 |\n\n## Live issues represented by receipts\n\nNo receipt-only issues.\n\n## Closed proof and implementation sources\n\n- [closed](./closed.md) — #1001\n\n## Closed non-revival tombstones\n\nNo tombstones.\n\n## Architecture reference\n\nNo references.\n"
    const result = validateIssueIndex(index, ["source.md", "closed.md", "missing.md"], snapshot()).join("\n")
    expect(result).toContain("missing.md is classified 0 times")
    expect(result).toContain("#1001 is classified as closed")
    expect(validateIssueIndex(index.replace("#1001 |", "#1002 |"), ["source.md", "closed.md"], snapshot()).join("\n")).toContain("differs")
  })
})

describe("policy, receipt, archive, and link regressions", () => {
  test("rejects physical Android gating and persona-neutral voice pauses", () => {
    const result = validatePolicy({
      "bad.md": "Nothing gates on physical Android elsewhere.\nThis release requires physical Android.\nWe paused persona-neutral voice.",
    }).join("\n")
    expect(result).toContain("revives physical Android")
    expect(result).toContain("pauses or removes persona-neutral voice")
  })

  test("rejects receipts without snapshot, proof rung, disposition, or date", () => {
    const bad = "| Evidence | Use |\n| --- | --- |\n| [undated](../receipt.md) | vague |\n"
    const result = validateReceiptIndex(bad).join("\n")
    expect(result).toContain("lacks snapshot")
    expect(result).toContain("expected 4")
    expect(result).toContain("no dated snapshot")
  })

  test("rejects missing Backroom/OpenAgents commit provenance", () => {
    const manifest = `
- Source repository: \`OpenAgentsInc/openagents\`
- Destination repository: \`OpenAgentsInc/backroom\`
- archive/openagents-sol-docs-2026-07-12/july9/
- Backroom import: \`dec8ae52\`
- OpenAgents link migration and source removal: \`b62ad88136\`
- Backroom final bidirectional receipt: \`b9645456\`
`
    expect(validateArchiveManifest(manifest).join("\n")).toContain("OpenAgents completed manifest")
  })

  describe("internal Markdown links", () => {
    let root = ""
    afterEach(() => {
      if (root) rmSync(root, { recursive: true, force: true })
    })

    test("rejects a broken internal target and a removed July 9 target", async () => {
      root = mkdtempSync(join(tmpdir(), "sol-doc-links-"))
      const file = join(root, "docs/sol/test.md")
      mkdirSync(dirname(file), { recursive: true })
      writeFileSync(
        file,
        "[missing](./missing.md)\n[removed](./2026-07-09-issue-triage.md)\n",
      )
      const result = (await validateMarkdownLinks(root, [file])).join("\n")
      expect(result).toContain("broken internal link")
      expect(result).toContain("removed archive source")
    })
  })
})
