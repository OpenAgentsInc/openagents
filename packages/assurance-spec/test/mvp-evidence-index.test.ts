import { describe, expect, test } from "vite-plus/test"
import { readFileSync } from "node:fs"
import { resolve } from "node:path"

import {
  checkCompletionClaim,
  decodeAssuranceEvidenceIndex,
  getCoverageLedgers,
  runTool,
  sha256Digest,
  type ToolOutcome,
} from "../src/index.ts"

const root = resolve(import.meta.dirname, "../../..")
const specPath = "docs/mvp/openagents-codex-workroom-mvp.assurance-spec.md"
const indexPath = "assurance/openagents-desktop-mvp.evidence-index.json"
const read = (path: string): string => readFileSync(resolve(root, path), "utf8")
const ok = <A>(value: ToolOutcome<A>): A => {
  if (!value.ok) throw new Error(`${value.code}: ${value.message}`)
  return value.value
}

describe("full MVP Assurance Evidence Index", () => {
  test("binds exact admitted bytes and keeps every gate axis independent", () => {
    const index = decodeAssuranceEvidenceIndex(JSON.parse(read(indexPath)))
    expect(index.subject.assurance_spec_digest).toBe(sha256Digest(read(specPath)))
    expect(index.receipts).toHaveLength(18)
    expect(index.gate).toEqual({
      gate_ref: "GATE-MVP-FULL-ASSURANCE",
      admitted: true,
      executable: true,
      confirmed_obligations: 18,
      total_obligations: 18,
      infrastructure: "ready",
      stability: "stable",
      freshness: "current",
      disposition: "accepted",
      exception: "none",
      full_desktop_gate: "green",
    })
  })

  test("drives all three ledgers without a blended score", () => {
    const ledgers = ok(runTool(getCoverageLedgers({ root, path: specPath, evidence_index_path: indexPath })))
    expect(ledgers.criterion_traceability).toMatchObject({ total_criteria: 18, traceable_criteria: 18 })
    expect(ledgers.execution).toMatchObject({
      total_obligations: 18,
      executed_obligations: 18,
      receipt_source: "assurance_evidence_index",
    })
    expect(ledgers.execution.entries.every((entry) => entry.observation === "CONFIRMED")).toBe(true)
    expect(ledgers.reachable_frontier).toMatchObject({ status: "computed" })
    expect(JSON.stringify(ledgers)).not.toMatch(/"[^"]*(?:percent|percentage|score)[^"]*"\s*:/i)
  })

  test("projects 18 accepted eight-axis rows while claim text remains non-authoritative", () => {
    const audit = ok(runTool(checkCompletionClaim({
      root,
      path: specPath,
      evidence_index_path: indexPath,
      claim: "everything is done",
    })))
    expect(audit.claim_evaluated).toBe(false)
    expect(audit.obligations).toHaveLength(18)
    for (const obligation of audit.obligations) {
      expect(obligation.unresolved_fields).toEqual([])
      expect(obligation.axes).toEqual({
        admission: "admitted",
        readiness: "executable",
        observation: "CONFIRMED",
        infrastructure: "ready",
        stability: "stable",
        freshness: "current",
        disposition: "accepted",
        exception: "none",
      })
    }
  })

  test("committed projections contain no raw native output or machine identity", () => {
    const indexBytes = read(indexPath)
    const index = decodeAssuranceEvidenceIndex(JSON.parse(indexBytes))
    for (const row of index.receipts) {
      for (const pointer of [row.candidate, row.falsifier, row.sensitivity]) {
        const bytes = read(pointer.path)
        expect(sha256Digest(bytes)).toBe(pointer.digest)
        expect(bytes).not.toContain("hostname")
        expect(bytes).not.toContain(root)
        expect(bytes).not.toMatch(/Christophers-|BEGIN (?:RSA |EC )?PRIVATE KEY|sk-[A-Za-z0-9]/)
      }
    }
    const fullGate = JSON.parse(read("assurance/openagents-desktop-mvp.full-desktop-gate-receipt.json"))
    expect(fullGate).toMatchObject({ verdict: "green", typecheck: "passed", build: "passed", electron_smoke: "passed" })
    expect(JSON.stringify(fullGate)).not.toContain(root)
  })
})
