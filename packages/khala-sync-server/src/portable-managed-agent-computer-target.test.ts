import { SQL } from "@openagentsinc/postgres-runtime"
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"
import type { PortableAgentGraph, PortableCheckpoint } from "@openagentsinc/portable-session-contract"

import { runMigrations } from "./migrate.js"
import {
  PortableManagedAgentComputerTargetError,
  PostgresManagedAgentComputerTarget,
  type ManagedAgentComputerPortableProvisioner,
} from "./portable-managed-agent-computer-target.js"
import { computePortableCheckpointDigest, type PortableCheckpointBundle } from "./portable-session-move.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres, type LocalPostgres } from "./test/local-postgres.js"
const ownerRef = "owner.port03.managed"
const targetRef = "target.port03.managed.agent-computer"
const sourceTargetRef = "target.port03.managed.local"
const NOW = new Date("2026-07-13T06:00:30.000Z")

const binding = (sessionRef: string) => ({
  schema: "openagents.portable_session_execution_binding.v1" as const,
  sessionRef,
  ownerRef,
  runRef: `run.${sessionRef}`,
  repositoryRef: "repository.OpenAgentsInc.openagents",
  pinnedBaseRef: "revision.port03.managed.base",
})

const graph = (generation: number): PortableAgentGraph => ({
  rootAgentRef: "agent.port03.managed.root",
  nodes: [
    {
      agentRef: "agent.port03.managed.root",
      threadRef: "thread.port03.managed.root",
      transcriptRef: "transcript.port03.managed.root",
      activityCursor: 4,
      lifecycle: "running",
      attachmentGeneration: generation,
    },
    {
      agentRef: "agent.port03.managed.child",
      parentAgentRef: "agent.port03.managed.root",
      threadRef: "thread.port03.managed.child",
      transcriptRef: "transcript.port03.managed.child",
      activityCursor: 2,
      lifecycle: "waiting",
      attachmentGeneration: generation,
    },
  ],
})

const cursors = [
  {
    threadRef: "thread.port03.managed.root",
    transcriptRef: "transcript.port03.managed.root",
    activityCursor: 4,
    eventCursor: 8,
  },
  {
    threadRef: "thread.port03.managed.child",
    transcriptRef: "transcript.port03.managed.child",
    activityCursor: 2,
    eventCursor: 5,
  },
]

const sha = (character: string): `sha256:${string}` =>
  `sha256:${character.repeat(64)}`

const bundle = (
  sessionRef: string,
  attachmentRef: string,
  generation: number,
  checkpointRef: string,
): PortableCheckpointBundle => {
  const withoutDigest = {
    schema: "openagents.portable_checkpoint.v1" as const,
    checkpointRef,
    sessionRef,
    sourceAttachmentRef: attachmentRef,
    sourceGeneration: generation,
    repositoryRef: binding(sessionRef).repositoryRef,
    repositoryRevisionRef: `revision.port03.managed.${generation}`,
    repositoryPostImageDigest: sha("a"),
    diffDigest: sha("b"),
    eventLogCursor: 13,
    catalogGenerationRef: `catalog.port03.managed.${generation}`,
    graphDigest: sha("c"),
    approvalRefs: ["approval.port03.managed.repository"],
    artifactRefs: ["artifact.port03.managed.checkpoint"],
    receiptRefs: ["receipt.port03.managed.checkpoint"],
    secretMaterial: "excluded" as const,
    processState: "excluded" as const,
  }
  const checkpoint: PortableCheckpoint = {
    ...withoutDigest,
    digest: computePortableCheckpointDigest(withoutDigest),
  }
  return {
    checkpoint,
    executionBinding: binding(sessionRef),
    graph: graph(generation),
    threadCursors: cursors,
  }
}

type Counts = Record<"stage" | "activate" | "abort" | "quiesce" | "checkpoint" | "reclaim", number>

