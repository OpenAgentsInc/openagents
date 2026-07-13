import { afterEach, describe, expect, test } from "bun:test"
import { mkdtempSync, readFileSync, rmSync, statSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"

import {
  CodexHandoffError,
  openCodexHandoffLedger,
  type CodexHandoffRequest,
  type CodexQuiescenceProof,
} from "./codex-handoff-contract.ts"

const roots: string[] = []
const ledgerPath = (): string => {
  const root = mkdtempSync(path.join(tmpdir(), "openagents-codex-handoff-"))
  roots.push(root)
  return path.join(root, "private", "handoffs.json")
}
afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true })
})

const request = (change: Partial<CodexHandoffRequest> = {}): CodexHandoffRequest => ({
  operationRef: "handoff.operation.1",
  identity: {
    workContextRef: "work-context.1",
    sessionRef: "session.1",
    workPacketRef: "packet.CW-AC-15.1",
    specRef: "spec.openagents-codex-workroom",
    specRevision: 5,
    specDigest: "a".repeat(64),
    criterionRefs: ["CW-AC-15"],
    openAgentsGeneration: 3,
  },
  pinnedRuntimeRef: "codex.compat.0.144.1",
  exactThreadCandidate: {
    providerThreadRef: "codex-thread.1",
    compatibleRuntimeRef: "codex.compat.0.144.1",
    compatibilityProofRef: "proof.compat.1",
    transcriptContinuityProofRef: "proof.transcript.1",
  },
  repositoryState: {
    postImageRef: "post-image.sha256.bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
    transcriptGapRef: "transcript-gap.1",
  },
  ...change,
})

const proof = (change: Partial<CodexQuiescenceProof> = {}): CodexQuiescenceProof => ({
  operationRef: "handoff.operation.1",
  workPacketRef: "packet.CW-AC-15.1",
  openAgentsGeneration: 3,
  disposition: "interrupted",
  lastDurableEventRef: "event.42",
  proofRef: "proof.quiescent.1",
  ...change,
})

describe("Open in Codex handoff admission", () => {
  test("quiesces the exact OpenAgents packet before admitting proven exact-thread continuation", async () => {
    const file = ledgerPath()
    const ledger = openCodexHandoffLedger(file)
    const calls: string[] = []
    const admitted = await ledger.admit(request(), async input => {
      calls.push(`${input.operationRef}:${input.identity.workPacketRef}`)
      // The quiescing state is durable before the external stop/reconcile effect.
      expect(openCodexHandoffLedger(file).get(input.operationRef)?.phase).toBe("quiescing")
      return { state: "quiescent", proof: proof() }
    })

    expect(calls).toEqual(["handoff.operation.1:packet.CW-AC-15.1"])
    expect(admitted).toMatchObject({
      phase: "admitted",
      quiescence: { disposition: "interrupted", proofRef: "proof.quiescent.1" },
      handoff: {
        mode: "exact_thread",
        providerThreadRef: "codex-thread.1",
        compatibilityProofRef: "proof.compat.1",
        transcriptContinuityProofRef: "proof.transcript.1",
      },
    })
    expect(statSync(file).mode & 0o777).toBe(0o600)
    expect(JSON.parse(readFileSync(file, "utf8")).records).toHaveLength(1)
  })

  test("falls back honestly to repository state when pinned runtime continuity is not proven", async () => {
    const ledger = openCodexHandoffLedger(ledgerPath())
    const admitted = await ledger.admit(request({
      exactThreadCandidate: {
        providerThreadRef: "codex-thread.1",
        compatibleRuntimeRef: "codex.compat.other",
        compatibilityProofRef: "proof.compat.other",
        transcriptContinuityProofRef: "proof.transcript.other",
      },
    }), async () => ({ state: "quiescent", proof: proof() }))

    expect(admitted.handoff).toEqual({
      mode: "repository_state",
      postImageRef: request().repositoryState!.postImageRef,
      transcriptGapRef: "transcript-gap.1",
      reason: "exact_thread_continuity_unproven",
    })
    expect(JSON.stringify(admitted.handoff)).not.toContain("codex-thread.1")
  })

  test("refuses to offer Codex while OpenAgents is active or quiescence identifies another packet", async () => {
    const active = openCodexHandoffLedger(ledgerPath())
    expect(await active.admit(request(), async () => ({ state: "not_quiescent" }))).toMatchObject({
      phase: "refused",
      refusal: "openagents_not_quiescent",
      handoff: null,
    })

    const mismatch = openCodexHandoffLedger(ledgerPath())
    expect(await mismatch.admit(request(), async () => ({
      state: "quiescent",
      proof: proof({ workPacketRef: "packet.someone-else" }),
    }))).toMatchObject({
      phase: "refused",
      refusal: "quiescence_identity_mismatch",
      handoff: null,
    })
  })

  test("reconciles an exact retry after restart without quiescing or dispatching twice", async () => {
    const file = ledgerPath()
    let quiesces = 0
    const first = openCodexHandoffLedger(file)
    const admitted = await first.admit(request(), async () => {
      quiesces += 1
      return { state: "quiescent", proof: proof() }
    })

    const afterRestart = openCodexHandoffLedger(file)
    expect(await afterRestart.admit(request(), async () => {
      quiesces += 1
      return { state: "quiescent", proof: proof() }
    })).toEqual(admitted)
    expect(quiesces).toBe(1)
  })

  test("resumes an exact operation left durably quiescing and rejects conflicting reuse", async () => {
    const file = ledgerPath()
    const beforeCrash = openCodexHandoffLedger(file)
    await expect(beforeCrash.admit(request(), async () => {
      throw new Error("host exited after durable admission")
    })).rejects.toThrow("host exited")
    expect(openCodexHandoffLedger(file).get("handoff.operation.1")?.phase).toBe("quiescing")

    const recovered = openCodexHandoffLedger(file)
    expect(await recovered.admit(request(), async () => ({
      state: "quiescent",
      proof: proof(),
    }))).toMatchObject({ phase: "admitted", handoff: { mode: "exact_thread" } })

    await expect(recovered.admit(request({ pinnedRuntimeRef: "codex.compat.changed" }), async () => ({
      state: "quiescent",
      proof: proof(),
    }))).rejects.toMatchObject({ reason: "conflicting_operation" } satisfies Partial<CodexHandoffError>)
  })

  test("does not silently retarget spec, criteria, packet, generation, or runtime identity", async () => {
    const ledger = openCodexHandoffLedger(ledgerPath())
    await ledger.admit(request(), async () => ({ state: "quiescent", proof: proof() }))
    const mutations: ReadonlyArray<CodexHandoffRequest> = [
      request({ identity: { ...request().identity, workPacketRef: "packet.changed" } }),
      request({ identity: { ...request().identity, specRevision: 6 } }),
      request({ identity: { ...request().identity, criterionRefs: ["CW-AC-16"] } }),
      request({ identity: { ...request().identity, openAgentsGeneration: 4 } }),
      request({ pinnedRuntimeRef: "codex.compat.changed" }),
    ]
    for (const changed of mutations) {
      await expect(ledger.admit(changed, async () => ({ state: "quiescent", proof: proof() })))
        .rejects.toMatchObject({ reason: "conflicting_operation" } satisfies Partial<CodexHandoffError>)
    }
  })
})
