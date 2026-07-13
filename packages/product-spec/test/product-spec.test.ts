import { describe, expect, test } from "bun:test"
import { mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join, resolve } from "node:path"

import {
  OPENAGENTS_CUSTOM_SECTIONS,
  PRODUCT_SPEC_EXTENSION,
  UPSTREAM_COMPATIBILITY,
  applyProductSpecEvidenceAttachmentEdit,
  computeProductSpecDocumentDigest,
  computeProductSpecIntentDigest,
  parseProductSpec,
  planProductSpecEvidenceAttachmentEdit,
  productSpecIntentProjection,
  serializeProductSpec,
  starterProductSpec,
  stripToolMetadata,
  validateExecutableProductSpec,
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

// ---------------------------------------------------------------------------
// PSEL-0: vendored upstream 0.19.0 conformance corpus. Every valid fixture
// must pass under both profiles; every invalid fixture must fail with the
// documented error code under both profiles.
// ---------------------------------------------------------------------------

describe("upstream 0.19.0 conformance corpus", () => {
  test("the compatibility target is pinned, not chased", () => {
    expect(UPSTREAM_COMPATIBILITY.version).toBe("0.19.0")
    expect(UPSTREAM_COMPATIBILITY.commit).toBe("9ef2654bdd01aef3985fef6ed5a9ab66365999e1")
    expect(UPSTREAM_COMPATIBILITY.unsupported).toContain("Decision Trace JSON validation")
  })

  const validFixtures = [
    "minimal",
    "with-ai-evals",
    "with-custom-section",
    "with-fenced-heading",
    "with-provisional-success-metric",
    "with-spec-dependency",
    "with-structured-scope-and-metrics",
    "with-traceability",
    "with-user-experience",
  ]
  for (const name of validFixtures) {
    for (const profile of ["openagents", "upstream"] as const) {
      test(`accepts valid/${name} under the ${profile} profile`, async () => {
        const result = validateProductSpec(
          await readFixture(`conformance/valid/${name}.product-spec.md`),
          { profile },
        )
        expect(result.errors).toEqual([])
        expect(result.valid).toBe(true)
      })
    }
  }

  const invalidFixtures: Array<[string, string]> = [
    ["malformed-applies-to", "invalid_applies_to"],
    ["malformed-related-artifact", "invalid_related_artifact"],
    ["malformed-spec-dependency", "invalid_related_artifact"],
    ["missing-frontmatter", "missing_frontmatter"],
    ["missing-required-section", "missing_required_section"],
    ["unsupported-version", "unsupported_version"],
  ]
  for (const [name, expectedCode] of invalidFixtures) {
    for (const profile of ["openagents", "upstream"] as const) {
      test(`rejects invalid/${name} with ${expectedCode} under the ${profile} profile`, async () => {
        const result = validateProductSpec(
          await readFixture(`conformance/invalid/${name}.product-spec.md`),
          { profile },
        )
        expect(result.valid).toBe(false)
        expect(result.errors.map((error) => error.code)).toContain(expectedCode)
      })
    }
  }
})

// ---------------------------------------------------------------------------
// Parsed structure: structured items, related artifacts, applies_to
// ---------------------------------------------------------------------------

describe("parsed structure", () => {
  test("custom section labels map to declared ids", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-custom-section.product-spec.md"),
    )
    expect(document.sections.map((section) => section.id)).toContain("custom-research-notes")
  })

  test("structured scope and SM-<n> success metrics are extracted", async () => {
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
    expect(metrics![0]?.id).toBe("SM-1")
    expect(metrics![0]?.target_status).toBeUndefined()
  })

  test("structured acceptance criteria carry AC-<n> ids and criterion text", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/minimal.product-spec.md"),
    )
    const criteria = document.sections.find(
      (section) => section.id === "acceptance_criteria",
    )?.acceptance_criteria
    expect(criteria?.map((item) => item.id)).toEqual(["AC-1", "AC-2", "AC-3"])
    expect(criteria?.[0]?.criterion).toContain("YouTube URLs")
  })

  test("ai evals parse EVAL-<n> ids, enums, cases, and pass_threshold", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-ai-evals.product-spec.md"),
    )
    const evals = document.sections.find(
      (section) => section.id === "acceptance_criteria",
    )?.ai_evals
    expect(evals).toBeDefined()
    expect(evals![0]?.id).toBe("EVAL-1")
    expect(evals![0]?.type).toBe("llm_judge")
    expect(evals![0]?.evaluator).toBe("llm")
    expect(evals![0]?.pass_threshold).toBe(0.85)
    expect(evals![0]?.cases).toEqual([
      { input: "Representative input for this eval.", expected: "Expected behavior for this eval." },
    ])
  })

  test("applies_to and related artifacts parse from the traceability fixture", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-traceability.product-spec.md"),
    )
    expect(document.frontmatter.applies_to).toEqual([
      { path: "apps/web/src/transcripts/" },
      { component: "transcript-search" },
    ])
    const artifacts = document.sections.find(
      (section) => section.id === "related_artifacts",
    )?.related_artifacts
    expect(artifacts).toHaveLength(2)
    expect(artifacts?.[0]).toEqual({
      type: "github_issue",
      url: "https://github.com/acme/transcripts/issues/123",
      title: "Build transcript search",
      section_id: "acceptance_criteria",
      item_id: "AC-1",
    })
  })

  test("product_spec dependencies default relation to relates_to when omitted", () => {
    const document = parseProductSpec(specWith({
      relatedArtifacts: `- type: product_spec
  product_spec_path: "./other.product-spec.md"`,
    }))
    const artifact = document.sections.find((s) => s.id === "related_artifacts")
      ?.related_artifacts?.[0]
    expect(artifact?.relation).toBe("relates_to")
  })

  test("headings inside fenced blocks never split sections", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-fenced-heading.product-spec.md"),
    )
    expect(document.sections.map((section) => section.id)).toEqual([
      "problem",
      "hypothesis",
      "scope",
      "acceptance_criteria",
      "success_metrics",
    ])
    expect(document.sections.find((s) => s.id === "scope")?.content).toContain("## Problem")
  })

  test("provisional success metrics keep target_status and target_owner", async () => {
    const document = parseProductSpec(
      await readFixture("conformance/valid/with-provisional-success-metric.product-spec.md"),
    )
    const metric = document.sections.find((s) => s.id === "success_metrics")
      ?.success_metrics?.[0]
    expect(metric?.target_status).toBe("provisional")
    expect(metric?.target_owner).toBe("Data lead")
  })
})

