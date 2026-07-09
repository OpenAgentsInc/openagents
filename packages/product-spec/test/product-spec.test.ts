import { describe, expect, test } from "bun:test"
import { readdirSync, statSync } from "node:fs"
import { join, resolve } from "node:path"

import {
  OPENAGENTS_CUSTOM_SECTIONS,
  PRODUCT_SPEC_EXTENSION,
  parseProductSpec,
  starterProductSpec,
  stripToolMetadata,
  validateProductSpec,
} from "../src/index.ts"

const packageRoot = resolve(import.meta.dir, "..")
const repoRoot = resolve(packageRoot, "../..")

const readFixture = async (relativePath: string): Promise<string> =>
  Bun.file(join(packageRoot, "fixtures", relativePath)).text()

const listSpecFiles = (root: string): string[] => {
  const results: string[] = []
  const walk = (dir: string) => {
    for (const entry of readdirSync(dir)) {
      const path = join(dir, entry)
      if (statSync(path).isDirectory()) walk(path)
      else if (path.endsWith(PRODUCT_SPEC_EXTENSION)) results.push(path)
    }
  }
  walk(root)
  return results.sort()
}

describe("upstream conformance corpus", () => {
  const validFixtures = [
    "minimal",
    "with-ai-evals",
    "with-custom-section",
    "with-structured-scope-and-metrics",
    "with-user-experience",
  ]
  for (const name of validFixtures) {
    test(`accepts valid/${name}`, async () => {
      const result = validateProductSpec(
        await readFixture(`conformance/valid/${name}.product-spec.md`),
      )
      expect(result.valid).toBe(true)
      expect(result.errors).toHaveLength(0)
    })
  }

  const invalidFixtures: Array<[string, string]> = [
    ["missing-frontmatter", "missing_frontmatter"],
    ["missing-required-section", "missing_required_section"],
    ["unsupported-version", "unsupported_version"],
  ]
  for (const [name, expectedCode] of invalidFixtures) {
    test(`rejects invalid/${name} with ${expectedCode}`, async () => {
      const result = validateProductSpec(
        await readFixture(`conformance/invalid/${name}.product-spec.md`),
      )
      expect(result.valid).toBe(false)
      expect(result.errors.map((error) => error.code)).toContain(expectedCode)
    })
  }
})

describe("parsed structure", () => {
  test("custom section labels map to declared ids", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-custom-section.product-spec.md"),
    )
    expect(document.sections.map((section) => section.id)).toContain("custom-research-notes")
  })

  test("structured blocks are extracted with types intact", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-structured-scope-and-metrics.product-spec.md"),
    )
    const scope = document.sections.find((section) => section.id === "scope")?.scope
    expect(scope).toBeDefined()
    expect(scope!.in.length).toBeGreaterThan(0)
    const metrics = document.sections.find(
      (section) => section.id === "success_metrics",
    )?.success_metrics
    expect(metrics).toBeDefined()
    expect(metrics![0]?.id).toMatch(/^[a-z0-9_]+$/)
  })

  test("ai evals parse pass_threshold and checks", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-ai-evals.product-spec.md"),
    )
    const evals = document.sections.find(
      (section) => section.id === "acceptance_criteria",
    )?.ai_evals
    expect(evals).toBeDefined()
    expect(evals![0]?.pass_threshold).toBeGreaterThan(0)
    expect(evals![0]?.checks.length).toBeGreaterThan(0)
  })
})

describe("openagents extensions", () => {
  test("extended fixture with custom sections + tool_metadata validates", async () => {
    const markdown = await readFixture("openagents/valid-extended.product-spec.md")
    const result = validateProductSpec(markdown)
    expect(result.valid).toBe(true)
    expect(result.document?.frontmatter.tool_metadata?.openagents_epic).toBe("8593")
    const ids = result.document?.sections.map((section) => section.id) ?? []
    for (const custom of OPENAGENTS_CUSTOM_SECTIONS) {
      expect(ids).toContain(custom.id)
    }
  })

  test("malformed success metric fixture is rejected", async () => {
    const result = validateProductSpec(
      await readFixture("openagents/invalid-bad-metric.product-spec.md"),
    )
    expect(result.valid).toBe(false)
    expect(result.errors.map((error) => error.code)).toContain("invalid_success_metric")
  })

  test("stripToolMetadata removes the block and keeps the document valid", async () => {
    const markdown = await readFixture("openagents/valid-extended.product-spec.md")
    const stripped = stripToolMetadata(markdown)
    const strippedFrontmatter = /^---\n([\s\S]*?)\n---\n/.exec(stripped)?.[1] ?? ""
    expect(strippedFrontmatter).not.toContain("tool_metadata")
    expect(strippedFrontmatter).not.toContain("openagents_epic")
    const result = validateProductSpec(stripped)
    expect(result.valid).toBe(true)
    expect(result.document?.frontmatter.tool_metadata).toBeUndefined()
  })

  test("starter spec validates and carries the custom section stubs", () => {
    const markdown = starterProductSpec({
      title: "Starter Fixture",
      now: "2026-07-08T00:00:00Z",
    })
    const result = validateProductSpec(markdown)
    expect(result.valid).toBe(true)
    const ids = result.document?.sections.map((section) => section.id) ?? []
    expect(ids).toContain("custom-owner-gates")
    expect(ids).toContain("custom-receipts")
    expect(ids).toContain("custom-promise-links")
  })
})

describe("repo specs tree gate", () => {
  const specsRoot = join(repoRoot, "specs")
  const specFiles = listSpecFiles(specsRoot)

  test("specs/ contains at least one Product Spec", () => {
    expect(specFiles.length).toBeGreaterThan(0)
  })

  for (const path of specFiles) {
    test(`validates ${path.slice(repoRoot.length + 1)}`, async () => {
      const result = validateProductSpec(await Bun.file(path).text())
      expect(result.errors).toHaveLength(0)
      expect(result.valid).toBe(true)
    })
  }
})
