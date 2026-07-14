import { mkdir, writeFile } from "node:fs/promises"
import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { spawnSync } from "node:child_process"

import { inventoryRepository, proposeAssuranceSpec } from "../src/index.ts"

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const git = (root: string, args: string[]) => {
  const result = spawnSync("git", ["-C", root, ...args], { stdio: "pipe" })
  if (result.status !== 0) throw new Error(result.stderr.toString())
}

const fixture = async (): Promise<string> => {
  const root = mkdtempSync(join(tmpdir(), "assurance-spec-repo-"))
  roots.push(root)
  git(root, ["init", "-q"])
  await mkdir(join(root, "tests"))
  await mkdir(join(root, "src"))
  await writeFile(join(root, "package.json"), JSON.stringify({ scripts: { test: "vp test", verify: "vp test && tsc --noEmit" } }, null, 2))
  await writeFile(join(root, "tests", "CW-AC-04-passes.test.ts"), "test('looks persuasive', () => expect(true).toBe(true))\n")
  await writeFile(join(root, "src", "main.ts"), "export const value = 1\n")
  git(root, ["add", "."])
  git(root, ["-c", "user.name=OpenAgents Test", "-c", "user.email=test@openagents.invalid", "commit", "-qm", "fixture"])
  return root
}

describe("repository proposal inventory", () => {
  test("binds candidate facts to committed HEAD without selecting proof", async () => {
    const root = await fixture()
    const inventory = inventoryRepository(root)
    expect(inventory.state).toBe("clean")
    expect(inventory.head).toMatch(/^[a-f0-9]{40,64}$/)
    expect(inventory.tree).toMatch(/^[a-f0-9]{40,64}$/)
    expect(inventory.candidate_artifact_refs).toEqual(["tests/CW-AC-04-passes.test.ts"])
    expect(inventory.declared_scripts.map((script) => script.name)).toEqual(["test", "verify"])
    expect(JSON.stringify(inventory)).not.toContain(root)
    const productSpec = `---\nspec_format_version: "0.1"\ntitle: "Fixture"\nartifact_type: "prd"\nspec_revision: 1\nauthor: "OpenAgents"\ncreated_at: "2026-07-13T00:00:00Z"\nupdated_at: "2026-07-13T00:00:00Z"\n---\n\n## Problem\n\nA real product problem exists for this fixture.\n\n## Hypothesis\n\nIf this ships, the fixture behavior improves measurably.\n\n## Scope\n\nThis fixture covers one bounded behavior only.\n\n## Acceptance Criteria\n\n- **CW-AC-04:** The actual criterion must be proved.\n\n## Success Metrics\n\nThe fixture has one reviewed success measure.\n`
    const proposal = proposeAssuranceSpec({ productSpecPath: "fixture.product-spec.md", productSpecMarkdown: productSpec, repositoryInventory: inventory })
    expect(proposal.ok).toBe(true)
    if (!proposal.ok) return
    expect(proposal.document.obligations[0]?.candidate_artifact_refs).toEqual([])
    expect(proposal.document.obligations[0]?.oracle).toBeUndefined()
    expect(proposal.adequacy.coverage.needs_design).toBe(1)
    expect(proposal.adequacy.diagnostics.map((diagnostic) => diagnostic.code)).toContain("repository_candidates_unmapped")
  })

  test("dirty tracked bytes change state but not the committed candidate inventory", async () => {
    const root = await fixture()
    const clean = inventoryRepository(root)
    await writeFile(join(root, "tests", "CW-AC-04-passes.test.ts"), "throw new Error('dirty')\n")
    await writeFile(join(root, "tests", "untracked-secret.test.ts"), "secret\n")
    const dirty = inventoryRepository(root)
    expect(dirty.state).toBe("dirty")
    expect(dirty.head).toBe(clean.head)
    expect(dirty.tree).toBe(clean.tree)
    expect(dirty.candidate_artifact_refs).toEqual(clean.candidate_artifact_refs)
    expect(dirty.candidate_artifact_refs).not.toContain("tests/untracked-secret.test.ts")
    expect(dirty.inventory_digest).not.toBe(clean.inventory_digest)
  })
})