// ---------------------------------------------------------------------------
// Upstream validation semantics (synthesized documents)
// ---------------------------------------------------------------------------

const specWith = (options: {
  frontmatterExtra?: string
  acceptanceCriteria?: string
  aiEvals?: string
  successMetrics?: string
  relatedArtifacts?: string
  createdAt?: string
  updatedAt?: string
}): string => {
  const acceptance = options.acceptanceCriteria ?? `- id: AC-1
  criterion: The synthesized document validates end to end.`
  const metrics = options.successMetrics ?? `- id: SM-1
  metric: synthesized_document_validation_rate
  target: "100%"
  window: every test run`
  return `---
spec_format_version: "0.1"
title: "Synthesized Fixture"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "${options.createdAt ?? "2026-07-13T00:00:00Z"}"
updated_at: "${options.updatedAt ?? "2026-07-13T00:00:00Z"}"
${options.frontmatterExtra ? `${options.frontmatterExtra}\n` : ""}---

## Problem

A synthesized fixture exercises one validation rule in isolation cleanly.

## Hypothesis

If each upstream rule has a focused test, semantic drift is caught exactly.

## Scope

In: exactly one validation rule per synthesized document under test.

## Acceptance Criteria

\`\`\`productspec-acceptance-criteria
${acceptance}
\`\`\`
${options.aiEvals ? `\n\`\`\`productspec-ai-evals\n${options.aiEvals}\n\`\`\`\n` : ""}
## Success Metrics

