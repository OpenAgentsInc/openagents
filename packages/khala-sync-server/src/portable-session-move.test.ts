import { SQL } from "@openagentsinc/postgres-runtime"
import { afterAll, beforeAll, beforeEach, describe, expect, test } from "vite-plus/test"

import {
  PortableCapabilityBroker,
  makeOpenAgentsManagedCapabilityAdapter,
  makeOwnerLocalCapabilityAdapter,
  type PortableAgentGraph,
  type PortableCheckpoint,
  type CapabilityBrokerConfig,
  type CapabilityBrokerPrivateDurableState,
  type CapabilityBrokerStateStore,
  type SecretMaterial,
} from "@openagentsinc/portable-session-contract"
import { Effect } from "effect"

import { runMigrations } from "./migrate.js"
import { withSyncTransaction } from "./outbox-writer.js"
import {
  appendPortableSessionEvent,
  quiescePortableSessionGraph,
  readPortableSessionAuthoritySnapshot,
  registerPortableSession,
  requestPortableSessionCommand,
} from "./portable-session-authority.js"
import {
  computePortableCheckpointDigest,
  PortableSessionMoveCoordinator,
  type PortableCheckpointBundle,
  type PortableSessionExecutionTarget,
  type PortableSessionMoveInput,
  type PortableTargetActivationReceipt,
} from "./portable-session-move.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js"
const owner = "owner.port03"
const sessionRef = "session.port03"
const localTargetRef = "target.port03.local"
const managedTargetRef = "target.port03.agent-computer"
const sourceAttachmentRef = "attachment.port03.local.1"
const digest = (character: string) => `sha256:${character.repeat(64)}` as const
const NOW = new Date("2026-07-13T06:00:00.000Z")
const EXPIRES = "2026-07-13T06:10:00.000Z"

const session = {
  schema: "openagents.portable_session.v1" as const,
  sessionRef,
  ownerRef: owner,
  identityBasis: "owner_minted" as const,
  workContextRef: "work.port03.repository",
  eventLogRef: "eventlog.port03",
  currentProjectionRef: "current.port03",
  volatileStreamRef: "stream.port03",
  commandScopeRef: "commands.port03",
  graph: {
    rootAgentRef: "agent.port03.root",
    nodes: [
      {
        agentRef: "agent.port03.root",
        threadRef: "thread.port03.root",
        transcriptRef: "transcript.port03.root",
        activityCursor: 0,
        lifecycle: "running" as const,
        attachmentGeneration: 1,
      },
      {
        agentRef: "agent.port03.child",
        parentAgentRef: "agent.port03.root",
        threadRef: "thread.port03.child",
        transcriptRef: "transcript.port03.child",
        activityCursor: 0,
        lifecycle: "running" as const,
        attachmentGeneration: 1,
      },
    ],
  },
  adoptedFromLocalHistory: false,
}

const executionBinding = {
  schema: "openagents.portable_session_execution_binding.v1" as const,
  sessionRef,
  ownerRef: owner,
  runRef: "run.port03.canonical",
  repositoryRef: "repository.OpenAgentsInc.openagents",
  pinnedBaseRef: "revision.port03.pinned",
}

const targets = [
  {
    targetRef: localTargetRef,
    targetClass: "owner_local" as const,
    adapterRef: "adapter.port03.local",
    ownerRef: owner,
    compatibilityRef: "compat.port03.v1",
    isolation: "owner_host_process" as const,
    dataPosture: "owner_device_only" as const,
    health: "ready" as const,
  },
  {
    targetRef: managedTargetRef,
    targetClass: "openagents_managed" as const,
    adapterRef: "adapter.port03.agent-computer",
    ownerRef: owner,
    compatibilityRef: "compat.port03.v1",
    isolation: "dedicated_microvm" as const,
    dataPosture: "openagents_managed_region" as const,
    health: "ready" as const,
  },
]

const sourceAttachment = {
  attachmentRef: sourceAttachmentRef,
  sessionRef,
  targetRef: localTargetRef,
  generation: 1,
  state: "active" as const,
  descendantAgentRefs: session.graph.nodes.map(node => node.agentRef),
  capabilityLeaseRefs: ["lease.port03.local.provider.1", "lease.port03.local.scm.1"],
  evidenceRefs: ["evidence.port03.local.started"],
}

