/**
 * AS-1 conformance corpus and schema/parser parity (#8760).
 *
 * The corpus is how the format stops being "whatever the one parser accepts"
 * (GAP_ANALYSIS §7; ASSURANCE_SPEC.md §12.4):
 *
 * - every valid fixture parses, validates, and round-trips BYTE-stable;
 * - every invalid fixture fails with the stable error code its filename
 *   declares (`<code-kebab>[--variant].assurance-spec.md`);
 * - every implemented error code has at least one invalid fixture, enforced
 *   mechanically against the exported code registries — a new code cannot
 *   land without growing the corpus;
 * - format-version discipline: any change that can make a previously valid
 *   document invalid must bump ASSURANCE_SPEC_FORMAT_VERSION and freeze the
 *   previous corpus per version. Fixtures are frozen bytes; do not regenerate
 *   them to make a serializer change pass.
 */
import { describe, expect, test } from "bun:test"
import { readdirSync, readFileSync } from "node:fs"
import { resolve } from "node:path"

import {
  ASSURANCE_FALSE_GREEN_IDENTIFIERS,
  ASSURANCE_NON_FIXTURE_ERROR_CODES,
  ASSURANCE_REVIEW_ERROR_CODES,
  ASSURANCE_SPEC_FORMAT_VERSION,
  ASSURANCE_STRUCTURAL_ERROR_CODES,
  ASSURANCE_STRUCTURED_BLOCK_NAMES,
  AssuranceSpecFrontmatterSchema,
  AssuranceSpecParseError,
  KNOWN_FRONTMATTER_KEYS,
  bindAssuranceReviewAnnotation,
  parseAssuranceSpec,
  serializeAssuranceSpec,
  validateAssuranceReviewAnnotation,
  validateAssuranceSpec,
} from "../src/index.ts"
import { sha256Digest } from "../src/tooling.ts"

const conformanceDir = resolve(import.meta.dirname, "../conformance")
const mvpAssurancePath = resolve(conformanceDir, "valid/mvp-proposal.assurance-spec.md")

const fixture = (relative: string): string => readFileSync(resolve(conformanceDir, relative), "utf8")
const names = (relative: string, extension: string): ReadonlyArray<string> =>
  readdirSync(resolve(conformanceDir, relative))
    .filter((name) => name.endsWith(extension))
    .map((name) => name.slice(0, -extension.length))
    .sort()

/** `<code-kebab>[--variant]` filename → stable snake_case error code. */
const expectedCode = (name: string): string => name.split("--")[0]!.replaceAll("-", "_")

describe("conformance corpus: valid documents", () => {
  const validNames = names("valid", ".assurance-spec.md")

  test("the corpus has valid fixtures", () => {
    expect(validNames.length).toBeGreaterThanOrEqual(5)
  })

  for (const name of validNames) {
    test(`valid/${name} validates and round-trips byte-stable`, () => {
      const source = fixture(`valid/${name}.assurance-spec.md`)
      const validation = validateAssuranceSpec(source)
      expect(validation.errors).toEqual([])
      expect(validation.valid).toBe(true)
      const document = parseAssuranceSpec(source)
      expect(serializeAssuranceSpec(document)).toBe(source)
      expect(parseAssuranceSpec(serializeAssuranceSpec(document))).toEqual(document)
    })

    test(`valid/${name} declares the supported format version`, () => {
      const document = parseAssuranceSpec(fixture(`valid/${name}.assurance-spec.md`))
      expect(document.frontmatter.assurance_spec_format_version).toBe(ASSURANCE_SPEC_FORMAT_VERSION)
    })
  }

  test("the corpus retains the frozen MVP revision-1 proposal bytes", () => {
    expect(fixture("valid/mvp-proposal.assurance-spec.md")).toBe(readFileSync(mvpAssurancePath, "utf8"))
  })

  test("custom sections round-trip with ids, order, and content preserved", () => {
    const document = parseAssuranceSpec(fixture("valid/with-custom-section.assurance-spec.md"))
    expect(document.customSections.map((section) => section.id)).toEqual([
      "custom-owner-gates",
      "custom-review-notes",
    ])
    expect(document.customSections[0]!.content).toContain("owner must sign off")
  })

  test("unknown frontmatter round-trips verbatim in authored order", () => {
    const document = parseAssuranceSpec(fixture("valid/with-unknown-frontmatter.assurance-spec.md"))
    expect(document.unknownFrontmatter).toEqual([
      { key: "created_at", raw: "\"2026-07-13T00:00:00Z\"" },
      { key: "linked_github_repo", raw: "\"OpenAgentsInc/openagents\"" },
    ])
  })

  test("custom-section preservation does not change serialization without custom sections", () => {
    const document = parseAssuranceSpec(fixture("valid/minimal.assurance-spec.md"))
    expect(document.customSections).toEqual([])
    expect(document.unknownFrontmatter).toEqual([])
  })
})