\`\`\`productspec-success-metrics
${metrics}
\`\`\`
${options.relatedArtifacts ? `\n## Related Artifacts\n\n\`\`\`productspec-related-artifacts\n${options.relatedArtifacts}\n\`\`\`\n` : ""}`
}

describe("upstream validation semantics", () => {
  const codes = (markdown: string, profile: "openagents" | "upstream" = "upstream") => {
    const result = validateProductSpec(markdown, { profile })
    return result.valid ? [] : result.errors.map((error) => error.code)
  }

  test("dangling related-artifact item_id is a typed error", () => {
    expect(
      codes(specWith({
        relatedArtifacts: `- type: github_issue
  url: "https://example.com/issue/1"
  item_id: AC-9`,
      })),
    ).toContain("invalid_related_artifact")
  })

  test("a resolvable item_id passes and an unusual artifact target only warns", () => {
    const result = validateProductSpec(
      specWith({
        relatedArtifacts: `- type: eval_run
  url: "https://example.com/runs/1"
  item_id: AC-1`,
      }),
      { profile: "upstream" },
    )
    expect(result.valid).toBe(true)
    expect(result.warnings.map((warning) => warning.code)).toContain(
      "unusual_related_artifact_target",
    )
  })

  test("duplicate item ids across AC/EVAL/SM families are rejected", () => {
    expect(
      codes(specWith({
        acceptanceCriteria: `- id: AC-1
  criterion: First criterion body.
- id: AC-1
  criterion: Second criterion body.`,
      })),
    ).toContain("duplicate_item_id")
  })

  test("acceptance criterion ids must use AC-<number>", () => {
    expect(
      codes(specWith({
        acceptanceCriteria: `- id: CW-AC-01
  criterion: Legacy identity is not silently aliased.`,
      })),
    ).toContain("invalid_acceptance_criterion")
  })

  test("ai evals require enum type/evaluator, EVAL-<n> ids, and non-empty cases", () => {
    expect(
      codes(specWith({
        aiEvals: `- id: EVAL-1
  type: vibe_check
  evaluator: automated_test
  pass_threshold: 0.9`,
      })),
    ).toContain("invalid_ai_eval")
    expect(
      codes(specWith({
        aiEvals: `- id: EVAL-1
  type: llm_judge
  evaluator: llm
  pass_threshold: 0.9
  cases:
    - input: "in"
      expected: "out"`,
      })),
    ).toEqual([])
  })

  test("provisional success metrics require target_owner", () => {
    expect(
      codes(specWith({
        successMetrics: `- id: SM-1
  metric: baseline_rate
  target: tbd
  target_status: provisional
  window: weekly`,
      })),
    ).toContain("invalid_success_metric")
  })

  test("applies_to items need exactly one of path or component", () => {
    expect(
      codes(specWith({ frontmatterExtra: `applies_to:\n  - path: "a/"\n    component: "b"` })),
    ).toContain("invalid_applies_to")
    expect(codes(specWith({ frontmatterExtra: `applies_to:\n  - path: "a/"` }))).toEqual([])
  })

  test("relation/product_spec_path/revision only apply to product_spec artifacts", () => {
    expect(
      codes(specWith({
        relatedArtifacts: `- type: github_issue
  url: "https://example.com/issue/1"
  relation: depends_on`,
      })),
    ).toContain("invalid_related_artifact")
  })

  test("frontmatter datetimes must be ISO 8601", () => {
    expect(codes(specWith({ updatedAt: "yesterday" }))).toContain("invalid_datetime")
    expect(codes(specWith({ updatedAt: "yesterday" }), "openagents")).toContain(
      "invalid_datetime",
    )
  })

  test("the upstream profile requires structured AC and SM items", () => {
    const prose = specWith({}).replace(
      /```productspec-acceptance-criteria[\s\S]*?```/,
      "- Prose criteria are allowed locally but not upstream.",
    )
    expect(codes(prose)).toContain("invalid_acceptance_criterion")
    expect(codes(prose, "openagents")).toEqual([])
  })

  test("the upstream profile rejects the OpenAgents segment/source metric fields", () => {
    const withLegacyFields = specWith({
      successMetrics: `- id: SM-1
  metric: synthesized_document_validation_rate
  target: "100%"
  window: every test run
  segment: package tests
  source: bun_test`,
    })
    expect(codes(withLegacyFields)).toContain("invalid_success_metric")
    expect(codes(withLegacyFields, "openagents")).toEqual([])
  })
})

