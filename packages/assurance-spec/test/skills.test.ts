import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import path from "node:path"

const packageRoot = path.resolve(import.meta.dirname, "..")
const readSkill = (name: string): string =>
  readFileSync(path.join(packageRoot, "skills", name, "SKILL.md"), "utf8")

describe("AssuranceSpec repository-installable skills", () => {
  test("work skill binds exact identity, staleness, eight axes, typed gaps, and authority limits", () => {
    const text = readSkill("assurancespec-work")
    for (const required of [
      "<assurance-spec path>@<revision>+<digest>#<obligation-id>",
      "begin_assurance_session",
      "check_assurance_session",
      "`evidence-present` is not `CONFIRMED`",
      "`CONFIRMED` is not accepted",
      "skip-and-green",
      "admit an AssuranceSpec or mutate its lifecycle state",
      "claim verification or completion authority",
    ]) expect(text).toContain(required)
  })

  test("authoring skill uses proposal-first laws and current validator vocabulary", () => {
    const text = readSkill("assurancespec-authoring")
    for (const required of [
      "assurance-spec propose",
      "one obligation per proof claim",
      "oracle and a falsifier",
      "Two mock-only component tests do not prove their wiring",
      "fixture-tier pass remains fixture-tier evidence",
      "Never renumber or reuse an ID. Supersede it",
      "assurance-spec validate",
      "assurance-spec coverage",
      "`lifecycle_state: proposed`",
      "`missing_obligation_criterion_ref`",
      "`uncovered_acceptance_criterion`",
      "`dangling_environment_ref`",
      "`missing_oracle`",
      "`missing_falsifier`",
    ]) expect(text).toContain(required)
  })

  test("ships the progressive authoring references", () => {
    for (const name of ["authoring", "oracles-and-falsifiers", "seams", "environments"]) {
      const text = readFileSync(
        path.join(packageRoot, "skills", "assurancespec-authoring", "references", `${name}.md`),
        "utf8",
      )
      expect(text.length).toBeGreaterThan(200)
    }
  })
})
