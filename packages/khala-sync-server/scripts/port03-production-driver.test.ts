import { createHash } from "node:crypto"

import { SQL } from "@openagentsinc/postgres-runtime"
import { afterAll, beforeAll, describe, expect, test } from "vite-plus/test"
import {
  makeOpenAgentsManagedCapabilityAdapter,
  makeOwnerLocalCapabilityAdapter,
  type PortableAgentGraph,
  type SecretMaterial,
} from "@openagentsinc/portable-session-contract"

import { runMigrations } from "../src/migrate.js"
import type { SyncSql } from "../src/sql.js"
import type { PortableManagedContinuationAuthority } from "../src/portable-managed-continuation.js"
import {
  PostgresPortableSessionMoveRuntime,
} from "../src/portable-session-move-runtime.js"
import type {
  PortableSessionExecutionTarget,
  PortableSessionMoveInput,
  PortableSessionMoveResult,
} from "../src/portable-session-move.js"
import {
  hasLocalPostgres,
  startLocalPostgres,
  type LocalPostgres,
} from "../src/test/local-postgres.js"
import {
  createPortableSessionProductionBroker,
  PortableSessionProductionDriver,
  PortableSessionProductionDriverError,
} from "./port03-production-driver.js"
const ownerRef = "owner.port03.driver"
const sessionRef = "session.port03.driver"
const localTargetRef = "target.port03.driver.local"
const managedTargetRef = "target.port03.driver.managed"
const sourceAttachmentRef = "attachment.port03.driver.local.1"
const managedAttachmentRef = "attachment.port03.driver.managed.2"
const destinationAttachmentRef = "attachment.port03.driver.local.3"

const unreachable = async (): Promise<never> => {
  throw new Error("driver orchestration test must not invoke target effects")
}

const target = (
  targetRef: string,
  targetClass: "owner_local" | "openagents_managed",
): PortableSessionExecutionTarget => ({
  targetRef,
  targetClass,
  quiesceGraph: unreachable,
  createCheckpoint: unreachable,
  cleanupSource: unreachable,
  stageCheckpoint: unreachable,
  activate: unreachable,
  abortStaged: unreachable,
})

const local = target(localTargetRef, "owner_local")
const managed = target(managedTargetRef, "openagents_managed")

const graph = {
  rootAgentRef: "agent.port03.driver.root",
  nodes: [
    {
      agentRef: "agent.port03.driver.root",
      threadRef: "thread.port03.driver.root",
      transcriptRef: "transcript.port03.driver.root",
      activityCursor: 1,
      lifecycle: "running" as const,
      attachmentGeneration: 1,
    },
    {
      agentRef: "agent.port03.driver.child",
      parentAgentRef: "agent.port03.driver.root",
      threadRef: "thread.port03.driver.child",
      transcriptRef: "transcript.port03.driver.child",
      activityCursor: 1,
      lifecycle: "running" as const,
      attachmentGeneration: 1,
    },
  ],
}

const executionBinding = {
  schema: "openagents.portable_session_execution_binding.v1" as const,
  sessionRef,
  ownerRef,
  runRef: "run.port03.driver.canonical",
  repositoryRef: "repository.OpenAgentsInc.openagents",
  pinnedBaseRef: "revision.port03.driver.pinned",
}

const command = (
  kind: "move" | "failback",
  attachmentRef: string,
  generation: number,
  destinationTargetRef: string,
) => ({
  schema: "openagents.portable_session_command.v1" as const,
  commandRef: `command.port03.driver.${kind}`,
  idempotencyKey: `idempotency.port03.driver.${kind}`,
  ownerRef,
  sessionRef,
  kind,
  expectedAttachmentRef: attachmentRef,
  expectedGeneration: generation,
  destinationTargetRef,
  checkpointRef: `checkpoint.port03.driver.${kind}`,
  expiresAt: "2099-01-01T00:00:00.000Z",
})

const legs = {
  localToManaged: {
    moveRef: "move.port03.driver.outbound",
    command: command("move", sourceAttachmentRef, 1, managedTargetRef),
    destinationAttachmentRef: managedAttachmentRef,
    destinationRunnerSessionRef: "runner.port03.driver.managed",
    capabilityTransfers: [],
  },
  managedToLocal: {
    moveRef: "move.port03.driver.failback",
    command: command("failback", managedAttachmentRef, 2, localTargetRef),
    destinationAttachmentRef,
    destinationRunnerSessionRef: "runner.port03.driver.local",
    capabilityTransfers: [],
  },
}