// ---------------------------------------------------------------------------
// PSEL-0: frozen revision-6 legacy baseline and recorded incompatibilities.
// These are the exact current incompatibilities as tests, not folklore.
// ---------------------------------------------------------------------------

describe("PSEL-0 legacy rev-6 baseline", () => {
  const baselinePath = join(
    packageRoot,
    "fixtures",
    "openagents",
    "legacy-rev6-mvp.product-spec.md",
  )

  test("the frozen rev-6 MVP baseline keeps its exact document digest", () => {
    const digest = computeProductSpecDocumentDigest(readFileSync(baselinePath))
    expect(digest).toBe("sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1")
  })

  test("the baseline validates and executes under the legacy profile with 18 CW-AC ids", async () => {
    const markdown = await Bun.file(baselinePath).text()
    const result = validateExecutableProductSpec(markdown)
    expect(result.executable).toBe(true)
    expect(result.errors).toEqual([])
    expect(result.criteria.map((criterion) => criterion.id)).toEqual(
      Array.from({ length: 18 }, (_, index) => `CW-AC-${String(index + 1).padStart(2, "0")}`),
    )
    expect(result.document?.frontmatter.spec_revision).toBe(6)
  })

  test("recorded incompatibility: the baseline fails the upstream profile with exactly these codes", async () => {
    const markdown = await Bun.file(baselinePath).text()
    const result = validateProductSpec(markdown, { profile: "upstream" })
    expect(result.valid).toBe(false)
    // No structured productspec-acceptance-criteria items (CW-AC-* prose is a
    // local profile) and OpenAgents-shaped success metrics (semantic snake_case
    // ids plus segment/source fields, no SM-<n>/target_status).
    expect([...new Set(result.errors.map((error) => error.code))].sort()).toEqual([
      "invalid_acceptance_criterion",
      "invalid_success_metric",
    ])
  })

  test("recorded incompatibility: legacy ai-eval and metric dialects fail upstream", async () => {
    const markdown = await readFixture("openagents/valid-extended.product-spec.md")
    expect(validateProductSpec(markdown).valid).toBe(true)
    const upstream = validateProductSpec(markdown, { profile: "upstream" })
    expect(upstream.valid).toBe(false)
    expect([...new Set(upstream.errors.map((error) => error.code))].sort()).toEqual([
      "invalid_acceptance_criterion",
      "invalid_ai_eval",
      "invalid_success_metric",
    ])
  })

  test("no silent ID aliasing: CW-AC-01 is not accepted as AC-1 anywhere", () => {
    const aliased = specWith({
      acceptanceCriteria: `- id: CW-AC-01
  criterion: Legacy identity must not silently become portable identity.`,
    })
    for (const profile of ["openagents", "upstream"] as const) {
      const result = validateProductSpec(aliased, { profile })
      expect(result.valid).toBe(false)
      expect(result.errors.map((error) => error.code)).toContain("invalid_acceptance_criterion")
    }
  })
})

// ---------------------------------------------------------------------------
// Round trips (serialize(parse(x)) reparses to the same document)
// ---------------------------------------------------------------------------