type BoundaryLog = {
  installed: Array<{ leaseRef: string; targetRef: string; materialBytes: number }>
  wiped: string[]
  revoked: string[]
  targetOperations: string[]
  sequence: string[]
}

class MemoryBrokerStateStore implements CapabilityBrokerStateStore {
  state: CapabilityBrokerPrivateDurableState | null = null
  load = async () => this.state === null ? null : structuredClone(this.state)
  save = async (state: CapabilityBrokerPrivateDurableState) => {
    this.state = structuredClone(state)
  }
}

const makeBroker = async (
  log: BoundaryLog,
  options: { failFirstReissueEvidence?: boolean } = {},
): Promise<Readonly<{ broker: PortableCapabilityBroker; config: CapabilityBrokerConfig }>> => {
  const material = new TextEncoder().encode("PORT03-CANARY-RAW-CREDENTIAL") as SecretMaterial
  const adapterRuntime = {
    install: async (input: { lease: { leaseRef: string; targetRef: string }; material: SecretMaterial }) => {
      log.installed.push({
        leaseRef: input.lease.leaseRef,
        targetRef: input.lease.targetRef,
        materialBytes: input.material.byteLength,
      })
      log.sequence.push(`install:${input.lease.targetRef}:${input.lease.leaseRef}`)
      return { installationRef: `installation.${input.lease.leaseRef}` }
    },
    wipe: async (input: { leaseRef: string }) => {
      log.wiped.push(input.leaseRef)
      return { wipeReceiptRef: `receipt.${input.leaseRef}.wiped` }
    },
  }
  let reissueEvidenceFailed = false
  const stateStore = new MemoryBrokerStateStore()
  const config: CapabilityBrokerConfig = {
    clock: { now: () => new Date(NOW) },
    maxTtlMs: 15 * 60 * 1_000,
    vault: {
      withSourceGrantMaterial: async ({ use }) => use(material),
      revokeSourceGrant: async ({ sourceGrantRef }) => {
        log.revoked.push(sourceGrantRef)
        log.sequence.push(`revoke:${sourceGrantRef}`)
      },
    },
    targets: [
      { targetRef: localTargetRef, targetClass: "owner_local", adapterRef: "adapter.port03.local", ready: true },
      { targetRef: managedTargetRef, targetClass: "openagents_managed", adapterRef: "adapter.port03.agent-computer", ready: true },
    ],
    adapters: [
      makeOwnerLocalCapabilityAdapter("adapter.port03.local", adapterRuntime),
      makeOpenAgentsManagedCapabilityAdapter("adapter.port03.agent-computer", adapterRuntime),
    ],
    evidenceSink: { append: async evidence => {
      if (options.failFirstReissueEvidence &&
          evidence.operation === "reissue" && !reissueEvidenceFailed) {
        reissueEvidenceFailed = true
        throw new Error("durable evidence sink unavailable")
      }
    } },
    stateStore,
  }
  const broker = new PortableCapabilityBroker(config)
  for (const [index, capability] of (["provider", "scm_write"] as const).entries()) {
    const leaseRef = sourceAttachment.capabilityLeaseRefs[index]!
    await Effect.runPromise(broker.issue({
      operationRef: `operation.port03.issue.${index}`,
      leaseRef,
      ownerRef: owner,
      sessionRef,
      attachmentRef: sourceAttachmentRef,
      attachmentGeneration: 1,
      targetRef: localTargetRef,
      capability,
      sourceGrantRef: `grant.port03.local.${index}`,
      accountRef: `account.port03.${index}`,
      permissions: capability === "provider" ? ["provider.execute"] : ["scm.write.repository"],
      expiresAt: EXPIRES,
    }))
    await Effect.runPromise(broker.redeem({
      operationRef: `operation.port03.redeem.${index}`,
      leaseRef,
    }))
  }
  return { broker, config }
}

type TargetFaults = {
  rejectStage?: boolean
  failCleanup?: boolean
  failActivationOnce?: boolean
  tamperCheckpoint?: "digest" | "graph" | "cursor" | "binding" | "secret_extra"
}

