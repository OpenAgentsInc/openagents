import { describe, expect, test } from "vite-plus/test"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import type {
  ProductSpecIdentity,
  ProductSpecPlan,
  ProductSpecProjection,
  ProductSpecRun,
  ProductSpecWorkPacket,
} from "../product-spec-workroom-contract.ts"
import {
  emptyProductSpecWorkspaceState,
  makeProductSpecWorkspaceHandlers,
  productSpecPacketPrompt,
  productSpecWorkspaceView,
  unavailableProductSpecRendererBridge,
  type ProductSpecRendererBridge,
} from "./product-spec-workspace.ts"

const identity: ProductSpecIdentity = {
  specRef: "spec.openagents.codex.mvp",
  relativePath: "docs/mvp/openagents-codex-mvp.product-spec.md",
  revision: 7,
  digest: `sha256:${"a".repeat(64)}`,
}

const projection: ProductSpecProjection = {
  state: "ready",
  title: "OpenAgents Codex MVP",
  sourceMarkdown: "# OpenAgents Codex MVP\n",
  identity,
  executable: true,
  criteria: [
    { id: "AC-1", criterionRef: "spec.openagents.codex.mvp#AC-1", body: "Open and validate a ProductSpec.", ordinal: 0 },
    { id: "AC-2", criterionRef: "spec.openagents.codex.mvp#AC-2", body: "Verify packet evidence separately.", ordinal: 1 },
  ],
  warnings: [],
}

const packets: ReadonlyArray<ProductSpecWorkPacket> = [
  {
    packetRef: "packet.ac-1",
    title: "Open and validate a ProductSpec.",
    criterionIds: ["AC-1"],
    criterionRefs: ["spec.openagents.codex.mvp#AC-1"],
    dependencyRefs: [],
    allocation: "root",
  state: "planned",
  evidenceRefs: [],
  evidenceReceipts: [],
  verifierRefs: [],
  verificationReceipts: [],
  ownerDisposition: null,
  },
  {
    packetRef: "packet.ac-2",
    title: "Verify packet evidence separately.",
    criterionIds: ["AC-2"],
    criterionRefs: ["spec.openagents.codex.mvp#AC-2"],
    dependencyRefs: ["packet.ac-1"],
    allocation: "child",
  state: "planned",
  evidenceRefs: [],
  evidenceReceipts: [],
  verifierRefs: [],
  verificationReceipts: [],
  ownerDisposition: null,
  },
]

const proposedPlan: ProductSpecPlan = {
  planRef: "plan.openagents.codex.mvp",
  spec: identity,
  workContextRef: "work.context.demo",
  state: "proposed",
  packets,
  deferredCriterionIds: [],
  proposedAt: "2026-07-13T12:00:00.000Z",
}

const acceptedRun = (nextPackets: ReadonlyArray<ProductSpecWorkPacket> = packets): ProductSpecRun => ({
  runRef: "run.openagents.codex.mvp",
  spec: identity,
  workContextRef: "work.context.demo",
  plan: {
    ...proposedPlan,
    state: "accepted",
    packets: nextPackets,
    acceptedAt: "2026-07-13T12:01:00.000Z",
  },
  createdAt: "2026-07-13T12:01:00.000Z",
  updatedAt: "2026-07-13T12:01:00.000Z",
})

const capableState = () => ({
  codingCatalog: {
    authority: "device_local" as const,
    authorityLabel: "This Mac" as const,
    selectedSessionRef: "session.demo",
    focus: { kind: "none" as const },
    pageOffset: 0,
    totalSessions: 1,
    nextOffset: null,
    activeCount: 1,
    recoveryCount: 0,
    archivedCount: 0,
    sessions: [{
      sessionRef: "session.demo",
      workContextRef: "work.context.demo",
      grantRef: "grant.demo",
      projectRef: "project.demo",
      repositoryRef: "repository.demo",
      worktreeRef: "worktree.demo",
      projectLabel: "Demo",
      repositoryLabel: "openagents",
      worktreeLabel: "main",
      state: "active" as const,
      lastActiveAt: "2026-07-13T12:00:00.000Z",
      recoveryReason: null,
    }],
  },
  productSpec: emptyProductSpecWorkspaceState(),
})