const managedContinuation = {
  operationRef: "operation.port03.driver.continue",
  providerLeaseRef: "lease.port03.driver.managed.provider",
  turns: graph.nodes.map(node => ({
    agentRef: node.agentRef,
    turnRef: `turn.port03.driver.${node.agentRef}`,
    task: `Continue the bounded fixture for ${node.agentRef}`,
  })),
}

const continuationAuthority: PortableManagedContinuationAuthority = {
  readExpectedCursors: async (input: { expectedGraph: PortableAgentGraph }) => input.expectedGraph.nodes.map(node => ({
    agentRef: node.agentRef,
    threadRef: node.threadRef,
    activityCursor: node.activityCursor,
    eventCursor: 1,
  })),
  commit: async input => input.receipt,
}

const result = (input: PortableSessionMoveInput): PortableSessionMoveResult => ({
  schema: "openagents.portable_session_move.v1",
  status: "completed",
  commandRef: input.command.commandRef,
  sessionRef,
  runRef: executionBinding.runRef,
  repositoryRef: executionBinding.repositoryRef,
  pinnedBaseRef: executionBinding.pinnedBaseRef,
  sourceAttachmentRef: input.command.expectedAttachmentRef,
  sourceGeneration: input.command.expectedGeneration,
  destinationAttachmentRef: input.destinationAttachmentRef,
  destinationGeneration: input.command.expectedGeneration + 1,
  ...(input.command.checkpointRef === undefined ? {} : { checkpointRef: input.command.checkpointRef }),
  capabilityLeaseRefs: [],
  acceptedWorkRefs: [],
  evidenceRefs: [`evidence.${input.command.commandRef}`],
})

const material = new TextEncoder().encode("fixture-only-secret") as SecretMaterial
const adapterPort = {
  install: async ({ lease }: { lease: { leaseRef: string } }) => ({
    installationRef: `installation.${lease.leaseRef}`,
  }),
  wipe: async ({ leaseRef }: { leaseRef: string }) => ({ wipeReceiptRef: `receipt.${leaseRef}.wiped` }),
}
const localAdapter = makeOwnerLocalCapabilityAdapter("adapter.port03.driver.local", adapterPort)
const managedAdapter = makeOpenAgentsManagedCapabilityAdapter("adapter.port03.driver.managed", adapterPort)
const broker = {
  vault: {
    withSourceGrantMaterial: async <A>({ use }: { use: (value: SecretMaterial) => Promise<A> }) => use(material),
    revokeSourceGrant: async () => undefined,
  },
  targets: [
    { targetRef: localTargetRef, targetClass: "owner_local" as const, adapterRef: localAdapter.adapterRef, ready: true },
    { targetRef: managedTargetRef, targetClass: "openagents_managed" as const, adapterRef: managedAdapter.adapterRef, ready: true },
  ],
  adapters: [localAdapter, managedAdapter],
  clock: { now: () => new Date("2026-07-13T12:00:00.000Z") },
}