describe("round trips", () => {
  const roundTripFixtures = [
    "conformance/valid/minimal",
    "conformance/valid/with-ai-evals",
    "conformance/valid/with-custom-section",
    "conformance/valid/with-fenced-heading",
    "conformance/valid/with-provisional-success-metric",
    "conformance/valid/with-spec-dependency",
    "conformance/valid/with-structured-scope-and-metrics",
    "conformance/valid/with-traceability",
    "conformance/valid/with-user-experience",
    "openagents/valid-extended",
    "openagents/evidence-edit-before",
    "openagents/legacy-rev6-mvp",
  ]
  for (const name of roundTripFixtures) {
    test(`round-trips ${name}`, async () => {
      const document = parseProductSpec(await readFixture(`${name}.product-spec.md`))
      const reparsed = parseProductSpec(serializeProductSpec(document))
      expect(reparsed).toEqual(document)
    })
  }
})

// ---------------------------------------------------------------------------
// PSEL-1: dual digests with golden projections
// ---------------------------------------------------------------------------

const GOLDEN = {
  beforeDocument: "sha256:41fde4c62446047d3394789fb839de0e90150899c98203d0aa95462f987f9e32",
  afterDocument: "sha256:eb6f8a8808131e5594af9f73d6ecf30d8f4c14f8ac5292223a33cd02092c3e39",
  sharedIntent: "sha256:b067b1ae1510d7809ce8b2bc8def263335e5b7f99068a6a22be235b72b2a4ee8",
} as const

describe("dual digests", () => {
  test("document digest is SHA-256 over exact bytes (goldens)", async () => {
    const before = await readFixture("openagents/evidence-edit-before.product-spec.md")
    const after = await readFixture("openagents/evidence-edit-after.product-spec.md")
    expect(computeProductSpecDocumentDigest(before)).toBe(GOLDEN.beforeDocument)
    expect(computeProductSpecDocumentDigest(after)).toBe(GOLDEN.afterDocument)
    expect(computeProductSpecDocumentDigest(`${before}\n`)).not.toBe(GOLDEN.beforeDocument)
  })

  test("an evidence-attachment-only edit changes the document digest but not the intent digest", async () => {
    const before = await readFixture("openagents/evidence-edit-before.product-spec.md")
    const after = await readFixture("openagents/evidence-edit-after.product-spec.md")
    expect(computeProductSpecDocumentDigest(after)).not.toBe(
      computeProductSpecDocumentDigest(before),
    )
    expect(computeProductSpecIntentDigest(before)).toBe(GOLDEN.sharedIntent)
    expect(computeProductSpecIntentDigest(after)).toBe(GOLDEN.sharedIntent)
  })

  test("the intent projection keeps product_spec dependencies and drops evidence attachments", async () => {
    const after = await readFixture("openagents/evidence-edit-after.product-spec.md")
    const projection = productSpecIntentProjection(parseProductSpec(after))
    expect(projection.projection_version).toBe("1")
    expect(projection.product_spec_dependencies).toHaveLength(1)
    expect(projection.product_spec_dependencies[0]?.artifact.product_spec_path).toBe(
      "./valid-extended.product-spec.md",
    )
    // The evidence-only related_artifacts residue is dropped entirely.
    expect(projection.sections.map((section) => section.id)).not.toContain("related_artifacts")
    // Provenance timestamps are excluded; everything else is intent-bound.
    expect(JSON.stringify(projection)).not.toContain("2026-07-13T06:30:00Z")
    expect(projection.frontmatter.tool_metadata?.openagents_epic).toBe("8757")
    expect(projection.frontmatter.applies_to).toEqual([{ path: "packages/product-spec/" }])
  })

  test("intent-bearing edits change the intent digest", async () => {
    const before = await readFixture("openagents/evidence-edit-before.product-spec.md")
    const baseline = computeProductSpecIntentDigest(before)
    // criterion text
    expect(
      computeProductSpecIntentDigest(
        before.replace("changes the document digest", "changes the exact document digest"),
      ),
    ).not.toBe(baseline)
    // spec_revision
    expect(
      computeProductSpecIntentDigest(before.replace("spec_revision: 3", "spec_revision: 4")),
    ).not.toBe(baseline)
    // product_spec dependency (intent-bound Related Artifact)
    expect(
      computeProductSpecIntentDigest(
        before.replace("./valid-extended.product-spec.md", "./renamed.product-spec.md"),
      ),
    ).not.toBe(baseline)
    // tool_metadata (consumed by execution/policy; intent-bound by default)
    expect(
      computeProductSpecIntentDigest(before.replace('openagents_epic: "8757"', 'openagents_epic: "9999"')),
    ).not.toBe(baseline)
    // provenance timestamps are excluded
    expect(
      computeProductSpecIntentDigest(
        before.replace('updated_at: "2026-07-13T00:00:00Z"', 'updated_at: "2026-07-14T00:00:00Z"'),
      ),
    ).toBe(baseline)
  })
})

