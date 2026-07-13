import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import { openCodexHandoffLedger } from "./codex-handoff-contract.ts"
import { makeCodexHandoffHost, openCodexHandoffBindings } from "./codex-handoff-host.ts"
import type { ProductSpecRun } from "./product-spec-workroom-contract.ts"

const roots: string[] = []
const root = (): string => {
  const value = mkdtempSync(path.join(tmpdir(), "oa-handoff-host-"))
  roots.push(value)
  return value
}
afterEach(() => {
  for (const value of roots.splice(0)) rmSync(value, { recursive: true, force: true })
})

const run = (): ProductSpecRun => ({
  runRef: "run.1",
  workContextRef: "work-context.1",
  spec: { specRef: "spec.1", relativePath: "docs/mvp/test.product-spec.md", revision: 5, digest: `sha256:${"a".repeat(64)}` },
  plan: {
    planRef: "plan.1",
    workContextRef: "work-context.1",
    spec: { specRef: "spec.1", relativePath: "docs/mvp/test.product-spec.md", revision: 5, digest: `sha256:${"a".repeat(64)}` },
    state: "accepted",
    packets: [{
      packetRef: "packet.1", title: "Handoff", criterionIds: ["CW-AC-15"], criterionRefs: ["criterion"],
      dependencyRefs: [], allocation: "root", state: "active", evidenceRefs: [], verifierRefs: [],
      activeLease: { leaseRef: "lease.1", executorRef: "executor.1", executionMode: "owner-present", admittedAt: "2026-07-13T00:00:00Z" },
    }],
    deferredCriterionIds: [], proposedAt: "2026-07-13T00:00:00Z", acceptedAt: "2026-07-13T00:00:00Z",
  },
  createdAt: "2026-07-13T00:00:00Z",
  updatedAt: "2026-07-13T00:00:00Z",
})

describe("trusted Codex handoff host", () => {
  test("binds one accepted ProductSpec packet to one turn and opens honest repository-state fallback after quiescence", async () => {
    const state = root()
    const bindings = openCodexHandoffBindings(path.join(state, "bindings.json"))
    expect(bindings.recordPacketAdmission(run(), "packet.1")).toBe(true)
    expect(bindings.bindNextTurn({ workContextRef: "work-context.1", sessionRef: "session.1", threadRef: "thread.1", turnRef: "turn.1" })).toBe(true)
    const order: string[] = []
    const result = await makeCodexHandoffHost({
      bindings,
      ledger: openCodexHandoffLedger(path.join(state, "handoffs.json")),
      pinnedRuntimeRef: "codex.compat.0.144.1",
      quiesce: async (_request, _binding, operationRef) => { order.push("quiesced"); return { state: "quiescent", proof: { operationRef, workPacketRef: "packet.1", openAgentsGeneration: 1, disposition: "interrupted", lastDurableEventRef: "event.1", proofRef: "proof.1" } } },
      repositoryState: async () => { order.push("post-image"); return { postImageRef: "status.1", transcriptGapRef: "gap.1" } },
      launch: async (_value, handoff) => { order.push(`launch:${handoff.mode}`); return "opened" },
    }).open({ threadRef: "thread.1", turnRef: "turn.1" })
    expect(result).toMatchObject({ state: "opened", workPacketRef: "packet.1", mode: "repository_state", transcriptGap: true })
    expect(order).toEqual(["quiesced", "post-image", "launch:repository_state"])
  })

  test("refuses unbound turns and never invokes quiescence or launch", async () => {
    const state = root()
    let effects = 0
    const host = makeCodexHandoffHost({
      bindings: openCodexHandoffBindings(path.join(state, "bindings.json")),
      ledger: openCodexHandoffLedger(path.join(state, "handoffs.json")),
      pinnedRuntimeRef: "codex.compat.0.144.1",
      quiesce: async () => { effects += 1; return { state: "not_quiescent" } },
      repositoryState: async () => { effects += 1; return null },
      launch: async () => { effects += 1; return "opened" },
    })
    expect(await host.open({ threadRef: "thread.none", turnRef: "turn.none" })).toMatchObject({
      state: "refused", reason: "work_identity_unavailable",
    })
    expect(effects).toBe(0)
  })

  test("restart reconciles the admitted handoff while launch can retry without stopping twice", async () => {
    const state = root()
    const bindings = openCodexHandoffBindings(path.join(state, "bindings.json"))
    bindings.recordPacketAdmission(run(), "packet.1")
    bindings.bindNextTurn({ workContextRef: "work-context.1", sessionRef: "session.1", threadRef: "thread.1", turnRef: "turn.1" })
    let quiesces = 0
    let launches = 0
    const make = () => makeCodexHandoffHost({
      bindings: openCodexHandoffBindings(path.join(state, "bindings.json")),
      ledger: openCodexHandoffLedger(path.join(state, "handoffs.json")),
      pinnedRuntimeRef: "codex.compat.0.144.1",
      quiesce: async (_request, _binding, operationRef) => { quiesces += 1; return { state: "quiescent", proof: { operationRef, workPacketRef: "packet.1", openAgentsGeneration: 1, disposition: "stopped", lastDurableEventRef: "event.1", proofRef: "proof.1" } } },
      repositoryState: async () => ({ postImageRef: "status.1", transcriptGapRef: "gap.1" }),
      launch: async () => { launches += 1; return launches === 1 ? "failed" : "opened" },
    })
    expect(await make().open({ threadRef: "thread.1", turnRef: "turn.1" })).toMatchObject({ state: "refused", reason: "launch_failed" })
    expect(await make().open({ threadRef: "thread.1", turnRef: "turn.1" })).toMatchObject({ state: "opened" })
    expect(quiesces).toBe(1)
    expect(launches).toBe(2)
  })
})