describe("conformance corpus: thin and empty section warnings", () => {
  test("skeleton documents warn thin_required_section, including the generated MVP proposal", () => {
    for (const name of ["minimal", "mvp-proposal"]) {
      const validation = validateAssuranceSpec(fixture(`valid/${name}.assurance-spec.md`))
      expect(validation.valid).toBe(true)
      expect(validation.warnings.map((warning) => warning.code)).toContain("thin_required_section")
      expect(validation.warnings.every((warning) => warning.severity === "warning")).toBe(true)
    }
  })

  test("a designed document with real narratives produces zero warnings", () => {
    const validation = validateAssuranceSpec(fixture("valid/designed.assurance-spec.md"))
    expect(validation.valid).toBe(true)
    expect(validation.warnings).toEqual([])
  })

  test("an emptied narrative warns empty_required_section without failing validity", () => {
    const source = fixture("valid/designed.assurance-spec.md")
    const emptied = source.replace(
      "## Gates\n\nOne activation gate arms the obligation when the designed evidence set exists and remains fresh. The gate expression below is a designed policy statement for later compilation, and evaluating it is a separate execution concern. A gate never admits, approves, or releases anything by itself under the declared authority boundaries.",
      "## Gates\n",
    )
    const validation = validateAssuranceSpec(emptied)
    expect(validation.valid).toBe(true)
    expect(validation.warnings).toEqual([{
      code: "empty_required_section",
      message: "Mandatory section has no narrative content: gates",
      severity: "warning",
      path: "sections.gates",
    }])
  })
})

describe("conformance corpus: invalid documents", () => {
  const invalidNames = names("invalid", ".assurance-spec.md")

  for (const name of invalidNames) {
    test(`invalid/${name} fails with ${expectedCode(name)}`, () => {
      const validation = validateAssuranceSpec(fixture(`invalid/${name}.assurance-spec.md`))
      expect(validation.valid).toBe(false)
      expect(validation.errors.map((error) => error.code)).toContain(expectedCode(name))
    })
  }

  test("every implemented structural error code has at least one invalid fixture", () => {
    const covered = new Set(invalidNames.map(expectedCode))
    for (const code of ASSURANCE_STRUCTURAL_ERROR_CODES) {
      expect(covered.has(code)).toBe(true)
    }
  })

  test("every invalid fixture names a registered code", () => {
    const registered = new Set<string>([
      ...ASSURANCE_STRUCTURAL_ERROR_CODES,
      ...ASSURANCE_NON_FIXTURE_ERROR_CODES,
    ])
    for (const name of invalidNames) {
      expect(registered.has(expectedCode(name))).toBe(true)
    }
  })

  test("referential integrity is enforced at parse time, not only by the validator", () => {
    const source = fixture("invalid/dangling-source-ref.assurance-spec.md")
    expect(() => parseAssuranceSpec(source)).toThrow(AssuranceSpecParseError)
    try {
      parseAssuranceSpec(source)
    } catch (error) {
      expect((error as AssuranceSpecParseError).diagnostic.code).toBe("dangling_source_ref")
    }
  })
})