// ---------------------------------------------------------------------------
// PSEL-1: typed, owner-confirmed evidence-attachment-only edit path
// ---------------------------------------------------------------------------

describe("evidence-attachment edit path", () => {
  const load = async () => ({
    before: await readFixture("openagents/evidence-edit-before.product-spec.md"),
    after: await readFixture("openagents/evidence-edit-after.product-spec.md"),
  })

  test("plans an evidence-only edit with matching intent digests", async () => {
    const { before, after } = await load()
    const plan = planProductSpecEvidenceAttachmentEdit({
      currentMarkdown: before,
      proposedMarkdown: after,
    })
    expect(plan.ok).toBe(true)
    if (plan.ok) {
      expect(plan.kind).toBe("evidence_attachment_only")
      expect(plan.before.intentDigest).toBe(plan.after.intentDigest)
      expect(plan.before.documentDigest).not.toBe(plan.after.documentDigest)
    }
  })

  test("refuses intent drift, revision bumps, no-ops, and created_at edits", async () => {
    const { before, after } = await load()
    const refusalCode = (proposedMarkdown: string) => {
      const plan = planProductSpecEvidenceAttachmentEdit({
        currentMarkdown: before,
        proposedMarkdown,
      })
      return plan.ok ? "ok" : plan.code
    }
    expect(refusalCode(before)).toBe("document_unchanged")
    expect(
      refusalCode(after.replace("changes the document digest", "changes the byte digest")),
    ).toBe("intent_changed")
    expect(refusalCode(after.replace("spec_revision: 3", "spec_revision: 4"))).toBe(
      "spec_revision_changed",
    )
    expect(
      refusalCode(
        after.replace('created_at: "2026-07-13T00:00:00Z"', 'created_at: "2026-07-12T00:00:00Z"'),
      ),
    ).toBe("created_at_changed")
    expect(refusalCode("not a product spec")).toBe("invalid_proposed_document")
  })

  test("apply writes only after an atomic exact-byte recheck", async () => {
    const { before, after } = await load()
    const dir = mkdtempSync(join(tmpdir(), "product-spec-evidence-"))
    const path = join(dir, "fixture.product-spec.md")
    try {
      writeFileSync(path, before, "utf8")
      const expectedDocumentDigest = computeProductSpecDocumentDigest(before)

      const unconfirmed = applyProductSpecEvidenceAttachmentEdit({
        path,
        expectedDocumentDigest,
        proposedMarkdown: after,
        ownerConfirmed: false,
      })
      expect(unconfirmed.ok).toBe(false)
      if (!unconfirmed.ok) expect(unconfirmed.code).toBe("owner_confirmation_required")
      expect(readFileSync(path, "utf8")).toBe(before)

      // A concurrent edit invalidates the reviewed digest: typed refusal, no write.
      writeFileSync(path, `${before}\n`, "utf8")
      const raced = applyProductSpecEvidenceAttachmentEdit({
        path,
        expectedDocumentDigest,
        proposedMarkdown: after,
        ownerConfirmed: true,
      })
      expect(raced.ok).toBe(false)
      if (!raced.ok) expect(raced.code).toBe("document_digest_mismatch")
      expect(readFileSync(path, "utf8")).toBe(`${before}\n`)

      // Restore and apply for real.
      writeFileSync(path, before, "utf8")
      const applied = applyProductSpecEvidenceAttachmentEdit({
        path,
        expectedDocumentDigest,
        proposedMarkdown: after,
        ownerConfirmed: true,
      })
      expect(applied.ok).toBe(true)
      if (applied.ok) {
        expect(applied.before.intentDigest).toBe(applied.after.intentDigest)
        expect(applied.after.documentDigest).toBe(computeProductSpecDocumentDigest(after))
      }
      expect(readFileSync(path, "utf8")).toBe(after)
    } finally {
      rmSync(dir, { recursive: true, force: true })
    }
  })
})

