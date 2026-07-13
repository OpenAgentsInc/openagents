import { describe, expect, test } from "bun:test"
import { cpSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  AssuranceSpecWorkSkillSha256,
  BuiltinProductSpecSkillError,
  ProductSpecWorkSkillSha256,
  verifyBuiltinAssuranceSpecWorkSkill,
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
  test("pins the product-owned proposal-only skills to the signed compatibility digest", () => {
    const verified = verifyBuiltinProductSpecWorkSkill(sourceRoot)
    const assurance = verifyBuiltinAssuranceSpecWorkSkill(sourceRoot)
    expect(verified.sha256).toBe(ProductSpecWorkSkillSha256)
    expect(assurance.sha256).toBe(AssuranceSpecWorkSkillSha256)
    expect(verified.manifest).toEqual({
      schema: "openagents.desktop.builtin_skill_manifest.v1",
      compatibilitySetVersion: 4,
      skills: [{
        name: "productspec-work",
        version: "0.2.0",
        relativePath: "productspec-work/SKILL.md",
        sha256: ProductSpecWorkSkillSha256,
        authority: "proposal_only",
        installationScope: "app_owned_extra_root",
        registrationSurface: "codex_app_server_native",
        ambientFallback: false,
        defaultCodexSessionAllowed: true,
        defaultCodexHomeMutationAllowed: false,
      }, {
        name: "assurancespec-work",
        version: "0.1.0",
        relativePath: "assurancespec-work/SKILL.md",
        sha256: AssuranceSpecWorkSkillSha256,
        authority: "proposal_only",
        installationScope: "app_owned_extra_root",
        registrationSurface: "codex_app_server_native",
        ambientFallback: false,
        defaultCodexSessionAllowed: true,
        defaultCodexHomeMutationAllowed: false,
      }],
    })

    const text = readFileSync(verified.skillPath, "utf8")
    for (const required of [
      "path@revision+digest#criterion-id",
      "At most one mutation lease may be active for a work packet.",
      "`evidence-present` is not `verified`.",
      "approve or apply a ProductSpec edit",
      "mark a criterion verified, accepted, or waived",
      "fall back to an ambient",
      "`incompatible_workflow`",
      "`get_run`, `propose_edit`,",
    ]) expect(text).toContain(required)

    const assuranceText = readFileSync(assurance.skillPath, "utf8")
    const publicAssuranceText = readFileSync(
      path.resolve(import.meta.dir, "../../../packages/assurance-spec/skills/assurancespec-work/SKILL.md"),
      "utf8",
    )
    expect(assuranceText).toBe(publicAssuranceText)
    for (const required of [
      "<assurance-spec path>@<revision>+<digest>#<obligation-id>",
      "`evidence-present` is not `CONFIRMED`",
      "`CONFIRMED` is not accepted",
      "skip-and-green result",
      "admit an AssuranceSpec or mutate its lifecycle state",
      "claim verification or completion authority",
      "fall back to an ambient",
    ]) expect(assuranceText).toContain(required)
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

    withCopy(root => {
      writeFileSync(path.join(root, "assurancespec-work", "SKILL.md"), "tampered\n")
      try {
        verifyBuiltinAssuranceSpecWorkSkill(root)
      } catch (error) {
        expect(error).toMatchObject({ reason: "asset_digest_mismatch" })
      }
    })
  })
})
