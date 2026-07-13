import { mkdtemp, readFile, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { afterEach, describe, expect, test } from "bun:test"

import {
  executePortableSessionControl,
  installPortableCapability,
  portableSessionRoot,
  PortableSessionControlError,
  type PortableSessionGuestRuntime,
} from "../deploy/agent-computer/portable-session-control.js"

const roots: string[] = []
afterEach(async () => Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true }))))

const operation = (action: string, operationRef: string, payload: Record<string, unknown>) => ({
  operationRef,
  action,
  ownerRef: "owner.port03.guest",
  targetRef: "target.port03.guest.managed",
  sessionRef: "session.port03.guest",
  attachmentRef: "attachment.port03.guest.managed",
  generation: 2,
  resourceRef: "resource.port03.guest",
  payload,
})

const graph = {
  rootAgentRef: "agent.port03.guest.root",
  nodes: [
    { agentRef: "agent.port03.guest.root" },
    { agentRef: "agent.port03.guest.child", parentAgentRef: "agent.port03.guest.root" },
  ],
}

const bundle = {
  checkpoint: {
    sessionRef: "session.port03.guest",
    sourceGeneration: 1,
    digest: `sha256:${"d".repeat(64)}`,
    repositoryRevisionRef: "a".repeat(40),
    repositoryPostImageDigest: `sha256:${"a".repeat(64)}`,
    diffDigest: `sha256:${"b".repeat(64)}`,
    graphDigest: `sha256:${"c".repeat(64)}`,
  },
  executionBinding: { runRef: "run.port03.guest" },
  graph,
  threadCursors: [{
    threadRef: "thread.port03.guest.root",
    transcriptRef: "transcript.port03.guest.root",
    activityCursor: 3,
    eventCursor: 8,
  }],
}

const fixture = async () => {
  const stateRoot = await mkdtemp(join(tmpdir(), "portable-session-control-"))
  roots.push(stateRoot)
  const calls: string[] = []
  const runtime: PortableSessionGuestRuntime = {
    verifyStage: async () => { calls.push("stage") },
    verifyCapabilities: async () => { calls.push("capabilities") },
    activate: async () => { calls.push("activate") },
    quiesce: async () => { calls.push("quiesce") },
    reclaim: async () => { calls.push("reclaim") },
    repositorySnapshot: async () => ({
      repositoryRevisionRef: "e".repeat(40),
      repositoryPostImageDigest: `sha256:${"e".repeat(64)}`,
      diffDigest: `sha256:${"f".repeat(64)}`,
    }),
  }
  return { stateRoot, calls, runtime }
}