const makeTarget = (
  targetRef: string,
  targetClass: "owner_local" | "openagents_managed",
  log: BoundaryLog,
  faults: TargetFaults = {},
): PortableSessionExecutionTarget => {
  const checkpoints = new Map<string, PortableCheckpointBundle>()
  const activations = new Map<string, PortableTargetActivationReceipt>()
  const activationFingerprints = new Map<string, string>()
  let activationFailed = false
  return {
    targetRef,
    targetClass,
    quiesceGraph: async input => {
      log.targetOperations.push(input.operationRef)
      return {
        quiescedAgentRefs: input.graph.nodes.map(node => node.agentRef),
        evidenceRefs: [`evidence.${input.operationRef}`],
      }
    },
    createCheckpoint: async input => {
      log.targetOperations.push(input.operationRef)
      const existing = checkpoints.get(input.operationRef)
      if (existing) return existing
      const graph: PortableAgentGraph = {
        ...input.graph,
        nodes: input.graph.nodes.map(node => ({ ...node })),
      }
      const withoutDigest = {
        schema: "openagents.portable_checkpoint.v1" as const,
        checkpointRef: input.checkpointRef,
        sessionRef: input.sessionRef,
        sourceAttachmentRef: input.attachmentRef,
        sourceGeneration: input.generation,
        repositoryRef: input.executionBinding.repositoryRef,
        repositoryRevisionRef: `revision.port03.generation.${input.generation}`,
        repositoryPostImageDigest: digest(input.generation % 2 === 0 ? "e" : "b"),
        diffDigest: digest(input.generation % 2 === 0 ? "f" : "c"),
        eventLogCursor: faults.tamperCheckpoint === "cursor" ? input.eventLogCursor - 1 : input.eventLogCursor,
        catalogGenerationRef: "catalog.port03.v1",
        graphDigest: faults.tamperCheckpoint === "graph"
          ? digest("d")
          : (await import("./portable-session-authority.js")).computePortableAgentGraphDigest(graph),
        approvalRefs: ["approval.port03.repository.write"],
        artifactRefs: ["artifact.port03.checkpoint"],
        receiptRefs: ["receipt.port03.checkpoint.sealed"],
        secretMaterial: "excluded" as const,
        processState: "excluded" as const,
      }
      const checkpoint: PortableCheckpoint = {
        ...withoutDigest,
        digest: faults.tamperCheckpoint === "digest"
          ? digest("a")
          : computePortableCheckpointDigest(withoutDigest),
      }
      const bundle: PortableCheckpointBundle & { password?: string } = {
        checkpoint,
        executionBinding: faults.tamperCheckpoint === "binding"
          ? { ...input.executionBinding, runRef: "run.port03.tampered" }
          : input.executionBinding,
        graph,
        threadCursors: input.threadCursors,
        ...(faults.tamperCheckpoint === "secret_extra"
          ? { password: "PORT03-CANARY-RAW-CREDENTIAL" }
          : {}),
      }
      checkpoints.set(input.operationRef, bundle)
      return bundle
    },
    cleanupSource: async input => {
      log.targetOperations.push(input.operationRef)
      return {
        cleanedAgentRefs: faults.failCleanup ? input.agentRefs.slice(0, 1) : input.agentRefs,
        processes: "released",
        scratch: "released",
        ports: "released",
        evidenceRefs: [`evidence.${input.operationRef}`],
      }
    },
    stageCheckpoint: async input => {
      log.targetOperations.push(input.operationRef)
      log.sequence.push(`stage:${targetRef}:${input.operationRef}`)
      if (faults.rejectStage) throw new Error("destination rejected checkpoint")
      return {
        checkpointDigest: input.bundle.checkpoint.digest,
        repositoryPostImageDigest: input.bundle.checkpoint.repositoryPostImageDigest,
        diffDigest: input.bundle.checkpoint.diffDigest,
        graphDigest: input.bundle.checkpoint.graphDigest,
        threadCursors: input.bundle.threadCursors,
        acceptingWork: false,
        evidenceRefs: [`evidence.${input.operationRef}`],
      }
    },
    activate: async input => {
      log.targetOperations.push(input.operationRef)
      const fingerprint = JSON.stringify(input)
      const priorFingerprint = activationFingerprints.get(input.operationRef)
      if (priorFingerprint !== undefined && priorFingerprint !== fingerprint) {
        throw new Error("activation operation ref replayed with different bytes")
      }
      const existing = activations.get(input.operationRef)
      if (existing) return existing
      const receipt = {
        schema: "openagents.ide_portable_destination_activation.v1" as const,
        receiptRef: `receipt.${input.operationRef}`,
        operationRef: input.operationRef,
        sessionRef: input.sessionRef,
        checkpointRef: input.checkpointRef,
        destinationTargetRef: targetRef,
        destinationAttachmentRef: input.destinationAttachmentRef,
        destinationGeneration: input.destinationGeneration,
        authentication: {
          state: "reauthenticated" as const,
          policyRef: `policy.portable.destination.${targetClass}.v1`,
          evidenceRef: `evidence.authentication.${input.operationRef}`,
          observedAt: "2026-07-13T06:00:00.000Z",
          expiresAt: null,
        },
        helpers: (["pty", "lsp", "dap", "watcher", "native"] as const).map(kind => ({
          kind, readiness: "unsupported" as const, instanceRef: null, versionRef: null,
          omissionRef: `omission.${targetClass}.${kind}`, evidenceRefs: [],
        })),
        activatedAgentRefs: session.graph.nodes.map(node => node.agentRef),
        acceptedWorkRefs: session.graph.nodes.map(node => ({
          agentRef: node.agentRef,
          turnRef: `turn.${input.destinationGeneration}.${node.agentRef}`,
        })),
        evidenceRefs: [`evidence.${input.operationRef}`],
      }
      activationFingerprints.set(input.operationRef, fingerprint)
      activations.set(input.operationRef, receipt)
      if (faults.failActivationOnce && !activationFailed) {
        activationFailed = true
        throw new Error("activation acknowledgement lost")
      }
      return receipt
    },
    abortStaged: async input => {
      log.targetOperations.push(input.operationRef)
      return { evidenceRefs: [`evidence.${input.operationRef}`] }
    },
  }
}