const nodeByKey = (value: unknown, key: string): Record<string, unknown> | undefined => {
  if (typeof value !== "object" || value === null) return undefined
  const node = value as Record<string, unknown>
  if (node.key === key) return node
  for (const child of Object.values(node)) {
    if (Array.isArray(child)) {
      for (const entry of child) {
        const found = nodeByKey(entry, key)
        if (found !== undefined) return found
      }
    } else {
      const found = nodeByKey(child, key)
      if (found !== undefined) return found
    }
  }
  return undefined
}

describe("ProductSpec Effect Native workroom", () => {
  test("restores the host-owned accepted run when an executable spec is reopened after renderer reload", async () => {
    const run = acceptedRun()
    const state = await Effect.runPromise(Effect.gen(function* () {
      const state = yield* SubscriptionRef.make(capableState())
      const handlers = makeProductSpecWorkspaceHandlers(state, {
        ...unavailableProductSpecRendererBridge,
        open: async () => ({ ok: true, value: { ...projection, activeRunRef: run.runRef } }),
        run: async (value) => typeof value === "object" && value !== null && "runRef" in value && value.runRef === run.runRef
          ? { ok: true, value: run }
          : { ok: false, reason: "not_found", message: "missing" },
      })
      yield* handlers.ProductSpecOpenRequested()
      return yield* SubscriptionRef.get(state)
    }))
    expect(state.productSpec.run?.runRef).toBe(run.runRef)
    expect(state.productSpec.plan?.state).toBe("accepted")
    expect(state.productSpec.notice).toContain("accepted run was restored")
  })

  test("proposes and confirms a revision only through host-confirmed edit receipts", async () => {
    const requests: Array<{ op: string; value: unknown }> = []
    const nextProjection: ProductSpecProjection = {
      ...projection,
      sourceMarkdown: projection.sourceMarkdown.replace("# OpenAgents", "# Revised OpenAgents"),
      identity: { ...identity, revision: 8, digest: `sha256:${"b".repeat(64)}` },
    }
    const proposal = {
      proposalRef: "product.edit.confirmed",
      workContextRef: "work.context.demo",
      previous: identity,
      next: nextProjection.identity,
      reconciliation: { retainedCriterionIds: ["AC-1", "AC-2"], changedCriterionIds: [], addedCriterionIds: [], removedCriterionIds: [] },
      diff: "--- accepted ProductSpec\n+++ proposed ProductSpec\n@@ -1,1 +1,1 @@\n-# OpenAgents\n+# Revised OpenAgents",
      proposedAt: "2026-07-13T12:00:00.000Z",
      state: "proposed" as const,
    }
    const bridge: ProductSpecRendererBridge = {
      ...unavailableProductSpecRendererBridge,
      proposeEdit: async value => { requests.push({ op: "propose", value }); return { ok: true, value: proposal } },
      confirmEdit: async value => { requests.push({ op: "confirm", value }); return { ok: true, value: { proposal: { ...proposal, state: "confirmed", confirmedAt: "2026-07-13T12:01:00.000Z" }, projection: nextProjection, reconciled: false, criterionDisposition: "supersede_affected_packets" } } },
    }
    const state = await Effect.runPromise(Effect.gen(function* () {
      const ref = yield* SubscriptionRef.make({ ...capableState(), productSpec: { ...emptyProductSpecWorkspaceState(), projection, editDraft: nextProjection.sourceMarkdown } })
      const handlers = makeProductSpecWorkspaceHandlers(ref, bridge)
      yield* handlers.ProductSpecEditProposed()
      yield* handlers.ProductSpecEditConfirmed()
      return yield* SubscriptionRef.get(ref)
    }))
    expect(requests[0]).toEqual({ op: "propose", value: { workContextRef: "work.context.demo", expectedCurrent: identity, proposedMarkdown: nextProjection.sourceMarkdown } })
    expect(requests[1]).toEqual({ op: "confirm", value: { proposalRef: proposal.proposalRef, expectedCurrent: identity, criterionDisposition: "supersede_affected_packets" } })
    expect(state.productSpec.projection).toEqual(nextProjection)
    expect(state.productSpec.editProposal).toBeNull()
    expect(state.productSpec.notice).toContain("new immutable identity")
  })

  test("renders an exact revision diff and explicit confirmation control", () => {
    const view = productSpecWorkspaceView({
      ...emptyProductSpecWorkspaceState(),
      projection,
      editDraft: projection.sourceMarkdown.replace("# OpenAgents", "# Revised OpenAgents"),
      editProposal: {
        proposalRef: "product.edit.demo",
        workContextRef: "work.context.demo",
        previous: identity,
        next: { ...identity, revision: 8, digest: `sha256:${"b".repeat(64)}` },
        reconciliation: {
          retainedCriterionIds: ["AC-1"],
          changedCriterionIds: ["AC-1"],
          addedCriterionIds: ["AC-3"],
          removedCriterionIds: ["AC-2"],
        },
        diff: "--- accepted ProductSpec\n+++ proposed ProductSpec\n@@ -1,1 +1,1 @@\n-# OpenAgents\n+# Revised OpenAgents",
        proposedAt: "2026-07-13T12:00:00.000Z",
        state: "proposed",
      },
    }, "work.context.demo")
    expect(nodeByKey(view, "product-spec-edit-diff")?.content).toContain("+# Revised OpenAgents")
    expect(nodeByKey(view, "product-spec-edit-reconciliation")?.content).toContain("Removed: AC-2")
    expect((nodeByKey(view, "product-spec-edit-confirm")?.onPress as { name?: string } | undefined)?.name).toBe("ProductSpecEditConfirmed")
  })

  test("projects validation identity, digest, criteria, and dependency-gated packets", () => {
    const run = acceptedRun()
    const view = productSpecWorkspaceView({
      ...emptyProductSpecWorkspaceState(),
      projection,
      plan: run.plan,
      run,
    }, "work.context.demo")

    expect(nodeByKey(view, "product-spec-effect-native")?.label).toBe("Effect Native")
    expect(nodeByKey(view, "product-spec-revision")?.content).toBe("Revision 7")
    expect(nodeByKey(view, "product-spec-digest")?.content).toBe(identity.digest)
    expect(nodeByKey(view, "product-spec-criterion-AC-1")).toBeDefined()
    expect(nodeByKey(view, "product-spec-admit-packet.ac-1")?.disabled).toBe(false)
    expect(nodeByKey(view, "product-spec-admit-packet.ac-2")?.disabled).toBe(true)
    expect(nodeByKey(view, "product-spec-admit-packet.ac-2")?.label).toBe("Waiting for dependencies")
    expect(nodeByKey(view, "product-spec-cancel-packet.ac-1")?.disabled).toBe(true)
  })

  test("renders and executes explicit old-plan reconciliation after a revision mismatch", async () => {
    const mismatchRun: ProductSpecRun = {
      ...acceptedRun([{ ...packets[0]!, state: "active", activeLease: { leaseRef: "lease.old", executorRef: "agent.root", executionMode: "owner-present", admittedAt: "2026-07-13T12:02:00.000Z" } }, packets[1]!]),
      plan: {
        ...acceptedRun().plan,
        state: "revision_mismatch",
        packets: [{ ...packets[0]!, state: "active", activeLease: { leaseRef: "lease.old", executorRef: "agent.root", executionMode: "owner-present", admittedAt: "2026-07-13T12:02:00.000Z" } }, packets[1]!],
      },
    }
    const disposedRun: ProductSpecRun = {
      ...mismatchRun,
      plan: {
        ...mismatchRun.plan,
        state: "superseded",
        packets: mismatchRun.plan.packets.map(packet => ({ ...packet, state: "superseded" as const, activeLease: null, blockedReason: "Intent changed" })),
      },
    }
    const requests: unknown[] = []
    const state = await Effect.runPromise(Effect.gen(function* () {
      const ref = yield* SubscriptionRef.make({
        ...capableState(),
        productSpec: { ...emptyProductSpecWorkspaceState(), projection, editDraft: projection.sourceMarkdown, run: mismatchRun, plan: mismatchRun.plan, blockedReason: "Intent changed" },
      })
      const handlers = makeProductSpecWorkspaceHandlers(ref, {
        ...unavailableProductSpecRendererBridge,
        disposeRun: async value => { requests.push(value); return { ok: true, value: disposedRun } },
      })
      yield* handlers.ProductSpecRunDispositionSelected("superseded")
      return yield* SubscriptionRef.get(ref)
    }))
    expect(requests).toEqual([{
      runRef: mismatchRun.runRef,
      disposition: "superseded",
      reason: "Intent changed",
      expectedSpec: identity,
    }])
    expect(state.productSpec.run?.plan.state).toBe("superseded")
    expect(state.productSpec.notice).toContain("editing is unlocked")

    const view = productSpecWorkspaceView({
      ...emptyProductSpecWorkspaceState(),
      projection,
      editDraft: projection.sourceMarkdown,
      run: mismatchRun,
      plan: mismatchRun.plan,
      blockedReason: "Intent changed",
    }, "work.context.demo")
    expect(nodeByKey(view, "product-spec-edit-run-lock")?.content).toContain("dispatch is stopped")
    expect((nodeByKey(view, "product-spec-run-supersede")?.onPress as { name?: string } | undefined)?.name).toBe("ProductSpecRunDispositionSelected")
    expect(nodeByKey(view, "product-spec-edit-draft")?.disabled).toBe(true)
  })

  test("binds plan and packet transitions to host-confirmed authority", async () => {
    const requests: Array<Readonly<{ op: string; value: unknown }>> = []
    const dispatched: Array<{ run: ProductSpecRun; packet: ProductSpecWorkPacket }> = []
    let currentRun = acceptedRun()
    const bridge: ProductSpecRendererBridge = {
      ...unavailableProductSpecRendererBridge,
      open: async (value) => { requests.push({ op: "open", value }); return { ok: true, value: projection } },
      proposePlan: async (value) => { requests.push({ op: "propose", value }); return { ok: true, value: proposedPlan } },
      acceptPlan: async (value) => { requests.push({ op: "accept", value }); return { ok: true, value: currentRun } },
      admitPacket: async (value) => {
        requests.push({ op: "admit", value })
        currentRun = acceptedRun([{ ...packets[0]!, state: "active", activeLease: { leaseRef: "lease.desktop.uuid.test", executorRef: "executor.desktop.owner", executionMode: "owner-present", admittedAt: "2026-07-13T12:02:00.000Z" } }, packets[1]!])
        return { ok: true, value: currentRun }
      },
      recordEvidence: async (value) => {
        requests.push({ op: "evidence", value })
        currentRun = acceptedRun([{ ...packets[0]!, state: "evidence_present", activeLease: null, evidenceRefs: ["evidence.test"], evidenceReceipts: [{ receiptRef: "receipt.evidence.test", evidenceRef: "evidence.test", kind: "receipt", producerRef: "executor.desktop.owner", spec: identity, criterionIds: ["AC-1"], producedAt: "2026-07-13T12:03:00.000Z" }] }, packets[1]!])
        return { ok: true, value: currentRun }
      },
      verifyEvidence: async (value) => {
        requests.push({ op: "verify", value })
        currentRun = acceptedRun([{ ...packets[0]!, state: "verified", activeLease: null, evidenceRefs: ["evidence.test"], verifierRefs: ["verifier.test"], verificationReceipts: [{ receiptRef: "receipt.verification.test", evidenceReceiptRefs: ["receipt.evidence.test"], outputRef: "verification.output.test", verifierRef: "verifier.test", spec: identity, criterionIds: ["AC-1"], verdict: "passed", verifiedAt: "2026-07-13T12:04:00.000Z" }] }, packets[1]!])
        return { ok: true, value: currentRun }
      },
      setOwnerDisposition: async (value) => {
        requests.push({ op: "owner", value })
        currentRun = acceptedRun([{ ...currentRun.plan.packets[0]!, ownerDisposition: { disposition: "accepted", ownerRef: "owner.desktop.local", decidedAt: "2026-07-13T12:05:00.000Z" } }, packets[1]!])
        return { ok: true, value: currentRun }
      },
    }

    const finalState = await Effect.runPromise(Effect.gen(function* () {
      const state = yield* SubscriptionRef.make(capableState())
      const handlers = makeProductSpecWorkspaceHandlers(
        state,
        bridge,
        () => "uuid.test",
        async (run, packet) => { dispatched.push({ run, packet }) },
      )
      yield* handlers.ProductSpecOpenRequested()
      yield* handlers.ProductSpecPlanProposed()
      yield* handlers.ProductSpecPlanAccepted()
      yield* handlers.ProductSpecPacketAdmitted("packet.ac-1")
      yield* handlers.ProductSpecEvidenceRefChanged("evidence.test")
      yield* handlers.ProductSpecEvidenceRecorded("packet.ac-1")
      yield* handlers.ProductSpecVerifierRefChanged("verifier.test")
      yield* handlers.ProductSpecVerificationOutputRefChanged("verification.output.test")
      yield* handlers.ProductSpecEvidenceVerified("packet.ac-1")
      yield* handlers.ProductSpecOwnerDispositionSelected({ packetRef: "packet.ac-1", disposition: "accepted" })
      return yield* SubscriptionRef.get(state)
    }))

    expect((requests[0]!.value as { workContextRef: string }).workContextRef).toBe("work.context.demo")
    const planRequest = requests.find((entry) => entry.op === "propose")!.value as { packets: ReadonlyArray<{ dependencyRefs: ReadonlyArray<string> }> }
    expect(planRequest.packets.map((packet) => packet.dependencyRefs)).toEqual([[], ["packet.ac-1"]])
    expect(requests.find((entry) => entry.op === "accept")!.value).toEqual({ planRef: proposedPlan.planRef, expectedSpec: identity })
    expect(requests.find((entry) => entry.op === "admit")!.value).toMatchObject({
      runRef: currentRun.runRef,
      packetRef: "packet.ac-1",
      leaseRef: "lease.desktop.uuid.test",
      expectedSpec: identity,
    })
    expect(requests.find((entry) => entry.op === "evidence")!.value).toMatchObject({ evidenceRef: "evidence.test", evidenceKind: "receipt", leaseRef: "lease.desktop.uuid.test", expectedSpec: identity })
    expect(requests.find((entry) => entry.op === "verify")!.value).toMatchObject({ verifierRef: "verifier.test", outputRef: "verification.output.test", evidenceReceiptRefs: ["receipt.evidence.test"], expectedSpec: identity })
    expect(requests.find((entry) => entry.op === "owner")!.value).toMatchObject({ disposition: "accepted", ownerRef: "owner.desktop.local", expectedSpec: identity })
    expect(finalState.productSpec.run?.plan.packets[0]?.ownerDisposition?.disposition).toBe("accepted")
    expect(dispatched).toHaveLength(1)
    expect(dispatched[0]?.packet).toMatchObject({
      packetRef: "packet.ac-1",
      state: "active",
      activeLease: { leaseRef: "lease.desktop.uuid.test" },
    })
    const prompt = productSpecPacketPrompt(dispatched[0]!.run, dispatched[0]!.packet)
    for (const line of [
      `Spec revision: ${identity.revision}`,
      `Spec digest: ${identity.digest}`,
      "Packet: packet.ac-1",
      "Lease: lease.desktop.uuid.test",
      "Allocation: root",
      "Acceptance criteria: AC-1",
      "Execute it in the current Codex turn.",
    ]) expect(prompt).toContain(line)
    const childPrompt = productSpecPacketPrompt(dispatched[0]!.run, {
      ...packets[1]!,
      state: "active",
      activeLease: {
        leaseRef: "lease.desktop.child.test",
        executorRef: "executor.desktop.child",
        executionMode: "owner-present",
        admittedAt: "2026-07-13T12:02:00.000Z",
      },
    })
    expect(childPrompt).toContain("Allocation: child")
    expect(childPrompt).toContain("Delegate its implementation through the native Codex child-agent tool")
    expect(finalState.productSpec.run?.plan.packets[0]?.state).toBe("verified")
    expect(finalState.productSpec.run?.plan.packets[1]?.state).toBe("planned")
  })
})