describe("retained Agent Computer portable-session-control", () => {
  test("keeps stage non-accepting, activates the graph, checkpoints, and reclaims exactly once", async () => {
    const { stateRoot, calls, runtime } = await fixture()
    const stage = operation("stage", "operation.port03.guest.stage", { bundle, capabilityLeaseRefs: ["lease.port03.guest"] })
    const staged = await executePortableSessionControl({ operation: stage, stateRoot, runtime })
    expect(staged).toMatchObject({ acceptingWork: false })
    expect(await executePortableSessionControl({ operation: stage, stateRoot, runtime })).toEqual(staged)
    expect(calls).toEqual(["stage"])

    await expect(executePortableSessionControl({
      operation: operation("activate", "operation.port03.guest.activate.invalid", {
        authorityEvidenceRef: "evidence.port03.guest.authority",
        capabilityLeaseRefs: [],
      }),
      stateRoot,
      runtime,
    })).rejects.toThrow("capability")

    const activated = await executePortableSessionControl({
      operation: operation("activate", "operation.port03.guest.activate", {
        authorityEvidenceRef: "evidence.port03.guest.authority",
        capabilityLeaseRefs: ["lease.port03.guest"],
      }),
      stateRoot,
      runtime,
    })
    expect(activated).toMatchObject({
      activatedAgentRefs: ["agent.port03.guest.root", "agent.port03.guest.child"],
      acceptedWorkRefs: [],
    })
    await executePortableSessionControl({
      operation: operation("quiesce", "operation.port03.guest.quiesce", { graph, threadCursors: bundle.threadCursors }),
      stateRoot,
      runtime,
    })
    const checkpoint = await executePortableSessionControl({
      operation: operation("checkpoint", "operation.port03.guest.checkpoint", {
        checkpointRef: "checkpoint.port03.guest.managed",
        eventLogCursor: 10,
        executionBinding: bundle.executionBinding,
        graph,
        threadCursors: bundle.threadCursors,
      }),
      stateRoot,
      runtime,
    }) as { checkpoint: Record<string, unknown> }
    expect(checkpoint.checkpoint).toMatchObject({
      sourceGeneration: 2,
      repositoryRevisionRef: "e".repeat(40),
      repositoryPostImageDigest: `sha256:${"e".repeat(64)}`,
    })
    expect(checkpoint.checkpoint.digest).toMatch(/^sha256:[a-f0-9]{64}$/)
    const reclaimed = await executePortableSessionControl({
      operation: operation("reclaim", "operation.port03.guest.reclaim", {
        agentRefs: ["agent.port03.guest.root", "agent.port03.guest.child"],
      }),
      stateRoot,
      runtime,
    })
    expect(reclaimed).toMatchObject({ processes: "released", scratch: "released", ports: "released" })
    expect(calls).toEqual(["stage", "capabilities", "activate", "quiesce", "reclaim"])
  })

  test("rejects conflicting replay and private-shaped payloads", async () => {
    const { stateRoot, runtime } = await fixture()
    const stage = operation("stage", "operation.port03.guest.stage", { bundle })
    await executePortableSessionControl({ operation: stage, stateRoot, runtime })
    await expect(executePortableSessionControl({
      operation: { ...stage, payload: { bundle: { ...bundle, checkpoint: { ...bundle.checkpoint, diffDigest: `sha256:${"e".repeat(64)}` } } } },
      stateRoot,
      runtime,
    })).rejects.toThrow("conflict")
    await expect(executePortableSessionControl({
      operation: operation("checkpoint", "operation.port03.guest.unsafe", { password: "not-allowed" }),
      stateRoot,
      runtime,
    })).rejects.toBeInstanceOf(PortableSessionControlError)
  })

  test("abort reclaims a staged graph without activating it", async () => {
    const { stateRoot, calls, runtime } = await fixture()
    await executePortableSessionControl({
      operation: operation("stage", "operation.port03.guest.abort-stage", { bundle }),
      stateRoot,
      runtime,
    })
    const aborted = await executePortableSessionControl({
      operation: operation("abort", "operation.port03.guest.abort", {}),
      stateRoot,
      runtime,
    })
    expect(aborted).toHaveProperty("evidenceRefs")
    expect(calls).toEqual(["stage", "reclaim"])
  })

  test("installs raw capability material behind a refs-only marker and wipes both", async () => {
    const { stateRoot, runtime } = await fixture()
    await executePortableSessionControl({
      operation: operation("stage", "operation.port03.guest.capability-stage", { bundle }),
      stateRoot,
      runtime,
    })
    const material = new TextEncoder().encode("opaque-test-material")
    const metadata = {
      operationRef: "operation.port03.guest.capability-install",
      ownerRef: "owner.port03.guest",
      targetRef: "target.port03.guest.managed",
      resourceRef: "resource.port03.guest",
      sessionRef: "session.port03.guest",
      attachmentRef: "attachment.port03.guest.managed",
      generation: 2,
      leaseRef: "lease.port03.guest.provider",
      evidenceRef: "evidence.port03.guest.provider",
      capability: "capability.provider.codex",
    }
    const installed = await installPortableCapability({ metadata, material, stateRoot }) as Record<string, unknown>
    expect(installed).toMatchObject({
      evidenceRef: metadata.evidenceRef,
      marker: { leaseRef: metadata.leaseRef, evidenceRef: metadata.evidenceRef },
      material: "excluded",
    })
    const sessionRoot = portableSessionRoot(stateRoot, metadata.sessionRef)
    const leaf = new Bun.CryptoHasher("sha256").update(metadata.leaseRef).digest("hex").slice(0, 24)
    expect(await readFile(join(sessionRoot, "capability-material", `${leaf}.material`), "utf8")).toBe("opaque-test-material")

    const wiped = await executePortableSessionControl({
      operation: operation("wipeCapability", "operation.port03.guest.capability-wipe", {
        leaseRef: metadata.leaseRef,
        installationRef: installed.installationRef,
      }),
      stateRoot,
      runtime,
    })
    expect(wiped).toMatchObject({ material: "excluded" })
    expect(await Bun.file(join(sessionRoot, "capability-material", `${leaf}.material`)).exists()).toBe(false)
    expect(await Bun.file(join(sessionRoot, "capabilities", `${leaf}.installed.json`)).exists()).toBe(false)
  })
})
