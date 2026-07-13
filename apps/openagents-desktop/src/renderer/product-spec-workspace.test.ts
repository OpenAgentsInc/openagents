import { describe, expect, test } from "bun:test"
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
    verifierRefs: [],
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
    verifierRefs: [],
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
  })

  test("binds plan and packet transitions to host-confirmed authority", async () => {
    const requests: Array<Readonly<{ op: string; value: unknown }>> = []
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
        currentRun = acceptedRun([{ ...packets[0]!, state: "evidence_present", activeLease: null, evidenceRefs: ["evidence.test"] }, packets[1]!])
        return { ok: true, value: currentRun }
      },
      verifyEvidence: async (value) => {
        requests.push({ op: "verify", value })
        currentRun = acceptedRun([{ ...packets[0]!, state: "verified", activeLease: null, evidenceRefs: ["evidence.test"], verifierRefs: ["verifier.test"] }, packets[1]!])
        return { ok: true, value: currentRun }
      },
    }

    const finalState = await Effect.runPromise(Effect.gen(function* () {
      const state = yield* SubscriptionRef.make(capableState())
      const handlers = makeProductSpecWorkspaceHandlers(state, bridge, () => "uuid.test")
      yield* handlers.ProductSpecOpenRequested()
      yield* handlers.ProductSpecPlanProposed()
      yield* handlers.ProductSpecPlanAccepted()
      yield* handlers.ProductSpecPacketAdmitted("packet.ac-1")
      yield* handlers.ProductSpecEvidenceRefChanged("evidence.test")
      yield* handlers.ProductSpecEvidenceRecorded("packet.ac-1")
      yield* handlers.ProductSpecVerifierRefChanged("verifier.test")
      yield* handlers.ProductSpecEvidenceVerified("packet.ac-1")
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
    expect(requests.find((entry) => entry.op === "evidence")!.value).toMatchObject({ evidenceRef: "evidence.test", leaseRef: "lease.desktop.uuid.test" })
    expect(requests.find((entry) => entry.op === "verify")!.value).toMatchObject({ verifierRef: "verifier.test" })
    expect(finalState.productSpec.run?.plan.packets[0]?.state).toBe("verified")
    expect(finalState.productSpec.run?.plan.packets[1]?.state).toBe("planned")
  })
})
