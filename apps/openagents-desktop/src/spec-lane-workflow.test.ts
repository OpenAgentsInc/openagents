import { mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { mkdtempSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fileURLToPath } from "node:url"

import {
  ASSURANCE_EVIDENCE_INDEX_FORMAT_VERSION,
  decodeAssuranceEvidenceIndex,
  parseAssuranceSpec,
  proposeAssuranceSpec,
  serializeAssuranceSpec,
  sha256Digest,
} from "@openagentsinc/assurance-spec"
import { describe, expect, test } from "vite-plus/test"

import {
  SPEC_LANE_MAX_PROMPT_CHARS,
  appendSpecLaneContext,
  projectSpecLaneTurn,
  specLaneRevalidationNote,
} from "./spec-lane-workflow.ts"

const fixture = () => {
  const root = mkdtempSync(join(tmpdir(), "spec-lane-"))
  const specs = join(root, "specs")
  mkdirSync(specs, { recursive: true })
  const productPath = join(specs, "lane.product-spec.md")
  const product = readFileSync(fileURLToPath(new URL(
    "../../../docs/mvp/openagents-codex-workroom-mvp.product-spec.md",
    import.meta.url,
  )), "utf8")
  writeFileSync(productPath, product)
  const proposal = proposeAssuranceSpec({
    productSpecPath: "specs/lane.product-spec.md",
    productSpecMarkdown: product,
    assuranceSpecId: "assurance.lane-proof",
  })
  if (!proposal.ok) throw new Error("fixture proposal failed")
  const assurance = serializeAssuranceSpec(proposal.document)
  const assurancePath = join(specs, "lane.assurance-spec.md")
  writeFileSync(assurancePath, assurance)
  return { root, product, assurance, assurancePath, document: parseAssuranceSpec(assurance) }
}

const qualifyingEvidence = ({
  assurance,
  document,
}: Pick<ReturnType<typeof fixture>, "assurance" | "document">) => ({
  assurance_evidence_index_format_version: ASSURANCE_EVIDENCE_INDEX_FORMAT_VERSION,
  subject: {
    product_spec_digest: document.subject.product_spec.document_digest,
    assurance_spec_digest: sha256Digest(assurance),
    manifest_digest: `sha256:${"1".repeat(64)}`,
    admission_digest: `sha256:${"2".repeat(64)}`,
  },
  gate: {
    gate_ref: "gate.lane",
    admitted: true,
    executable: true,
    confirmed_obligations: 1,
    total_obligations: document.obligations.length,
    infrastructure: "ready",
    stability: "stable",
    freshness: "current",
    disposition: "accepted",
    exception: "none",
    full_desktop_gate: "green",
  },
  receipts: document.obligations.map(item => ({
    obligation_id: item.id,
    criterion_refs: item.criterion_refs,
    candidate: { ref: `candidate.${item.id}`, digest: `sha256:${"3".repeat(64)}`, path: "proof/candidate.json" },
    falsifier: { ref: `falsifier.${item.id}`, digest: `sha256:${"4".repeat(64)}`, path: "proof/falsifier.json" },
    sensitivity: { ref: `sensitivity.${item.id}`, digest: `sha256:${"5".repeat(64)}`, path: "proof/sensitivity.json" },
    axes: {
      admission: "admitted",
      readiness: "executable",
      observation: "CONFIRMED",
      infrastructure: "ready",
      stability: "stable",
      freshness: "current",
      disposition: "accepted",
      exception: "none",
    },
  })),
  companion_evidence_refs: [],
  public_safety: { classification: "reviewed_public_safe", raw_artifacts_public: false },
} as const)

describe("lane-independent spec workflow", () => {
  test("projects ProductSpec and unmet obligations under a hard prompt bound", () => {
    const { root } = fixture()
    const projection = projectSpecLaneTurn(root)
    expect(projection.snapshot.productSpecs).toHaveLength(1)
    expect(projection.snapshot.assuranceSpecs).toHaveLength(1)
    expect(projection.snapshot.obligations.every(item => item.state === "unmet")).toBe(true)
    expect(projection.promptContext).toContain("UNMET")
    expect(projection.promptContext.length).toBeLessThanOrEqual(SPEC_LANE_MAX_PROMPT_CHARS)
    expect(projection.promptContext).toMatch(/not permission to alter acceptance, admission, verification, release, or public claims\.$/)
    expect(appendSpecLaneContext("Do one thing", projection)).toContain("OWNER TURN INSTRUCTION")
  })

  test("revalidates a failing obligation into a bounded owner-visible note on two lane refs", () => {
    const { root, assurance, document } = fixture()
    const before = projectSpecLaneTurn(root).snapshot
    const evidence = qualifyingEvidence({ assurance, document })
    expect(() => decodeAssuranceEvidenceIndex(evidence)).not.toThrow()
    writeFileSync(join(root, "specs", "lane.assurance-evidence-index.json"), `${JSON.stringify(evidence)}\n`)
    const after = projectSpecLaneTurn(root).snapshot

    for (const laneRef of ["codex-local", "fable-local"]) {
      const note = specLaneRevalidationNote(laneRef, before, after)
      expect(note).toContain(`Spec revalidation · ${laneRef}`)
      expect(note).toContain("→ confirmed")
      expect(note).toContain("does not admit, verify, release")
    }
  })

  test("fails evidence closed when its index is malformed", () => {
    const { root } = fixture()
    writeFileSync(join(root, "specs", "bad.assurance-evidence-index.json"), "{}\n")
    const projection = projectSpecLaneTurn(root)
    expect(projection.snapshot.obligations.every(item => item.state === "unmet")).toBe(true)
    expect(projection.snapshot.diagnostics).toContain("specs/bad.assurance-evidence-index.json: evidence index is not schema-valid")
  })

  test("fails stale ProductSpec-bound evidence closed instead of rounding obligations green", () => {
    const { root, assurance, document } = fixture()
    const evidence = qualifyingEvidence({ assurance, document })
    const staleEvidence = {
      ...evidence,
      subject: {
        ...evidence.subject,
        product_spec_digest: `sha256:${"9".repeat(64)}`,
      },
    }
    expect(() => decodeAssuranceEvidenceIndex(staleEvidence)).not.toThrow()
    writeFileSync(join(root, "specs", "lane.assurance-evidence-index.json"), `${JSON.stringify(staleEvidence)}\n`)

    const projection = projectSpecLaneTurn(root)

    expect(projection.snapshot.obligations.every(item => item.state === "unmet")).toBe(true)
    expect(projection.snapshot.diagnostics).toContain(
      "specs/lane.assurance-evidence-index.json: evidence index does not bind the exact ProductSpec digest",
    )
    expect(projection.promptContext).toContain("UNMET")
  })
})