const command = (
  kind: "move" | "failback",
  sourceRef: string,
  generation: number,
  destinationTargetRef: string,
  checkpointRef: string,
) => ({
  schema: "openagents.portable_session_command.v1" as const,
  commandRef: `command.port03.${kind}.${generation}`,
  idempotencyKey: `idempotency.port03.${kind}.${generation}`,
  ownerRef: owner,
  sessionRef,
  kind,
  expectedAttachmentRef: sourceRef,
  expectedGeneration: generation,
  destinationTargetRef,
  checkpointRef,
  expiresAt: "2099-01-01T00:00:00.000Z",
})

const transfers = (sourceRefs: readonly string[], generation: number, destination: "managed" | "local") =>
  sourceRefs.map((sourceLeaseRef, index) => ({
    sourceLeaseRef,
    destinationLeaseRef: `lease.port03.${destination}.${generation + 1}.${index}`,
    destinationSourceGrantRef: `grant.port03.${destination}.${generation + 1}.${index}`,
    expiresAt: EXPIRES,
  }))

describe.skipIf(!hasLocalPostgres())("PORT-03 graph-wide portable move coordinator", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_portable_move")
    await admin.end()
    await runMigrations({ databaseUrl: pg.urlFor("khala_sync_portable_move") })
    sql = SQL({ url: pg.urlFor("khala_sync_portable_move"), max: 10 })
  })

  beforeEach(async () => {
    await sql`TRUNCATE khala_sync_portable_sessions, khala_sync_portable_targets, khala_sync_changelog, khala_sync_scopes RESTART IDENTITY CASCADE`
    await withSyncTransaction(sql as unknown as SyncSql, writer =>
      registerPortableSession(writer, { session, executionBinding, targets, attachment: sourceAttachment }, owner, "mutation.port03.register"))
    await withSyncTransaction(sql as unknown as SyncSql, async writer => {
      await appendPortableSessionEvent(writer, {
        eventRef: "event.port03.root.1",
        sessionRef,
        threadRef: "thread.port03.root",
        threadCursor: 1,
        attachmentRef: sourceAttachmentRef,
        attachmentGeneration: 1,
        eventKind: "activity_cursor",
        current: { lifecycle: "running", activityCursor: 0 },
      }, "mutation.port03.root.1")
      await appendPortableSessionEvent(writer, {
        eventRef: "event.port03.child.1",
        sessionRef,
        threadRef: "thread.port03.child",
        threadCursor: 1,
        attachmentRef: sourceAttachmentRef,
        attachmentGeneration: 1,
        eventKind: "activity_cursor",
        current: { lifecycle: "running", activityCursor: 0 },
      }, "mutation.port03.child.1")
    })
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  const setup = async (faults: {
    local?: TargetFaults
    managed?: TargetFaults
    loseCompletionAck?: boolean
    failFirstReissueEvidence?: boolean
  } = {}) => {
    const log: BoundaryLog = { installed: [], wiped: [], revoked: [], targetOperations: [], sequence: [] }
    const brokerHarness = await makeBroker(log, {
      ...(faults.failFirstReissueEvidence ? { failFirstReissueEvidence: true } : {}),
    })
    const { broker } = brokerHarness
    const local = makeTarget(localTargetRef, "owner_local", log, faults.local)
    const managed = makeTarget(managedTargetRef, "openagents_managed", log, faults.managed)
    let transactionCount = 0
    const coordinator = new PortableSessionMoveCoordinator({
      sql: sql as unknown as SyncSql,
      transaction: async run => {
        transactionCount += 1
        const result = await withSyncTransaction(sql as unknown as SyncSql, run)
        if (faults.loseCompletionAck && transactionCount === 3) {
          throw new Error("committed completion acknowledgement lost")
        }
        return result
      },
      broker,
    })
    return { broker, brokerConfig: brokerHarness.config, coordinator, local, log, managed }
  }

  const firstMove = (local: PortableSessionExecutionTarget, managed: PortableSessionExecutionTarget): PortableSessionMoveInput => ({
    command: command("move", sourceAttachmentRef, 1, managedTargetRef, "checkpoint.port03.local.1"),
    destinationAttachmentRef: "attachment.port03.managed.2",
    capabilityTransfers: transfers(sourceAttachment.capabilityLeaseRefs, 1, "managed"),
    source: local,
    destination: managed,
  })

  test("moves the exact child-bearing graph local → Agent Computer → local with one attachment and no duplicate work", async () => {
    const { broker, coordinator, local, log, managed } = await setup()
    const moveInput = firstMove(local, managed)
    const moved = await coordinator.move(moveInput)
    expect(moved.status).toBe("completed")
    expect(moved).toMatchObject({
      runRef: executionBinding.runRef,
      repositoryRef: executionBinding.repositoryRef,
      pinnedBaseRef: executionBinding.pinnedBaseRef,
    })
    expect(moved.acceptedWorkRefs).toHaveLength(2)
    expect(new Set(moved.acceptedWorkRefs.map(item => item.turnRef)).size).toBe(2)

    let snapshot = await readPortableSessionAuthoritySnapshot(sql as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    expect(snapshot?.session.current_attachment_ref).toBe("attachment.port03.managed.2")
    expect(snapshot?.executionBinding).toMatchObject({
      run_ref: executionBinding.runRef,
      repository_ref: executionBinding.repositoryRef,
      pinned_base_ref: executionBinding.pinnedBaseRef,
    })
    expect(Number(snapshot?.session.current_attachment_generation)).toBe(2)
    expect(snapshot?.attachments.filter(row => row.state === "active")).toHaveLength(1)
    expect(snapshot?.attachments.map(row => row.state)).toEqual(["detached", "active"])
    expect(snapshot?.agents.map(row => [row.agent_ref, row.parent_agent_ref, row.thread_ref, row.transcript_ref])).toEqual([
      ["agent.port03.child", "agent.port03.root", "thread.port03.child", "transcript.port03.child"],
      ["agent.port03.root", null, "thread.port03.root", "transcript.port03.root"],
    ])
    expect(snapshot?.current.map(row => [row.thread_ref, Number(row.latest_cursor)])).toEqual([
      ["thread.port03.child", 1],
      ["thread.port03.root", 1],
    ])
    expect(snapshot?.checkpoints).toHaveLength(1)
    expect(snapshot?.checkpoints[0]?.repository_post_image_digest).toBe(digest("b"))
    expect(snapshot?.checkpoints[0]?.diff_digest).toBe(digest("c"))
    expect(log.wiped).toEqual(expect.arrayContaining(sourceAttachment.capabilityLeaseRefs))
    expect(log.revoked).toEqual(expect.arrayContaining(["grant.port03.local.0", "grant.port03.local.1"]))
    const managedStageIndex = log.sequence.findIndex(item => item.startsWith(`stage:${managedTargetRef}:`))
    const firstRevokeIndex = log.sequence.findIndex(item => item.startsWith("revoke:"))
    const firstManagedInstallIndex = log.sequence.findIndex(item => item.startsWith(`install:${managedTargetRef}:`))
    expect(managedStageIndex).toBeGreaterThanOrEqual(0)
    expect(firstRevokeIndex).toBeGreaterThan(managedStageIndex)
    expect(firstManagedInstallIndex).toBeGreaterThan(firstRevokeIndex)

    await expect(withSyncTransaction(sql as unknown as SyncSql, writer =>
      appendPortableSessionEvent(writer, {
        eventRef: "event.port03.source.late",
        sessionRef,
        threadRef: "thread.port03.root",
        threadCursor: 2,
        attachmentRef: sourceAttachmentRef,
        attachmentGeneration: 1,
        eventKind: "activity_cursor",
        current: { lifecycle: "running", activityCursor: 2 },
      }, "mutation.port03.source.late"))).rejects.toMatchObject({ code: "stale_generation" })

    const replayed = await coordinator.move(moveInput)
    expect(replayed.status).toBe("replayed")
    expect(replayed.acceptedWorkRefs).toEqual(moved.acceptedWorkRefs)
    expect(broker.snapshot().leases.filter(row => row.lease.state === "redeemed" && row.lease.targetRef === managedTargetRef)).toHaveLength(2)

    const managedLeaseRefs = moved.capabilityLeaseRefs
    const failbackInput: PortableSessionMoveInput = {
      command: command("failback", "attachment.port03.managed.2", 2, localTargetRef, "checkpoint.port03.managed.2"),
      destinationAttachmentRef: "attachment.port03.local.3",
      capabilityTransfers: transfers(managedLeaseRefs, 2, "local"),
      source: managed,
      destination: local,
    }
    const failedBack = await coordinator.move(failbackInput)
    expect(failedBack.status).toBe("completed")
    expect(failedBack.runRef).toBe(executionBinding.runRef)
    expect(failedBack.acceptedWorkRefs).toHaveLength(2)
    snapshot = await readPortableSessionAuthoritySnapshot(sql as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    expect(snapshot?.session.current_attachment_ref).toBe("attachment.port03.local.3")
    expect(Number(snapshot?.session.current_attachment_generation)).toBe(3)
    expect(snapshot?.attachments.filter(row => row.state === "active")).toHaveLength(1)
    expect(snapshot?.attachments.map(row => row.state)).toEqual(["detached", "detached", "active"])
    expect(snapshot?.agents.every(row => Number(row.attachment_generation) === 3)).toBe(true)
    expect(snapshot?.checkpoints).toHaveLength(2)
    expect(snapshot?.commands.map(row => row.status)).toEqual(["completed", "completed"])

    const serialized = JSON.stringify({ moved, failedBack, snapshot, broker: broker.snapshot() })
    expect(serialized).not.toContain("PORT03-CANARY-RAW-CREDENTIAL")
    expect(serialized).not.toContain("destinationSourceGrantRef")
  })

  test("destination rejection leaves a durable failed outcome, fenced source, released destination leases, and no accepted work", async () => {
    const { broker, coordinator, local, log, managed } = await setup({ managed: { rejectStage: true } })
    const result = await coordinator.move(firstMove(local, managed))
    expect(result).toMatchObject({ status: "failed", reasonRef: "reason.portable_move.destination_rejected" })
    expect(result.acceptedWorkRefs).toEqual([])
    const snapshot = await readPortableSessionAuthoritySnapshot(sql as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    expect(snapshot?.session.state).toBe("recovery_required")
    expect(snapshot?.session.current_attachment_ref).toBe(sourceAttachmentRef)
    expect(snapshot?.attachments.map(row => row.state)).toEqual(["quiesced"])
    expect(snapshot?.commands).toHaveLength(1)
    expect(snapshot?.commands[0]?.status).toBe("failed")
    expect(broker.snapshot().leases.filter(row => row.lease.targetRef === managedTargetRef)).toEqual([])
    expect(log.revoked).toEqual([])
    expect(log.installed.filter(item => item.targetRef === managedTargetRef)).toEqual([])
    expect(log.targetOperations).toContain("operation.command.port03.move.1.destination.abort")
  })

  test.each(["digest", "graph", "cursor", "binding", "secret_extra"] as const)("rejects a tampered %s checkpoint before broker transfer or destination stage", async tamperCheckpoint => {
    const { broker, coordinator, local, managed } = await setup({ local: { tamperCheckpoint } })
    const result = await coordinator.move(firstMove(local, managed))
    expect(result).toMatchObject({ status: "failed", reasonRef: "reason.portable_move.checkpoint_invalid" })
    expect(broker.snapshot().leases.filter(row => row.lease.targetRef === managedTargetRef)).toHaveLength(0)
    const snapshot = await readPortableSessionAuthoritySnapshot(sql as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    expect(snapshot?.session.state).toBe("recovery_required")
    expect(snapshot?.checkpoints).toHaveLength(0)
  })

  test("refuses movement for a legacy session row without a canonical execution binding", async () => {
    await sql`DELETE FROM khala_sync_portable_session_execution_bindings WHERE session_ref = ${sessionRef}`
    const { coordinator, local, managed, log } = await setup()
    await expect(coordinator.move(firstMove(local, managed))).rejects.toMatchObject({
      reason: "authority_rejected",
    })
    expect(log.targetOperations).toEqual([])
  })

  test("source cleanup failure cannot advance authority or leave destination grants live", async () => {
    const { broker, coordinator, local, managed } = await setup({ local: { failCleanup: true } })
    const result = await coordinator.move(firstMove(local, managed))
    expect(result).toMatchObject({ status: "failed", reasonRef: "reason.portable_move.source_cleanup_failed" })
    const snapshot = await readPortableSessionAuthoritySnapshot(sql as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    expect(snapshot?.session.current_attachment_ref).toBe(sourceAttachmentRef)
    expect(snapshot?.attachments.map(row => row.state)).toEqual(["quiesced"])
    expect(broker.snapshot().leases.filter(row => row.lease.targetRef === managedTargetRef).every(row => row.lease.state === "released")).toBe(true)
  })

  test("a reissue evidence failure releases the requested destination lease instead of orphaning authority", async () => {
    const { broker, coordinator, local, log, managed } = await setup({ failFirstReissueEvidence: true })
    const result = await coordinator.move(firstMove(local, managed))
    expect(result).toMatchObject({ status: "failed", reasonRef: "reason.portable_move.broker_failed" })
    const destination = broker.snapshot().leases.find(row =>
      row.lease.leaseRef === "lease.port03.managed.2.0")
    expect(destination?.lease.state).toBe("released")
    expect(broker.snapshot().leases.filter(row =>
      row.lease.targetRef === managedTargetRef && ["issued", "redeemed"].includes(row.lease.state))).toHaveLength(0)
    expect(log.targetOperations).toContain("operation.command.port03.move.1.destination.abort")
  })

  test("lost activation acknowledgement converges through a completed-command replay without another accepted parent or child turn", async () => {
    const { coordinator, local, managed } = await setup({ managed: { failActivationOnce: true } })
    const input = firstMove(local, managed)
    const first = await coordinator.move(input)
    expect(first.status).toBe("activation_pending_reconcile")
    expect(first.acceptedWorkRefs).toEqual([])
    const replay = await coordinator.move(input)
    expect(replay.status).toBe("replayed")
    expect(replay.acceptedWorkRefs).toHaveLength(2)
    expect(new Set(replay.acceptedWorkRefs.map(item => `${item.agentRef}:${item.turnRef}`)).size).toBe(2)
    const duplicate = await coordinator.move(input)
    expect(duplicate.acceptedWorkRefs).toEqual(replay.acceptedWorkRefs)
    const snapshot = await readPortableSessionAuthoritySnapshot(sql as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    expect(snapshot?.commands).toHaveLength(1)
    expect(snapshot?.checkpoints).toHaveLength(1)
    expect(snapshot?.attachments.filter(row => row.state === "active")).toHaveLength(1)
  })

  test("a lost committed PORT-01 completion acknowledgement reconciles without aborting or releasing the authoritative destination", async () => {
    const { broker, coordinator, local, log, managed } = await setup({ loseCompletionAck: true })
    const result = await coordinator.move(firstMove(local, managed))
    expect(result.status).toBe("replayed")
    expect(result.acceptedWorkRefs).toHaveLength(2)
    expect(log.targetOperations).not.toContain("operation.command.port03.move.1.destination.abort")
    expect(broker.snapshot().leases.filter(row =>
      row.lease.targetRef === managedTargetRef && row.lease.state === "redeemed")).toHaveLength(2)
    const snapshot = await readPortableSessionAuthoritySnapshot(
      sql as unknown as SyncSql,
      { sessionRef, ownerUserId: owner },
    )
    expect(snapshot?.commands[0]?.status).toBe("completed")
    expect(snapshot?.session.current_attachment_ref).toBe("attachment.port03.managed.2")
  })

  test("refuses a runtime target whose class does not match the durable session target", async () => {
    const { coordinator, local, log } = await setup()
    const masqueradingManaged = makeTarget(managedTargetRef, "owner_local" as never, log)
    await expect(coordinator.move(firstMove(local, masqueradingManaged))).rejects.toMatchObject({
      reason: "target_mismatch",
    })
    expect(log.targetOperations).toEqual([])
  })

  test("a fresh coordinator and SQL handle resume an accepted quiesced move after one capability already reissued", async () => {
    const { broker, brokerConfig, local, log, managed } = await setup()
    const input = firstMove(local, managed)
    await withSyncTransaction(sql as unknown as SyncSql, writer => requestPortableSessionCommand(
      writer,
      input.command,
      owner,
      "mutation.port03.interrupted.accept",
    ))
    const quiesced = await local.quiesceGraph({
      operationRef: `operation.${input.command.commandRef}.source.quiesce`,
      sessionRef,
      attachmentRef: sourceAttachmentRef,
      generation: 1,
      graph: session.graph,
      threadCursors: [
        { threadRef: "thread.port03.child", transcriptRef: "transcript.port03.child", activityCursor: 0, eventCursor: 1 },
        { threadRef: "thread.port03.root", transcriptRef: "transcript.port03.root", activityCursor: 0, eventCursor: 1 },
      ],
    })
    await withSyncTransaction(sql as unknown as SyncSql, writer => quiescePortableSessionGraph(writer, {
      commandRef: input.command.commandRef,
      descendantAgentRefs: quiesced.quiescedAgentRefs,
      evidenceRefs: quiesced.evidenceRefs,
    }, "mutation.port03.interrupted.quiesce"))
    const firstTransfer = input.capabilityTransfers[0]!
    await Effect.runPromise(broker.reissue({
      operationRef: `operation.${input.command.commandRef}.capability.${firstTransfer.sourceLeaseRef}.reissue`,
      leaseRef: firstTransfer.sourceLeaseRef,
      newLeaseRef: firstTransfer.destinationLeaseRef,
      destinationSourceGrantRef: firstTransfer.destinationSourceGrantRef,
      destinationAttachmentRef: input.destinationAttachmentRef,
      destinationAttachmentGeneration: 2,
      destinationTargetRef: managedTargetRef,
      expiresAt: firstTransfer.expiresAt,
    }))

    const restarted = SQL({ url: pg.urlFor("khala_sync_portable_move"), max: 2 })
    const restoredBroker = await PortableCapabilityBroker.restore(brokerConfig)
    expect(restoredBroker).not.toBe(broker)
    const resumed = new PortableSessionMoveCoordinator({
      sql: restarted as unknown as SyncSql,
      transaction: run => withSyncTransaction(restarted as unknown as SyncSql, run),
      broker: restoredBroker,
    })
    const result = await resumed.move(input)
    await restarted.end()
    expect(result.status).toBe("completed")
    expect(result.capabilityLeaseRefs).toHaveLength(2)
    expect(restoredBroker.snapshot().leases.filter(row => row.lease.targetRef === managedTargetRef && row.lease.state === "redeemed")).toHaveLength(2)
    expect(log.targetOperations.filter(operation => operation.endsWith("source.quiesce"))).toHaveLength(2)
    const snapshot = await readPortableSessionAuthoritySnapshot(sql as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    expect(snapshot?.commands).toHaveLength(1)
    expect(snapshot?.attachments.filter(row => row.state === "active")).toHaveLength(1)
  })

  test("stale generation is refused before runtime mutation and remains refused after a fresh SQL handle", async () => {
    const { coordinator, local, managed, log } = await setup()
    const stale = firstMove(local, managed)
    const result = await coordinator.move({
      ...stale,
      command: { ...stale.command, expectedGeneration: 7 },
    })
    expect(result.status).toBe("failed")
    expect(log.targetOperations).toEqual([])
    const restarted = SQL({ url: pg.urlFor("khala_sync_portable_move"), max: 1 })
    const snapshot = await readPortableSessionAuthoritySnapshot(restarted as unknown as SyncSql, { sessionRef, ownerUserId: owner })
    await restarted.end()
    expect(snapshot?.commands).toHaveLength(0)
    expect(snapshot?.session.current_attachment_ref).toBe(sourceAttachmentRef)
    expect(snapshot?.attachments.map(row => row.state)).toEqual(["active"])
  })
})
