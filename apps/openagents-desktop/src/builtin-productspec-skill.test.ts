import { describe, expect, test } from "bun:test"
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  BuiltinProductSpecSkillError,
  ProductSpecWorkSkillSha256,
  verifyBuiltinProductSpecWorkSkill,
} from "./builtin-productspec-skill.ts"

const sourceRoot = path.resolve(import.meta.dir, "../resources/builtin-skills")
const withCopy = (run: (root: string) => void): void => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-productspec-work-"))
  try {
    cpSync(sourceRoot, root, { recursive: true })
    run(root)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
}

describe("built-in productspec-work compatibility asset", () => {
  test("pins one product-owned proposal-only skill to the signed compatibility digest", () => {
    const verified = verifyBuiltinProductSpecWorkSkill(sourceRoot)
    expect(verified.sha256).toBe(ProductSpecWorkSkillSha256)
    expect(verified.manifest).toEqual({
      schema: "openagents.desktop.builtin_skill_manifest.v1",
      compatibilitySetVersion: 1,
      skills: [{
        name: "productspec-work",
        version: "0.1.0",
        relativePath: "productspec-work/SKILL.md",
        sha256: ProductSpecWorkSkillSha256,
        authority: "proposal_only",
        installationScope: "named_isolated_codex_home",
        registrationSurface: "codex_app_server_native",
        ambientFallback: false,
        defaultCodexHomeAllowed: false,
      }],
    })

    const text = readFileSync(verified.skillPath, "utf8")
    for (const required of [
      "path@revision+digest#criterion-id",
      "At most one mutation lease may be active for a work packet.",
      "`evidence-present` is not `verified`.",
      "approve or apply a ProductSpec edit",
      "mark a criterion verified, accepted, or waived",
      "Never search for or fall back to an ambient",
    ]) expect(text).toContain(required)
  })

  test("fails closed when the manifest or asset moves outside the compatibility set", () => {
    withCopy(root => {
      const manifestPath = path.join(root, "manifest.json")
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as { skills: Array<{ sha256: string }> }
      manifest.skills[0]!.sha256 = "0".repeat(64)
      writeFileSync(manifestPath, `${JSON.stringify(manifest)}\n`)
      expect(() => verifyBuiltinProductSpecWorkSkill(root)).toThrow(BuiltinProductSpecSkillError)
      try {
        verifyBuiltinProductSpecWorkSkill(root)
      } catch (error) {
        expect(error).toMatchObject({ reason: "manifest_digest_mismatch" })
      }
    })

    withCopy(root => {
      writeFileSync(path.join(root, "productspec-work", "SKILL.md"), "tampered\n")
      try {
        verifyBuiltinProductSpecWorkSkill(root)
      } catch (error) {
        expect(error).toMatchObject({ reason: "asset_digest_mismatch" })
      }
    })
  })
})