test("production broker composes exact grant reissue and managed marker installation", async () => {
  const requests: string[] = []
  const targetRequests: string[] = []
  const installed: string[] = []
  const managedResourceRef = "resource.port03.driver.managed"
  const fetch = async (request: string | URL | Request, init?: RequestInit): Promise<Response> => {
    const url = new URL(request instanceof Request ? request.url : request.toString())
    requests.push(url.pathname)
    const body = JSON.parse(String(init?.body)) as Record<string, unknown>
    if (url.pathname.endsWith("/revoke")) {
      return Response.json({ grant: { grantRef: body.grantRef, status: "revoked" }, material: "excluded" })
    }
    if (url.pathname.endsWith("/reissue")) {
      return Response.json({ grant: { grantRef: body.destinationGrantRef, status: "issued" }, material: "excluded" })
    }
    return Response.json({
      grant: { grantRef: body.grantRef, status: "resolved" },
      authMaterial: { authContentJson: "fixture-production-broker-material" },
    })
  }
  const resourceSql = (async () => [{
    owner_user_id: ownerRef,
    target_ref: managedTargetRef,
    session_ref: sessionRef,
    attachment_ref: managedAttachmentRef,
    generation: 2,
    resource_ref: managedResourceRef,
    state: "staged",
    accepting_work: false,
  }]) as unknown as SyncSql
  const targetFetch = async (request: string | URL | Request, init?: RequestInit): Promise<Response> => {
    const url = new URL(request instanceof Request ? request.url : request.toString())
    targetRequests.push(url.pathname)
    const headers = new Headers(init?.headers)
    const leaseRef = headers.get("X-OA-Lease-Ref")!
    const evidenceRef = headers.get("X-OA-Evidence-Ref")!
    installed.push(new TextDecoder().decode(init?.body as Uint8Array))
    return Response.json({
      installationRef: `installation.agent-computer.capability.${createHash("sha256")
        .update(`${managedResourceRef}|${leaseRef}`)
        .digest("hex")
        .slice(0, 16)}`,
      evidenceRef,
      resourceRef: managedResourceRef,
      marker: { leaseRef, evidenceRef },
      material: "excluded",
    })
  }
  const production = createPortableSessionProductionBroker({
    grantAuthority: {
      baseUrl: "https://openagents.example",
      serviceBearer: "service.fixture.port03",
      fetch,
      bindings: [{
        grantRef: "grant.port03.driver.source",
        ownerUserId: ownerRef,
        kind: "provider",
        providerAccountRef: "account.port03.driver",
      }],
    },
    sql: resourceSql,
    ownerRef,
    sessionRef,
    local: {
      targetRef: localTargetRef,
      adapterRef: "adapter.port03.production.local",
      installation: { pylonHome: "/tmp/openagents-port03-driver-pylon" },
    },
    managed: {
      targetRef: managedTargetRef,
      adapterRef: "adapter.port03.production.managed",
      installation: {
        baseUrl: "https://agent-computer.example",
        bearerToken: "fixture-managed-bearer-token",
        fetch: targetFetch,
      },
    },
  })
  const transferLeg = {
    ...legs.localToManaged,
    capabilityTransfers: [{
      sourceLeaseRef: "lease.port03.driver.source",
      destinationLeaseRef: "lease.port03.driver.destination",
      destinationSourceGrantRef: "grant.port03.driver.destination",
      expiresAt: "2099-01-01T00:00:00.000Z",
    }],
  }
  production.prepare(transferLeg)
  await production.config.vault.revokeSourceGrant({
    sourceGrantRef: "grant.port03.driver.source",
    leaseRef: "lease.port03.driver.source",
  })
  const managedAdapter = production.config.adapters.find(adapter =>
    adapter.targetClass === "openagents_managed")!
  await production.config.vault.withSourceGrantMaterial({
    sourceGrantRef: "grant.port03.driver.destination",
    leaseRef: "lease.port03.driver.destination",
    use: material => managedAdapter.redeem({
      lease: {
        leaseRef: "lease.port03.driver.destination",
        ownerRef,
        sessionRef,
        attachmentRef: managedAttachmentRef,
        attachmentGeneration: 2,
        targetRef: managedTargetRef,
        capability: "provider",
        accountRef: "account.port03.driver",
        expiresAt: "2099-01-01T00:00:00.000Z",
        state: "issued",
      },
      permissions: ["provider.turn.execute"],
      material,
    }),
  })

  expect(requests).toEqual([
    "/api/portable-capability-grants/provider/revoke",
    "/api/portable-capability-grants/provider/reissue",
    "/api/provider-accounts/chatgpt-codex/grants/resolve",
  ])
  expect(targetRequests).toEqual(["/v1/portable-agent-computers/capabilities/install"])
  expect(installed).toEqual(["fixture-production-broker-material"])
  expect(JSON.stringify(production.config.targets)).not.toContain("fixture-production-broker-material")
})

