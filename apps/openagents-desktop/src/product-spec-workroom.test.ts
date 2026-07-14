import { afterEach, describe, expect, test } from "vite-plus/test"
import { existsSync, mkdtempSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { makeProductSpecWorkroom } from "./product-spec-workroom.ts"

const roots: string[] = []
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const validSpec = (revision = 1): string => `---
spec_format_version: "0.1"
title: "Fixture Product"
artifact_type: "prd"
spec_revision: ${revision}
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T00:00:00Z"
---

## Problem

Developers need a durable workroom for agentic product work.

## Hypothesis

If work follows exact criteria then completion becomes inspectable and reliable.

## Scope

\`\`\`productspec-scope
in:
  - durable product work
out:
  - unrelated provider parity
cut:
  - silent completion authority
\`\`\`

## Acceptance Criteria

- **FX-AC-01:** Open one executable ProductSpec with an immutable digest.
- **FX-AC-02:** Accept two dependency-addressed work packets.
- **FX-AC-03:** Keep evidence-present separate from verified completion.

## Success Metrics

\`\`\`productspec-success-metrics
- id: fixture_integrity
  metric: fixture_runs_with_exact_identity
  target: "100%"
  window: every fixture run
  segment: fixture ProductSpecs
  source: fixture receipts
\`\`\`
`

const withPublicEvidenceAttachment = (markdown: string): string => `${markdown.replace(
  'updated_at: "2026-07-13T00:00:00Z"',
  'updated_at: "2026-07-13T12:30:00Z"',
)}
## Related Artifacts

\`\`\`productspec-related-artifacts
- type: github_issue
  url: "https://github.com/OpenAgentsInc/openagents/issues/8805"
  title: "Owner-reviewed evidence attachment path"
\`\`\`
`

const harness = () => {
  const root = mkdtempSync(join(tmpdir(), "oa-product-spec-"))
  roots.push(root)
  const workspaceRoot = join(root, "workspace")
  const stateRoot = join(root, "state")
  mkdirSync(join(workspaceRoot, "specs"), { recursive: true })
  writeFileSync(join(workspaceRoot, "specs", "fixture.product-spec.md"), validSpec())
  const service = makeProductSpecWorkroom({
    workspaceRoot,
    stateRoot,
    now: () => "2026-07-13T12:00:00.000Z",
  })
  return { root, workspaceRoot, stateRoot, service }
}

const openFixture = (service: ReturnType<typeof makeProductSpecWorkroom>) => {
  const result = service.open({
    workContextRef: "work.context.fixture",
    relativePath: "specs/fixture.product-spec.md",
  })
  expect(result.ok).toBe(true)
  if (!result.ok || result.value.state !== "ready") throw new Error("fixture spec did not open")
  return result.value
}

const validPlan = (identity: ReturnType<typeof openFixture>["identity"]) => ({
  workContextRef: "work.context.fixture",
  spec: identity,
  packets: [
    {
      packetRef: "work.packet.authority",
      title: "Implement ProductSpec authority",
      criterionIds: ["FX-AC-01", "FX-AC-03"],
      dependencyRefs: [],
      allocation: "root" as const,
    },
    {
      packetRef: "work.packet.execution",
      title: "Implement accepted packet execution",
      criterionIds: ["FX-AC-02"],
      dependencyRefs: ["work.packet.authority"],
      allocation: "child" as const,
    },
  ],
  deferredCriterionIds: [],
})

describe("ProductSpec workroom authority", () => {
  test("opens an executable spec with exact digest and criterion refs", () => {
    const { service } = harness()
    const projection = openFixture(service)
    expect(projection.identity.digest).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(projection.criteria.map(criterion => criterion.id)).toEqual([
      "FX-AC-01",
      "FX-AC-02",
      "FX-AC-03",
    ])
    expect(projection.criteria[0]?.criterionRef).toContain(
      `@1+${projection.identity.digest}#FX-AC-01`,
    )
  })

  test("creates a viewable starter draft but refuses execution until IDs are authored", () => {
    const { service } = harness()
    const result = service.create({
      workContextRef: "work.context.fixture",
      relativePath: "specs/new.product-spec.md",
      title: "New Product",
    })
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.value.state).toBe("invalid")
    if (result.value.state === "invalid") {
      expect(result.value.standardValid).toBe(true)
      expect(result.value.errors.map(error => error.code)).toContain(
        "missing_acceptance_criterion_id",
      )
    }
  })

  test("persists accepted plans, fences duplicate leases, and separates evidence from verification", () => {
    const { workspaceRoot, stateRoot, service } = harness()
    const projection = openFixture(service)
    const proposed = service.proposePlan(validPlan(projection.identity))
    expect(proposed.ok).toBe(true)
    if (!proposed.ok) return
    const accepted = service.acceptPlan({
      planRef: proposed.value.planRef,
      expectedSpec: projection.identity,
    })
    expect(accepted.ok).toBe(true)
    if (!accepted.ok) return

    const restarted = makeProductSpecWorkroom({
      workspaceRoot,
      stateRoot,
      now: () => "2026-07-13T12:01:00.000Z",
    })
    expect(restarted.run(accepted.value.runRef)).toEqual({ ok: true, value: accepted.value })
    expect(restarted.open({
      workContextRef: "work.context.fixture",
      relativePath: "specs/fixture.product-spec.md",
    })).toMatchObject({ ok: true, value: { state: "ready", activeRunRef: accepted.value.runRef } })

    const blockedByDependency = restarted.admitPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.execution",
      leaseRef: "lease.execution.1",
      executorRef: "agent.child.1",
      executionMode: "afk",
      expectedSpec: projection.identity,
    })
    expect(blockedByDependency).toMatchObject({ ok: false, reason: "dependency_not_verified" })

    const lease = {
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.authority.1",
      executorRef: "agent.root",
      executionMode: "owner-present" as const,
      expectedSpec: projection.identity,
    }
    expect(restarted.admitPacket(lease)).toMatchObject({ ok: true })
    expect(restarted.admitPacket(lease)).toMatchObject({ ok: true, reconciled: true })
    expect(restarted.admitPacket({
      ...lease,
      leaseRef: "lease.authority.conflict",
      executorRef: "agent.other",
    })).toMatchObject({ ok: false, reason: "lease_conflict" })

    expect(restarted.verifyEvidence({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      verifierRef: "verifier.agent.prose",
      outputRef: "verification.output.precondition",
      evidenceReceiptRefs: ["receipt.evidence.missing"],
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "evidence_required" })

    const evidenced = restarted.recordEvidence({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.authority.1",
      evidenceRef: "evidence.tests.productspec",
      evidenceKind: "test_run",
      expectedSpec: projection.identity,
    })
    expect(evidenced).toMatchObject({ ok: true })
    if (!evidenced.ok) return
    expect(evidenced.value.plan.packets[0]?.state).toBe("evidence_present")
    expect(evidenced.value.plan.packets[0]?.evidenceProducerRef).toBe("agent.root")
    const evidenceReceiptRef = evidenced.value.plan.packets[0]?.evidenceReceipts[0]?.receiptRef
    expect(evidenceReceiptRef).toStartWith("receipt.evidence.")
    if (evidenceReceiptRef === undefined) return

    expect(restarted.verifyEvidence({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      verifierRef: "agent.root",
      outputRef: "verification.output.self",
      evidenceReceiptRefs: [evidenceReceiptRef],
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "verifier_required" })

    const verified = restarted.verifyEvidence({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      verifierRef: "verifier.tests.productspec",
      outputRef: "verification.output.tests.productspec",
      evidenceReceiptRefs: [evidenceReceiptRef],
      expectedSpec: projection.identity,
    })
    expect(verified).toMatchObject({ ok: true })
    if (!verified.ok) return
    expect(verified.value.plan.packets[0]?.state).toBe("verified")
    expect(verified.value.plan.packets[0]?.verificationReceipts[0]).toMatchObject({
      evidenceReceiptRefs: [evidenceReceiptRef],
      outputRef: "verification.output.tests.productspec",
      verdict: "passed",
    })
    expect(restarted.setOwnerDisposition({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      disposition: "waived",
      ownerRef: "owner.desktop.local",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "invalid_request" })
    const ownerAccepted = restarted.setOwnerDisposition({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      disposition: "accepted",
      ownerRef: "owner.desktop.local",
      expectedSpec: projection.identity,
    })
    expect(ownerAccepted).toMatchObject({ ok: true })
    if (!ownerAccepted.ok) return
    expect(ownerAccepted.value.plan.packets[0]?.ownerDisposition).toMatchObject({
      disposition: "accepted",
      ownerRef: "owner.desktop.local",
    })
    expect(restarted.setOwnerDisposition({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      disposition: "accepted",
      ownerRef: "owner.desktop.local",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: true, reconciled: true })
    expect(restarted.admitPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.execution",
      leaseRef: "lease.execution.1",
      executorRef: "agent.child.1",
      executionMode: "afk",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: true })
  })

  test("rejects duplicate/cyclic packets and stops dispatch after revision change", () => {
    const { workspaceRoot, service } = harness()
    const projection = openFixture(service)
    expect(service.proposePlan({
      ...validPlan(projection.identity),
      packets: [
        validPlan(projection.identity).packets[0]!,
        {
          ...validPlan(projection.identity).packets[1]!,
          criterionIds: ["FX-AC-01", "FX-AC-03"],
        },
      ],
    })).toMatchObject({ ok: false, reason: "invalid_plan" })

    expect(service.proposePlan({
      ...validPlan(projection.identity),
      packets: [
        { ...validPlan(projection.identity).packets[0]!, dependencyRefs: ["work.packet.execution"] },
        validPlan(projection.identity).packets[1]!,
      ],
    })).toMatchObject({ ok: false, reason: "invalid_plan" })

    const proposed = service.proposePlan(validPlan(projection.identity))
    if (!proposed.ok) throw new Error("plan did not propose")
    const accepted = service.acceptPlan({
      planRef: proposed.value.planRef,
      expectedSpec: projection.identity,
    })
    if (!accepted.ok) throw new Error("plan did not accept")
    writeFileSync(
      join(workspaceRoot, "specs", "fixture.product-spec.md"),
      validSpec(2).replace("immutable digest", "changed immutable digest"),
    )
    const refused = service.admitPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.authority.1",
      executorRef: "agent.root",
      executionMode: "owner-present",
      expectedSpec: projection.identity,
    })
    expect(refused).toMatchObject({ ok: false, reason: "revision_mismatch" })
    expect(service.blockPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.authority.1",
      reason: "stale work",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "revision_mismatch" })
    expect(service.recordEvidence({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.authority.1",
      evidenceRef: "evidence.stale",
      evidenceKind: "receipt",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "revision_mismatch" })
    expect(service.verifyEvidence({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      verifierRef: "verifier.stale",
      outputRef: "verification.output.stale",
      evidenceReceiptRefs: ["receipt.evidence.stale"],
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "revision_mismatch" })
    const persisted = service.run(accepted.value.runRef)
    expect(persisted).toMatchObject({ ok: true })
    if (persisted.ok) expect(persisted.value.plan.state).toBe("revision_mismatch")
  })

  test("persists idempotent failed, cancelled, and superseded packet dispositions", () => {
    const { service } = harness()
    const projection = openFixture(service)
    const proposed = service.proposePlan(validPlan(projection.identity))
    if (!proposed.ok) throw new Error("plan did not propose")
    const accepted = service.acceptPlan({ planRef: proposed.value.planRef, expectedSpec: projection.identity })
    if (!accepted.ok) throw new Error("plan did not accept")
    const request = {
      runRef: accepted.value.runRef,
      packetRef: "work.packet.execution",
      disposition: "cancelled" as const,
      reason: "Owner removed this packet from the accepted scope.",
      expectedSpec: projection.identity,
    }
    expect(service.disposePacket(request)).toMatchObject({ ok: true, value: { plan: { packets: [{}, { state: "cancelled", blockedReason: request.reason }] } } })
    expect(service.disposePacket(request)).toMatchObject({ ok: true, reconciled: true })
    expect(service.disposePacket({ ...request, disposition: "superseded" })).toMatchObject({ ok: false, reason: "invalid_transition" })
    expect(service.admitPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.execution",
      leaseRef: "lease.cancelled",
      executorRef: "agent.child",
      executionMode: "afk",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "invalid_transition" })
  })

  test("stops dispatch on revision mismatch until the old plan is explicitly disposed", () => {
    const { workspaceRoot, service } = harness()
    const projection = openFixture(service)
    const proposed = service.proposePlan(validPlan(projection.identity))
    expect(proposed.ok).toBe(true)
    if (!proposed.ok) return
    const accepted = service.acceptPlan({ planRef: proposed.value.planRef, expectedSpec: projection.identity })
    expect(accepted.ok).toBe(true)
    if (!accepted.ok) return
    expect(service.admitPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.before.revision",
      executorRef: "agent.root",
      executionMode: "owner-present",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: true })

    writeFileSync(join(workspaceRoot, "specs", "fixture.product-spec.md"), validSpec(2).replace("immutable digest", "revised immutable digest"))
    const refreshed = service.run(accepted.value.runRef)
    expect(refreshed).toMatchObject({ ok: true, value: { plan: { state: "revision_mismatch" } } })
    expect(service.open({
      workContextRef: "work.context.fixture",
      relativePath: "specs/fixture.product-spec.md",
    })).toMatchObject({ ok: true, value: { activeRunRef: accepted.value.runRef } })
    expect(service.admitPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.execution",
      leaseRef: "lease.after.revision",
      executorRef: "agent.child",
      executionMode: "afk",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: false, reason: "revision_mismatch" })

    const disposed = service.disposeRun({
      runRef: accepted.value.runRef,
      disposition: "superseded",
      reason: "Revision 2 replaces the accepted intent.",
      expectedSpec: projection.identity,
    })
    expect(disposed).toMatchObject({
      ok: true,
      value: {
        plan: {
          state: "superseded",
          packets: [
            { state: "superseded", activeLease: null, blockedReason: "Revision 2 replaces the accepted intent." },
            { state: "superseded", blockedReason: "Revision 2 replaces the accepted intent." },
          ],
        },
      },
    })
    expect(service.disposeRun({
      runRef: accepted.value.runRef,
      disposition: "superseded",
      reason: "Revision 2 replaces the accepted intent.",
      expectedSpec: projection.identity,
    })).toMatchObject({ ok: true, reconciled: true })
    expect(service.open({
      workContextRef: "work.context.fixture",
      relativePath: "specs/fixture.product-spec.md",
    })).toMatchObject({ ok: true, value: { state: "ready" } })
  })

  test("requires a confirmed revision bump and returns criterion reconciliation", () => {
    const { stateRoot, service } = harness()
    const current = openFixture(service)
    const unchangedRevision = service.proposeEdit({
      workContextRef: "work.context.fixture",
      expectedCurrent: current.identity,
      proposedMarkdown: validSpec().replace("immutable digest", "changed digest"),
    })
    expect(unchangedRevision).toMatchObject({ ok: false, reason: "revision_not_incremented" })

    const proposedMarkdown = validSpec(2)
      .replace("immutable digest", "reviewed immutable digest")
      .replace("- **FX-AC-03:** Keep evidence-present separate from verified completion.\n", "")
      .replace(
        "- **FX-AC-02:** Accept two dependency-addressed work packets.\n",
        "- **FX-AC-02:** Accept two dependency-addressed work packets.\n- **FX-AC-04:** Reconcile intent before redispatch.\n",
      )
    const proposed = service.proposeEdit({
      workContextRef: "work.context.fixture",
      expectedCurrent: current.identity,
      proposedMarkdown,
    })
    expect(proposed).toMatchObject({ ok: true })
    if (!proposed.ok) return
    expect(proposed.value.reconciliation).toEqual({
      retainedCriterionIds: ["FX-AC-01", "FX-AC-02"],
      changedCriterionIds: ["FX-AC-01"],
      addedCriterionIds: ["FX-AC-04"],
      removedCriterionIds: ["FX-AC-03"],
    })
    expect(proposed.value.diff).toContain("--- accepted ProductSpec")
    expect(proposed.value.diff).toContain("+++ proposed ProductSpec")
    expect(proposed.value.diff).toContain("+spec_revision: 2")
    expect(proposed.value.diff).toContain("-spec_revision: 1")
    expect(openFixture(service).identity.revision).toBe(1)

    const confirmed = service.confirmEdit({
      proposalRef: proposed.value.proposalRef,
      expectedCurrent: current.identity,
      criterionDisposition: "supersede_affected_packets",
    })
    expect(confirmed).toMatchObject({ ok: true, value: { reconciled: false } })
    if (!confirmed.ok) return
    expect(confirmed.value.projection.state).toBe("ready")
    if (confirmed.value.projection.state === "ready") {
      expect(confirmed.value.projection.identity).toEqual(proposed.value.next)
    }
    expect(service.confirmEdit({
      proposalRef: proposed.value.proposalRef,
      expectedCurrent: current.identity,
      criterionDisposition: "supersede_affected_packets",
    })).toMatchObject({ ok: true, reconciled: true, value: { reconciled: true } })

    const plan = service.proposePlan({
      workContextRef: "work.context.fixture",
      spec: proposed.value.next,
      packets: [
        { packetRef: "work.packet.revision", title: "Retain revision", criterionIds: ["FX-AC-01", "FX-AC-02"], dependencyRefs: [], allocation: "root" },
        { packetRef: "work.packet.reconcile", title: "Reconcile criteria", criterionIds: ["FX-AC-04"], dependencyRefs: ["work.packet.revision"], allocation: "child" },
      ],
      deferredCriterionIds: [],
    })
    expect(plan.ok).toBe(true)
    if (!plan.ok) return
    expect(service.acceptPlan({ planRef: plan.value.planRef, expectedSpec: proposed.value.next })).toMatchObject({ ok: true })
    const snapshots = readdirSync(`${stateRoot}/snapshots`)
    expect(snapshots).toHaveLength(1)
    expect(existsSync(`${stateRoot}/snapshots/${proposed.value.next.digest.slice("sha256:".length)}.product-spec.md`)).toBe(true)
  })

  test("owner-confirms an exact evidence-only edit while retaining the old run as stale history", () => {
    const { service } = harness()
    const current = openFixture(service)
    const planned = service.proposePlan(validPlan(current.identity))
    if (!planned.ok) throw new Error("plan did not propose")
    const accepted = service.acceptPlan({ planRef: planned.value.planRef, expectedSpec: current.identity })
    if (!accepted.ok) throw new Error("plan did not accept")
    const admitted = service.admitPacket({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.evidence.attachment",
      executorRef: "agent.root",
      executionMode: "owner-present",
      expectedSpec: current.identity,
    })
    if (!admitted.ok) throw new Error("packet did not admit")
    const evidenced = service.recordEvidence({
      runRef: accepted.value.runRef,
      packetRef: "work.packet.authority",
      leaseRef: "lease.evidence.attachment",
      evidenceRef: "evidence.pre.attachment",
      evidenceKind: "test_run",
      expectedSpec: current.identity,
    })
    if (!evidenced.ok) throw new Error("evidence did not record")
    const receiptRef = evidenced.value.plan.packets[0]?.evidenceReceipts[0]?.receiptRef

    const proposal = service.proposeEvidenceAttachment({
      workContextRef: "work.context.fixture",
      expectedCurrent: current.identity,
      proposedMarkdown: withPublicEvidenceAttachment(current.sourceMarkdown),
    })
    expect(proposal).toMatchObject({
      ok: true,
      value: {
        kind: "evidence_attachment_only",
        before: { documentDigest: current.identity.digest, revision: 1 },
        after: { revision: 1 },
        state: "proposed",
      },
    })
    if (!proposal.ok) return
    expect(proposal.value.after.documentDigest).not.toBe(proposal.value.before.documentDigest)
    expect(proposal.value.after.intentDigest).toBe(proposal.value.before.intentDigest)
    expect(openFixture(service).identity).toEqual(current.identity)

    const confirmed = service.confirmEvidenceAttachment({
      proposalRef: proposal.value.proposalRef,
      expectedCurrent: current.identity,
    })
    expect(confirmed).toMatchObject({
      ok: true,
      value: {
        projection: { state: "ready", identity: { revision: 1, digest: proposal.value.after.documentDigest } },
        historicalRun: { runRef: accepted.value.runRef, plan: { state: "revision_mismatch" } },
        reconciled: false,
      },
    })
    if (!confirmed.ok || confirmed.value.historicalRun === null) return
    expect(confirmed.value.historicalRun.plan.packets[0]?.evidenceReceipts[0]?.receiptRef).toBe(receiptRef)
    expect(service.run(accepted.value.runRef)).toMatchObject({
      ok: true,
      value: { plan: { state: "revision_mismatch" } },
    })
    expect(service.confirmEvidenceAttachment({
      proposalRef: proposal.value.proposalRef,
      expectedCurrent: current.identity,
    })).toMatchObject({ ok: true, reconciled: true, value: { reconciled: true } })
  })

  test("evidence attachments fail closed on intent drift and exact-byte races", () => {
    const { workspaceRoot, service } = harness()
    const current = openFixture(service)
    expect(service.proposeEvidenceAttachment({
      workContextRef: "work.context.fixture",
      expectedCurrent: current.identity,
      proposedMarkdown: current.sourceMarkdown.replace("immutable digest", "different intent"),
    })).toMatchObject({ ok: false, reason: "revision_not_incremented" })

    const proposal = service.proposeEvidenceAttachment({
      workContextRef: "work.context.fixture",
      expectedCurrent: current.identity,
      proposedMarkdown: withPublicEvidenceAttachment(current.sourceMarkdown),
    })
    if (!proposal.ok) throw new Error("evidence edit did not propose")
    writeFileSync(join(workspaceRoot, "specs", "fixture.product-spec.md"), `${current.sourceMarkdown}\n`)
    expect(service.confirmEvidenceAttachment({
      proposalRef: proposal.value.proposalRef,
      expectedCurrent: current.identity,
    })).toMatchObject({ ok: false, reason: "proposal_stale" })
  })
})