// ---------------------------------------------------------------------------
// OpenAgents extensions (pre-existing local behavior, preserved)
// ---------------------------------------------------------------------------

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

  test("the MVP spec is executable with unique author-visible criteria", async () => {
    const markdown = await Bun.file(
      join(repoRoot, "docs", "mvp", "openagents-codex-workroom-mvp.product-spec.md"),
    ).text()
    const result = validateExecutableProductSpec(markdown)
    expect(result.executable).toBe(true)
    expect(result.errors).toEqual([])
    expect(result.criteria).toHaveLength(18)
    expect(result.criteria.map(criterion => criterion.id)).toEqual(
      Array.from({ length: 18 }, (_, index) => `CW-AC-${String(index + 1).padStart(2, "0")}`),
    )
  })

  test("upstream-structured criteria remain standard-valid but cannot execute", async () => {
    // The executable profile requires author-visible prose bullet IDs; a
    // fully upstream-structured document stays viewable but is not
    // executable until the PSEL-2 migration defines the portable path.
    const markdown = await readFixture("conformance/valid/minimal.product-spec.md")
    expect(validateProductSpec(markdown).valid).toBe(true)
    const result = validateExecutableProductSpec(markdown)
    expect(result.executable).toBe(false)
    expect(result.errors.map(error => error.code)).toContain("missing_acceptance_criteria")
  })

  test("prose criteria without ids remain standard-valid but cannot execute", () => {
    const markdown = specWith({}).replace(
      /```productspec-acceptance-criteria[\s\S]*?```/,
      "- A prose criterion without an author-visible ID.",
    )
    expect(validateProductSpec(markdown).valid).toBe(true)
    const result = validateExecutableProductSpec(markdown)
    expect(result.executable).toBe(false)
    expect(result.errors.map(error => error.code)).toContain("missing_acceptance_criterion_id")
  })

  test("duplicate criterion IDs refuse executable admission", async () => {
    const markdown = await Bun.file(
      join(repoRoot, "docs", "mvp", "openagents-codex-workroom-mvp.product-spec.md"),
    ).text()
    const duplicate = markdown.replace("**CW-AC-02:**", "**CW-AC-01:**")
    const result = validateExecutableProductSpec(duplicate)
    expect(result.executable).toBe(false)
    expect(result.errors.map(error => error.code)).toContain("duplicate_acceptance_criterion_id")
  })
})

// ---------------------------------------------------------------------------
// Repo Product Spec roots gate (default profile stays authoritative)
// ---------------------------------------------------------------------------

describe("repo Product Spec roots gate", () => {
  const productSpecRoots = [join(repoRoot, "specs"), join(repoRoot, "docs", "mvp")]
  const specFiles = productSpecRoots.flatMap(listSpecFiles).sort()

  test("configured roots contain at least one Product Spec", () => {
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