describe.skipIf(!hasLocalPostgres())("PORT-03 production round-trip driver", () => {
  let pg: LocalPostgres
  let sql: SQL

  beforeAll(async () => {
    pg = await startLocalPostgres()
    const admin = SQL({ url: pg.url, max: 1 })
    await admin.unsafe("CREATE DATABASE khala_sync_port03_driver")
    await admin.end()
    const databaseUrl = pg.urlFor("khala_sync_port03_driver")
    await runMigrations({ databaseUrl })
    sql = SQL({ url: databaseUrl, max: 4 })
    await sql`
      INSERT INTO khala_sync_portable_sessions
        (session_ref, owner_user_id, owner_scope_ref, work_context_ref,
         event_log_ref, current_projection_ref, command_scope_ref,
         root_agent_ref, state)
      VALUES
        (${sessionRef}, ${ownerRef}, ${`scope.user.${ownerRef}`},
         ${`work.${sessionRef}`}, ${`eventlog.${sessionRef}`},
         ${`current.${sessionRef}`}, ${`commands.${sessionRef}`},
         ${graph.rootAgentRef}, 'active')
    `
  })

  afterAll(async () => {
    material.fill(0)
    if (sql !== undefined) await sql.end()
    if (pg !== undefined) await pg.stop()
  })

  test("sequences local → managed continuation → local through durable move claims", async () => {
    const invoked: string[] = []
    const runtime = new PostgresPortableSessionMoveRuntime({
      sql: sql as unknown as SyncSql,
      transaction: unreachable,
      coordinatorFactory: () => ({
        move: async input => {
          invoked.push(input.command.commandRef)
          return result(input)
        },
      }),
    })
    const driver = new PortableSessionProductionDriver({
      runtime,
      broker,
      local,
      managed,
      continuationAuthority,
      continuation: {
        run: async input => ({
          acceptedWorkRefs: input.plan.turns.map(({ agentRef, turnRef }) => ({ agentRef, turnRef })),
          threadCursors: input.expectedGraph.nodes.map(node => ({
            agentRef: node.agentRef,
            threadRef: node.threadRef,
            activityCursor: node.activityCursor + 1,
            eventCursor: 2,
          })),
          evidenceRefs: ["evidence.port03.driver.continuation"],
          replay: "executed",
        }),
      },
    })

    const receipt = await driver.runRoundTrip({
      proofClass: "deterministic",
      executionBinding,
      expectedGraph: graph,
      managedContinuation,
      ...legs,
    })

    expect(invoked).toEqual([
      legs.localToManaged.command.commandRef,
      legs.managedToLocal.command.commandRef,
    ])
    expect(receipt).toMatchObject({
      schema: "openagents.portable_session_round_trip_candidate.v1",
      proofClass: "deterministic",
      sessionRef,
      runRef: executionBinding.runRef,
      repositoryRef: executionBinding.repositoryRef,
      pinnedBaseRef: executionBinding.pinnedBaseRef,
      localSourceAttachmentRef: sourceAttachmentRef,
      managedAttachmentRef,
      localDestinationAttachmentRef: destinationAttachmentRef,
      finalGeneration: 3,
      liveAcceptanceClaimed: false,
    })
    expect(receipt.acceptedWorkRefs.map(row => row.agentRef).sort()).toEqual(
      graph.nodes.map(node => node.agentRef).sort(),
    )
    expect(JSON.stringify(receipt)).not.toContain("fixture-only-secret")

    const claims = await sql`
      SELECT active_move_ref
      FROM khala_sync_portable_capability_brokers
      WHERE owner_user_id = ${ownerRef} AND session_ref = ${sessionRef}
    `
    expect(claims).toHaveLength(1)
    expect(claims[0]?.active_move_ref).toBeNull()
  })

  test("rejects a root-only graph before any move side effect", async () => {
    let moved = false
    const driver = new PortableSessionProductionDriver({
      runtime: { move: async () => { moved = true; throw new Error("unreachable") } },
      broker,
      local,
      managed,
      continuationAuthority,
      continuation: { run: async () => ({
        acceptedWorkRefs: [],
        threadCursors: [],
        evidenceRefs: [],
        replay: "executed",
      }) },
    })
    await expect(driver.runRoundTrip({
      proofClass: "deterministic",
      executionBinding,
      expectedGraph: { rootAgentRef: graph.rootAgentRef, nodes: [graph.nodes[0]!] },
      managedContinuation,
      ...legs,
    })).rejects.toBeInstanceOf(PortableSessionProductionDriverError)
    expect(moved).toBeFalse()
  })
})
