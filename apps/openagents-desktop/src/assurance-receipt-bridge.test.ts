import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { canonicalArtifact, sha256Digest, type AssuranceReceipt } from "@openagentsinc/assurance-spec"

import { bridgeAssuranceReceipt } from "./assurance-receipt-bridge.ts"
import { makeProductSpecWorkroom } from "./product-spec-workroom.ts"

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const spec = `---
spec_format_version: "0.1"
title: "Bridge Fixture"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T00:00:00Z"
---

## Problem

Receipt links need an authority-preserving bridge.

## Hypothesis

If exact receipts are resolved before mutation, non-confirming results cannot become verified.

## Scope

\`\`\`productspec-scope
in:
  - typed receipt bridge
out:
  - release authority
cut:
  - self verification
\`\`\`

## Acceptance Criteria

- **FX-AC-01:** Register one independently reviewed exact Assurance Receipt.
- **FX-AC-02:** Keep owner disposition separate.

## Success Metrics

\`\`\`productspec-success-metrics
- id: bridge_integrity
  metric: exact_receipts_only
  target: "100%"
  window: every bridge call
  segment: local fixture
  source: bridge tests
\`\`\`
`

const harness = () => {
  const root = mkdtempSync(join(tmpdir(), "oa-assurance-bridge-"))
  roots.push(root)
  const workspaceRoot = join(root, "workspace")
  const stateRoot = join(root, "state")
  mkdirSync(join(workspaceRoot, "specs"), { recursive: true })
  writeFileSync(join(workspaceRoot, "specs", "fixture.product-spec.md"), spec)
  const workroom = makeProductSpecWorkroom({ workspaceRoot, stateRoot, now: () => "2026-07-13T12:00:00.000Z" })
  const opened = workroom.open({ workContextRef: "work.context.bridge", relativePath: "specs/fixture.product-spec.md" })
  if (!opened.ok || opened.value.state !== "ready") throw new Error("fixture open failed")
  const proposed = workroom.proposePlan({
    workContextRef: "work.context.bridge",
    spec: opened.value.identity,
    packets: [
      { packetRef: "packet.evidence", title: "Evidence", criterionIds: ["FX-AC-01"], dependencyRefs: [], allocation: "root" },
      { packetRef: "packet.owner", title: "Owner boundary", criterionIds: ["FX-AC-02"], dependencyRefs: [], allocation: "child" },
    ],
    deferredCriterionIds: [],
  })
  if (!proposed.ok) throw new Error(proposed.message)
  const accepted = workroom.acceptPlan({ planRef: proposed.value.planRef, expectedSpec: opened.value.identity })
  if (!accepted.ok) throw new Error(accepted.message)
  const admitted = workroom.admitPacket({
    runRef: accepted.value.runRef,
    packetRef: "packet.evidence",
    leaseRef: "lease.evidence.1",
    executorRef: "runner.local.1",
    executionMode: "owner-present",
    expectedSpec: opened.value.identity,
  })
  if (!admitted.ok) throw new Error(admitted.message)
  return { root, workroom, identity: opened.value.identity, runRef: accepted.value.runRef }
}

const receipt = (productSpecDigest: string, adapterRef = "openagents.bun_test.v1"): AssuranceReceipt => ({
  assurance_receipt_format_version: "0.1",
  receipt_ref: "receipt.bridge.fixture",
  manifest_digest: sha256Digest("manifest"),
  product_spec_digest: productSpecDigest,
  assurance_spec_digest: sha256Digest("assurance"),
  admission_digest: sha256Digest("admission"),
  obligation_id: "AO-FX-AC-01-01",
  criterion_refs: ["FX-AC-01"],
  environment_ref: "ENV-OA-LOCAL-BUN-1",
  adapter_ref: adapterRef,
  execution_unit_ref: "unit.fx.candidate",
  producer_ref: "runner.local.1",
  reviewer_ref: "reviewer.independent.1",
  native_report_ref: "candidate.junit.xml",
  native_report_digest: sha256Digest("junit"),
  command_digest: sha256Digest("command"),
  source_digest: sha256Digest("source"),
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
  public_safety: { classification: "private", contains_raw_output: false },
})

const invoke = (state: ReturnType<typeof harness>, value: AssuranceReceipt) => {
  const artifact = canonicalArtifact(value)
  return bridgeAssuranceReceipt({
    workroom: state.workroom,
    resolverRoot: join(state.root, "resolver"),
    receiptBytes: artifact.bytes,
    expectedReceiptDigest: artifact.digest,
    expectedManifestDigest: sha256Digest("manifest"),
    expectedAssuranceSpecDigest: sha256Digest("assurance"),
    expectedAdmissionDigest: sha256Digest("admission"),
    expectedObligationId: "AO-FX-AC-01-01",
    expectedSpec: state.identity,
    expectedCriterionIds: ["FX-AC-01"],
    authorizedReviewerRefs: ["reviewer.independent.1"],
    runRef: state.runRef,
    packetRef: "packet.evidence",
    leaseRef: "lease.evidence.1",
    verifierRef: "reviewer.independent.1",
  })
}

describe("openagents.assurance_receipt_bridge.v1", () => {
  test("registers exact qualifying bytes and leaves owner disposition pending", () => {
    const state = harness()
    const result = invoke(state, receipt(state.identity.digest))
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.handle).toMatch(/^assurance\.receipt\.[a-f0-9]{64}$/)
    expect(readFileSync(result.resolverPath, "utf8")).toContain('"observation": "CONFIRMED"')
    const packet = result.run.plan.packets.find((candidate) => candidate.packetRef === "packet.evidence")
    expect(packet).toMatchObject({
      state: "verified",
      evidenceRefs: [result.handle],
      evidenceProducerRef: "runner.local.1",
      verifierRefs: ["reviewer.independent.1"],
      ownerDisposition: null,
    })
  })

  test("mutation receipts use the unchanged normalized bridge and gain no special authority", () => {
    const state = harness()
    const result = invoke(state, receipt(state.identity.digest, "openagents.mutation.v1"))
    expect(result.ok).toBe(true)
    if (!result.ok) return
    const packet = result.run.plan.packets.find((candidate) => candidate.packetRef === "packet.evidence")
    expect(packet).toMatchObject({
      state: "verified",
      ownerDisposition: null,
    })
  })

  test("refuses every nonqualifying axis before host mutation", () => {
    const mutations = [
      { observation: "REFUTED" },
      { observation: "INCONCLUSIVE" },
      { freshness: "stale" },
      { stability: "flaky" },
      { infrastructure: "unavailable" },
      { disposition: "pending_review" },
      { exception: "scoped" },
    ] as const
    for (const mutation of mutations) {
      const state = harness()
      const base = receipt(state.identity.digest)
      const result = invoke(state, { ...base, axes: { ...base.axes, ...mutation } })
      expect(result).toMatchObject({ ok: false, code: "receipt_not_qualifying" })
      const run = state.workroom.run(state.runRef)
      expect(run.ok).toBe(true)
      if (run.ok) expect(run.value.plan.packets[0]?.state).toBe("active")
    }
  })

  test("refuses subject mismatch and self review", () => {
    const mismatch = harness()
    expect(invoke(mismatch, receipt(`sha256:${"0".repeat(64)}`))).toMatchObject({ ok: false, code: "binding_mismatch" })
    const self = harness()
    expect(invoke(self, { ...receipt(self.identity.digest), reviewer_ref: "runner.local.1" })).toMatchObject({
      ok: false,
      code: "reviewer_not_independent",
    })
  })
})