const fakeProvisioner = (counts: Counts): ManagedAgentComputerPortableProvisioner => ({
  stage: async input => {
    counts.stage += 1
    return {
      destinationRunnerSessionReservationRef:
        `runner-session-reservation.${input.bundle.checkpoint.sessionRef}`,
      resourceRef: `resource.agent-computer.${input.bundle.checkpoint.sessionRef}`,
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
    counts.activate += 1
    const nodes = graph(input.generation).nodes
    const observedAt = "2026-07-13T06:00:00.000Z"
    return {
      schema: "openagents.ide_portable_destination_activation.v1",
      receiptRef: `receipt.${input.operationRef}`,
      operationRef: input.operationRef,
      sessionRef: input.sessionRef,
      checkpointRef: input.checkpointRef,
      destinationTargetRef: input.targetRef,
      destinationAttachmentRef: input.attachmentRef,
      destinationRunnerSessionReservationRef:
        `runner-session-reservation.${input.sessionRef}`,
      destinationGeneration: input.generation,
      authentication: {
        state: "reauthenticated",
        policyRef: "policy.portable.destination.openagents_managed.v1",
        evidenceRef: input.authorityEvidenceRef,
        observedAt,
        expiresAt: null,
      },
      helpersObservedAt: observedAt,
      helpers: (["pty", "lsp", "dap", "watcher", "native"] as const).map(kind => ({
        kind, readiness: "unsupported" as const, instanceRef: null, versionRef: null,
        omissionRef: `omission.managed.${kind}`, evidenceRefs: [],
      })),
      activatedAgentRefs: nodes.map(node => node.agentRef),
      acceptedWorkRefs: nodes.map(node => ({
        agentRef: node.agentRef,
        turnRef: `turn.${input.generation}.${node.agentRef}`,
      })),
      evidenceRefs: [`evidence.${input.operationRef}`, input.authorityEvidenceRef],
    }
  },
  abort: async input => {
    counts.abort += 1
    return { evidenceRefs: [`evidence.${input.operationRef}.reclaimed`] }
  },
  quiesce: async input => {
    counts.quiesce += 1
    return {
      quiescedAgentRefs: input.graph.nodes.map(node => node.agentRef),
      evidenceRefs: [`evidence.${input.operationRef}`],
    }
  },
  checkpoint: async input => {
    counts.checkpoint += 1
    return bundle(input.sessionRef, input.attachmentRef, input.generation, input.checkpointRef)
  },
  reclaim: async input => {
    counts.reclaim += 1
    return {
      cleanedAgentRefs: input.agentRefs,
      processes: "released",
      scratch: "released",
      ports: "released",
      evidenceRefs: [`evidence.${input.operationRef}`],
    }
  },
})

describe.skipIf(!hasLocalPostgres())("PORT-03 managed Agent Computer target", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_portable_managed_target")
    await admin.end()
    await runMigrations({ databaseUrl: pg.urlFor("khala_sync_portable_managed_target") })
    sql = SQL({ url: pg.urlFor("khala_sync_portable_managed_target"), max: 10 })
  })

  afterAll(async () => {
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  const seed = async (suffix: string): Promise<Readonly<{
    sessionRef: string
    sourceAttachmentRef: string
    destinationAttachmentRef: string
  }>> => {
    const sessionRef = `session.port03.managed.${suffix}`
    const sourceAttachmentRef = `attachment.port03.managed.${suffix}.source`
    const destinationAttachmentRef = `attachment.port03.managed.${suffix}.destination`
    await sql`
      INSERT INTO khala_sync_portable_targets
        (target_ref, owner_user_id, target_class, adapter_ref, compatibility_ref,
         isolation, data_posture, health)
      VALUES
        (${sourceTargetRef}, ${ownerRef}, 'owner_local', 'adapter.port03.local',
         'compat.port03.v1', 'owner_host_process', 'owner_device_only', 'ready'),
        (${targetRef}, ${ownerRef}, 'openagents_managed', 'adapter.port03.agent-computer',
         'compat.port03.v1', 'dedicated_microvm', 'openagents_managed_region', 'ready')
      ON CONFLICT (target_ref) DO NOTHING
    `
    await sql`
      INSERT INTO khala_sync_portable_sessions
        (session_ref, owner_user_id, owner_scope_ref, work_context_ref, event_log_ref,
         current_projection_ref, command_scope_ref, root_agent_ref, state,
         latest_event_cursor, current_attachment_ref, current_attachment_generation)
      VALUES
        (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`}, ${`work.${sessionRef}`},
         ${`eventlog.${sessionRef}`}, ${`projection.${sessionRef}`}, ${`commands.${sessionRef}`},
         'agent.port03.managed.root', 'active', 13, ${sourceAttachmentRef}, 1)
    `
    await sql`
      INSERT INTO khala_sync_portable_session_targets (session_ref, target_ref)
      VALUES (${sessionRef}, ${sourceTargetRef}), (${sessionRef}, ${targetRef})
    `
    await sql`
      INSERT INTO khala_sync_portable_attachments
        (attachment_ref, session_ref, target_ref, generation, state,
         descendant_agent_refs_json, capability_lease_refs_json, evidence_refs_json)
      VALUES
        (${sourceAttachmentRef}, ${sessionRef}, ${sourceTargetRef}, 1, 'active',
         ${JSON.stringify(graph(1).nodes.map(node => node.agentRef))}::jsonb,
         '[]'::jsonb, '["evidence.port03.managed.source"]'::jsonb)
    `
    return { sessionRef, sourceAttachmentRef, destinationAttachmentRef }
  }

  const commitDestinationAuthority = async (fixture: Awaited<ReturnType<typeof seed>>): Promise<void> => {
    await sql.begin(async tx => {
      await tx`
        UPDATE khala_sync_portable_attachments SET state = 'detached'
        WHERE attachment_ref = ${fixture.sourceAttachmentRef}
      `
      await tx`
        INSERT INTO khala_sync_portable_attachments
          (attachment_ref, session_ref, target_ref, generation, state,
           descendant_agent_refs_json, capability_lease_refs_json, checkpoint_ref,
           evidence_refs_json)
        VALUES
          (${fixture.destinationAttachmentRef}, ${fixture.sessionRef}, ${targetRef}, 2, 'active',
           ${JSON.stringify(graph(2).nodes.map(node => node.agentRef))}::jsonb,
           '["lease.port03.managed.provider"]'::jsonb,
           ${`checkpoint.${fixture.sessionRef}.source`},
           '["evidence.port03.managed.authority-commit"]'::jsonb)
      `
      await tx`
        UPDATE khala_sync_portable_sessions
        SET current_attachment_ref = ${fixture.destinationAttachmentRef},
            current_attachment_generation = 2
        WHERE session_ref = ${fixture.sessionRef}
      `
    })
  }

  test("retains a non-accepting stage, gates activation on authority, and replays after restart", async () => {
    const fixture = await seed("lifecycle")
    const counts: Counts = { stage: 0, activate: 0, abort: 0, quiesce: 0, checkpoint: 0, reclaim: 0 }
    const provisioner = fakeProvisioner(counts)
    const target = new PostgresManagedAgentComputerTarget({
      sql: sql as unknown as SyncSql,
      ownerRef,
      targetRef,
      provisioner,
      now: () => NOW,
    })
    const sourceBundle = bundle(
      fixture.sessionRef,
      fixture.sourceAttachmentRef,
      1,
      `checkpoint.${fixture.sessionRef}.source`,
    )
    const stageInput = {
      operationRef: `operation.${fixture.sessionRef}.stage`,
      bundle: sourceBundle,
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: ["lease.port03.managed.provider"],
    }

    expect((await target.stageCheckpoint(stageInput)).acceptingWork).toBe(false)
    const staged = await sql`
      SELECT state, accepting_work FROM khala_sync_portable_managed_targets
      WHERE session_ref = ${fixture.sessionRef}
    `
    expect(staged[0]).toMatchObject({ state: "staged", accepting_work: false })

    const activateInput = {
      operationRef: `operation.${fixture.sessionRef}.activate`,
      checkpointRef: sourceBundle.checkpoint.checkpointRef,
      sessionRef: fixture.sessionRef,
      executionBinding: binding(fixture.sessionRef),
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: ["lease.port03.managed.provider"],
    }
    await expect(target.activate(activateInput)).rejects.toMatchObject({ code: "authority_not_committed" })
    expect(counts.activate).toBe(0)

    await commitDestinationAuthority(fixture)
    const activated = await target.activate(activateInput)
    expect(activated.activatedAgentRefs).toHaveLength(2)
    expect(counts.activate).toBe(1)

    const restarted = new PostgresManagedAgentComputerTarget({
      sql: sql as unknown as SyncSql,
      ownerRef,
      targetRef,
      provisioner,
      now: () => NOW,
    })
    expect(await restarted.stageCheckpoint(stageInput)).toEqual(await target.stageCheckpoint(stageInput))
    expect(await restarted.activate(activateInput)).toEqual(activated)
    expect(counts).toMatchObject({ stage: 1, activate: 1 })

    // Simulate a crash after the provisioner effect and durable target-state
    // transition but before the operation result was acknowledged. The same
    // bytes may re-enter the idempotent provisioner, then complete durably.
    await sql`
      UPDATE khala_sync_portable_managed_target_operations
      SET status = 'pending', result_json = NULL
      WHERE owner_user_id = ${ownerRef} AND target_ref = ${targetRef}
        AND operation_ref = ${activateInput.operationRef}
    `
    expect(await restarted.activate(activateInput)).toEqual(activated)
    expect(counts.activate).toBe(2)
    expect(await restarted.activate(activateInput)).toEqual(activated)
    expect(counts.activate).toBe(2)

    const managedGraph = graph(2)
    const quiesceInput = {
      operationRef: `operation.${fixture.sessionRef}.quiesce`,
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.destinationAttachmentRef,
      generation: 2,
      graph: managedGraph,
      threadCursors: cursors,
    }
    expect((await restarted.quiesceGraph(quiesceInput)).quiescedAgentRefs).toHaveLength(2)
    await expect(restarted.quiesceGraph({ ...quiesceInput, operationRef: `${quiesceInput.operationRef}.stale`, generation: 1 }))
      .rejects.toMatchObject({ code: "stale_generation" })
    expect(counts.quiesce).toBe(1)
    const nextBundle = await restarted.createCheckpoint({
      operationRef: `operation.${fixture.sessionRef}.checkpoint`,
      checkpointRef: `checkpoint.${fixture.sessionRef}.failback`,
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.destinationAttachmentRef,
      generation: 2,
      eventLogCursor: 13,
      executionBinding: binding(fixture.sessionRef),
      graph: managedGraph,
      threadCursors: cursors,
    })
    expect(nextBundle.checkpoint.sourceGeneration).toBe(2)
    const cleanup = await restarted.cleanupSource({
      operationRef: `operation.${fixture.sessionRef}.cleanup`,
      sessionRef: fixture.sessionRef,
      attachmentRef: fixture.destinationAttachmentRef,
      generation: 2,
      agentRefs: managedGraph.nodes.map(node => node.agentRef),
    })
    expect(cleanup).toMatchObject({ processes: "released", scratch: "released", ports: "released" })
    const reclaimed = await sql`
      SELECT state, accepting_work FROM khala_sync_portable_managed_targets
      WHERE session_ref = ${fixture.sessionRef}
    `
    expect(reclaimed[0]).toMatchObject({ state: "reclaimed", accepting_work: false })
  })

  test("aborts one retained stage idempotently and refuses conflicting operation bytes", async () => {
    const fixture = await seed("abort")
    const counts: Counts = { stage: 0, activate: 0, abort: 0, quiesce: 0, checkpoint: 0, reclaim: 0 }
    const provisioner = fakeProvisioner(counts)
    const target = new PostgresManagedAgentComputerTarget({
      sql: sql as unknown as SyncSql,
      ownerRef,
      targetRef,
      provisioner,
    })
    const stageInput = {
      operationRef: `operation.${fixture.sessionRef}.stage`,
      bundle: bundle(fixture.sessionRef, fixture.sourceAttachmentRef, 1, `checkpoint.${fixture.sessionRef}.source`),
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: [] as string[],
    }
    await target.stageCheckpoint(stageInput)
    await expect(target.stageCheckpoint({ ...stageInput, destinationGeneration: 3 })).rejects.toBeInstanceOf(
      PortableManagedAgentComputerTargetError,
    )
    expect(counts.stage).toBe(1)

    const abortInput = {
      operationRef: `operation.${fixture.sessionRef}.abort`,
      sessionRef: fixture.sessionRef,
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
    }
    const first = await target.abortStaged(abortInput)
    const restarted = new PostgresManagedAgentComputerTarget({
      sql: sql as unknown as SyncSql,
      ownerRef,
      targetRef,
      provisioner,
    })
    expect(await restarted.abortStaged(abortInput)).toEqual(first)
    expect(counts.abort).toBe(1)
  })

  test("rejects a private-shaped managed provisioner receipt before persistence", async () => {
    const fixture = await seed("private")
    const counts: Counts = { stage: 0, activate: 0, abort: 0, quiesce: 0, checkpoint: 0, reclaim: 0 }
    const base = fakeProvisioner(counts)
    const target = new PostgresManagedAgentComputerTarget({
      sql: sql as unknown as SyncSql,
      ownerRef,
      targetRef,
      provisioner: {
        ...base,
        stage: async input => ({
          ...(await base.stage(input)),
          resourceRef: "/home/agent/private.sock",
        }),
      },
    })
    await expect(target.stageCheckpoint({
      operationRef: `operation.${fixture.sessionRef}.stage`,
      bundle: bundle(fixture.sessionRef, fixture.sourceAttachmentRef, 1, `checkpoint.${fixture.sessionRef}.source`),
      destinationAttachmentRef: fixture.destinationAttachmentRef,
      destinationGeneration: 2,
      capabilityLeaseRefs: [],
    })).rejects.toMatchObject({ code: "unsafe_result" })
    const retained = await sql`
      SELECT COUNT(*)::int AS count FROM khala_sync_portable_managed_targets
      WHERE session_ref = ${fixture.sessionRef}
    `
    expect(Number(retained[0]!.count)).toBe(0)
  })
})