describe("conformance corpus: review annotations", () => {
  const mvpBytes = readFileSync(mvpAssurancePath, "utf8")
  const mvpDocument = parseAssuranceSpec(mvpBytes)
  const mvpSubject = { document: mvpDocument, documentDigest: sha256Digest(mvpBytes) }

  const reviewDiagnostics = (json: string): ReadonlyArray<string> => {
    const validation = validateAssuranceReviewAnnotation(json)
    const codes = validation.errors.map((error) => error.code)
    if (validation.annotation !== undefined && validation.valid) {
      codes.push(...bindAssuranceReviewAnnotation(validation.annotation, mvpSubject).errors.map((error) => error.code))
    }
    return codes
  }

  const validNames = names("review/valid", ".assurance-review.json")
  const invalidNames = names("review/invalid", ".assurance-review.json")

  for (const name of validNames) {
    test(`review/valid/${name} validates and binds the exact MVP subject`, () => {
      const source = fixture(`review/valid/${name}.assurance-review.json`)
      const validation = validateAssuranceReviewAnnotation(source)
      expect(validation.errors).toEqual([])
      expect(validation.valid).toBe(true)
      const binding = bindAssuranceReviewAnnotation(validation.annotation!, mvpSubject)
      expect(binding.errors).toEqual([])
      expect(binding.bound).toBe(true)
    })
  }

  for (const name of invalidNames) {
    test(`review/invalid/${name} fails with ${expectedCode(name)}`, () => {
      expect(reviewDiagnostics(fixture(`review/invalid/${name}.assurance-review.json`))).toContain(expectedCode(name))
    })
  }

  test("every review error code has at least one invalid fixture", () => {
    const covered = new Set(invalidNames.map(expectedCode))
    for (const code of ASSURANCE_REVIEW_ERROR_CODES) {
      expect(covered.has(code)).toBe(true)
    }
  })

  test("a review bound to different bytes is rejected even when the id and revision match", () => {
    const source = fixture("review/valid/mvp-proposal-review.assurance-review.json")
    const annotation = validateAssuranceReviewAnnotation(source).annotation!
    const binding = bindAssuranceReviewAnnotation(annotation, {
      document: mvpDocument,
      documentDigest: sha256Digest(`${mvpBytes} `),
    })
    expect(binding.bound).toBe(false)
    expect(binding.errors.map((error) => error.code)).toEqual(["review_subject_digest_mismatch"])
  })
})

describe("schema/parser parity", () => {
  test("Episode 252 false-green identifiers remain exact and ordered", () => {
    expect(ASSURANCE_FALSE_GREEN_IDENTIFIERS).toEqual([
      "false_green_fixture_assert",
      "false_green_api_mirror",
      "false_green_mocked_seam",
      "false_green_coverage_theater",
      "false_green_round_up",
    ])
  })

  test("the parser's known frontmatter keys are exactly the schema's fields", () => {
    expect([...KNOWN_FRONTMATTER_KEYS].sort()).toEqual(
      Object.keys(AssuranceSpecFrontmatterSchema.fields).sort(),
    )
  })

  test("the serializer emits exactly the schema's frontmatter fields for a profile-only document", () => {
    const source = fixture("valid/minimal.assurance-spec.md")
    const raw = /^---\n([\s\S]*?)\n---\n/.exec(serializeAssuranceSpec(parseAssuranceSpec(source)))![1]!
    const emitted = raw.split("\n").map((line) => line.split(":")[0]!)
    expect(emitted.sort()).toEqual(Object.keys(AssuranceSpecFrontmatterSchema.fields).sort())
  })

  test("parser and serializer share one structured-block-name map covering the block-bearing sections", () => {
    expect(Object.keys(ASSURANCE_STRUCTURED_BLOCK_NAMES).sort()).toEqual([
      "authority_boundaries",
      "environments",
      "evidence_policy",
      "gates",
      "obligations",
      "risk_model",
      "subject",
    ])
  })

  test("format-version discipline is pinned: bumping the version requires new corpus fixtures", () => {
    // If this assertion fails you changed the format version. Freeze the
    // current conformance fixtures under a per-version directory and seed the
    // new version's corpus before landing (ASSURANCE_SPEC.md §13).
    expect(ASSURANCE_SPEC_FORMAT_VERSION).toBe("0.1")
  })
})
