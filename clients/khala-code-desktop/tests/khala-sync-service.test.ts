import { describe, expect, test } from "bun:test"
import {
  BootstrapEntity,
  BootstrapResponse,
  type BootstrapRequest,
  canonicalJson,
  ChangelogEntry,
  EntityId,
  EntityType,
  fleetRunScope,
  LogPage,
  MustRefetchFrame,
  MutationResult,
  PushResponse,
  SyncVersion,
  SyncVersionWatermark,
  type MutationEnvelope,
  type SyncScope,
} from "@openagentsinc/khala-sync"
import {
  KhalaSyncTransportError,
  type KhalaSyncTransport,
  type LiveSocketHandlers,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"
import {
  createKhalaCodeDesktopKhalaSyncService,
  FLEET_PAUSE_RUN_MUTATOR_NAME,
  FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME,
  khalaCodeDesktopKhalaSyncFleetEnabled,
  khalaSyncFleetDisabledState,
} from "../src/bun/khala-sync-service"

/**
 * KS-6.2 (#8303) service tests: the desktop Khala Sync fleet consumer over
 * a deterministic in-memory fake transport (the khala-sync-client test
 * fakes are not exported, so this file carries a scoped local fake that
 * implements the SPEC §3 semantics the service exercises). No network, no
 * real sockets; timing is injected so backoff paths run instantly.
 */

const tick = (): Promise<void> => new Promise(resolve => setTimeout(resolve, 0))

const waitFor = async (
  condition: () => boolean | Promise<boolean>,
  label: string,
  ticks = 3000,
): Promise<void> => {
  for (let index = 0; index < ticks; index++) {
    if (await condition()) return
    await tick()
  }
  throw new Error(`timed out waiting for: ${label}`)
}

const RUN_ID = "fleet_run.test.1"
const SCOPE = fleetRunScope(RUN_ID)
const FIXED_TIME = "2026-07-04T00:00:00.000Z"

const fleetRunImage = (patch: Record<string, unknown> = {}): string =>
  canonicalJson({
    counters: {
      activeAssignments: 1,
      blockedAssignments: 0,
      completedAssignments: 2,
      failedAssignments: 0,
      workUnitsTotal: 5,
    },
    desiredSlots: 3,
    runId: RUN_ID,
    startedAt: FIXED_TIME,
    status: "running",
    updatedAt: FIXED_TIME,
    workerKind: "codex",
    ...patch,
  })

const workerImage = canonicalJson({
  accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
  phase: "dispatched",
  updatedAt: FIXED_TIME,
  workerId: "worker.slot.1",
})

const assignmentImage = canonicalJson({
  assignmentRef: "assignment.test.1",
  issueRef: "#8303",
  status: "running",
  updatedAt: FIXED_TIME,
})

const accountImage = canonicalJson({
  accountRefHash: "account.pylon.codex.0123456789abcdef01234567",
  readiness: "ready",
  updatedAt: FIXED_TIME,
})

interface FakeEntry {
  readonly entityType: string
  readonly entityId: string
  readonly postImageJson: string
}

/** Minimal SPEC §3 fake server for the fleet scope. */
class FakeFleetSyncServer {
  readonly log: Array<ChangelogEntry> = []
  socket: { handlers: LiveSocketHandlers; open: boolean } | null = null
  /** Mutator behavior switch: reject every push in-band (unauthorized_scope). */
  rejectPushes = false
  /** Hold pushes (network fault) so optimistic state stays unconfirmed. */
  holdPushes = false
  readonly seenAuthTokens: Array<string> = []
  readonly pushedMutations: Array<MutationEnvelope> = []
  clientLastMutationId = 0

  lastVersion(): number {
    return this.log.length === 0 ? 0 : this.log[this.log.length - 1]!.version
  }

  commit(entries: ReadonlyArray<FakeEntry>): void {
    const version = SyncVersion.make(this.lastVersion() + 1)
    const rows = entries.map(
      entry =>
        new ChangelogEntry({
          scope: SCOPE,
          version,
          entityType: EntityType.make(entry.entityType),
          entityId: EntityId.make(entry.entityId),
          op: "upsert",
          postImageJson: entry.postImageJson,
          committedAt: FIXED_TIME,
        }),
    )
    this.log.push(...rows)
    if (this.socket !== null && this.socket.open) {
      this.socket.handlers.onFrame({
        _tag: "DeltaFrame",
        scope: SCOPE,
        entries: rows,
        cursor: version,
      } as never)
    }
  }

  replaceScope(entries: ReadonlyArray<FakeEntry>): void {
    this.log.length = 0
    this.commit(entries)
  }

  emitMustRefetch(): void {
    if (this.socket !== null && this.socket.open) {
      this.socket.handlers.onFrame(
        new MustRefetchFrame({ scope: SCOPE, reason: "scope_reset" }) as never,
      )
    }
  }

  currentEntities(): Array<FakeEntry> {
    const state = new Map<string, FakeEntry>()
    for (const entry of this.log) {
      state.set(`${entry.entityType}/${entry.entityId}`, {
        entityType: entry.entityType,
        entityId: entry.entityId,
        postImageJson: entry.postImageJson!,
      })
    }
    return [...state.values()]
  }
}

const fakeTransport = (
  server: FakeFleetSyncServer,
  authToken: () => string,
): KhalaSyncTransport => ({
  bootstrap: (request: BootstrapRequest) =>
    Effect.sync(() => {
      server.seenAuthTokens.push(authToken())
      return new BootstrapResponse({
        protocolVersion: 1,
        scope: request.scope,
        entities: server
          .currentEntities()
          .map(
            entity =>
              new BootstrapEntity({
                entityType: EntityType.make(entity.entityType),
                entityId: EntityId.make(entity.entityId),
                postImageJson: entity.postImageJson,
              }),
          ),
        cursor: SyncVersionWatermark.make(server.lastVersion()),
      })
    }),
  logPage: (scope: SyncScope, cursor) =>
    Effect.sync(() => {
      server.seenAuthTokens.push(authToken())
      const entries = server.log.filter(entry => entry.version > cursor)
      const next = entries.length === 0
        ? cursor
        : entries[entries.length - 1]!.version
      return new LogPage({
        protocolVersion: 1,
        scope,
        entries,
        nextCursor: SyncVersionWatermark.make(next),
        upToDate: true,
      })
    }),
  push: request =>
    Effect.suspend(() => {
      server.seenAuthTokens.push(authToken())
      if (server.holdPushes) {
        return Effect.fail(
          new KhalaSyncTransportError("network", true, "fake offline push"),
        )
      }
      const results: Array<MutationResult> = []
      let last = server.clientLastMutationId
      for (const mutation of request.mutations) {
        server.pushedMutations.push(mutation)
        if (mutation.mutationId <= last) {
          results.push(
            new MutationResult({ mutationId: mutation.mutationId, status: "duplicate" }),
          )
          continue
        }
        last = mutation.mutationId
        if (server.rejectPushes) {
          results.push(
            new MutationResult({
              errorCode: "unauthorized_scope",
              errorMessageSafe: "this fleet run scope belongs to a different user",
              mutationId: mutation.mutationId,
              status: "rejected",
            }),
          )
          continue
        }
        const args = JSON.parse(mutation.argsJson) as {
          runId: string
          desiredSlots?: number
        }
        const current = server
          .currentEntities()
          .find(entity => entity.entityType === "fleet_run" && entity.entityId === args.runId)
        const base = current === undefined
          ? (JSON.parse(fleetRunImage()) as Record<string, unknown>)
          : (JSON.parse(current.postImageJson) as Record<string, unknown>)
        const patch: Record<string, unknown> =
          mutation.name === FLEET_SET_DESIRED_SLOTS_MUTATOR_NAME
            ? { desiredSlots: args.desiredSlots }
            : mutation.name === FLEET_PAUSE_RUN_MUTATOR_NAME
              ? { status: "paused" }
              : { status: "running" }
        server.commit([
          {
            entityType: "fleet_run",
            entityId: args.runId,
            postImageJson: canonicalJson({ ...base, ...patch }),
          },
        ])
        results.push(
          new MutationResult({ mutationId: mutation.mutationId, status: "applied" }),
        )
      }
      server.clientLastMutationId = last
      return Effect.succeed(
        new PushResponse({ protocolVersion: 1, results, lastMutationId: last }),
      )
    }),
  connectLive: (_scope, _cursor, handlers) =>
    Effect.sync(() => {
      server.seenAuthTokens.push(authToken())
      const record = { handlers, open: true }
      server.socket = record
      return {
        close: () => {
          record.open = false
        },
      }
    }),
})

type ServiceHarness = {
  readonly server: FakeFleetSyncServer
  readonly service: ReturnType<typeof createKhalaCodeDesktopKhalaSyncService>
  readonly env: Record<string, string | undefined>
}

const makeHarness = (
  envOverrides: Record<string, string | undefined> = {},
): ServiceHarness => {
  const server = new FakeFleetSyncServer()
  const env: Record<string, string | undefined> = {
    KHALA_SYNC_FLEET: "1",
    OPENAGENTS_AGENT_TOKEN: "oa_agent_test_token_1",
    OPENAGENTS_BASE_URL: "https://openagents.test",
    KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH: "/tmp/khala-sync-service-test-missing-settings.json",
    ...envOverrides,
  }
  const service = createKhalaCodeDesktopKhalaSyncService({
    env,
    storePath: ":memory:",
    sleep: () => tick(),
    random: () => 0.5,
    now: () => new Date(FIXED_TIME),
    transport: config => fakeTransport(server, config.authToken),
  })
  return { server, service, env }
}

describe("khala-sync-service flag gating", () => {
  test("flag parsing accepts 1/true only", () => {
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "1" })).toBe(true)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "true" })).toBe(true)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({ KHALA_SYNC_FLEET: "0" })).toBe(false)
    expect(khalaCodeDesktopKhalaSyncFleetEnabled({})).toBe(false)
  })

  test("flag off: state answers honestly disabled and mutate refuses", async () => {
    const { service } = makeHarness({ KHALA_SYNC_FLEET: undefined })
    expect(await service.fleetState({ runId: RUN_ID })).toEqual(khalaSyncFleetDisabledState())
    expect(await service.fleetMutate({ action: "pause", runId: RUN_ID })).toEqual({
      ok: false,
      error: "khala_sync_fleet_disabled",
    })
    await service.close()
  })

  test("missing OpenAgents auth: enabled but honestly not connected", async () => {
    const { service } = makeHarness({ OPENAGENTS_AGENT_TOKEN: undefined })
    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.enabled).toBe(true)
    expect(state.authState).toBe("missing")
    expect(state.phase).toBe("idle")
    expect(state.error).toBe("missing_openagents_auth")
    expect(await service.fleetMutate({ action: "resume", runId: RUN_ID })).toEqual({
      ok: false,
      error: "missing_openagents_auth",
    })
    await service.close()
  })
})

describe("khala-sync-service fleet scope consumption", () => {
  // Oracle khala_sync_phase_is_session_truth.service for contract
  // khala_code.fleet.khala_sync_indicator_truthful.v1: the RPC-exposed phase
  // is the session's real scope state — "live" appears only once bootstrap +
  // catch-up completed and the live socket is open.
  test("subscribe: synced fleet entities appear through the RPC view and phase reaches live", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
      { entityType: "fleet_worker", entityId: "worker.slot.1", postImageJson: workerImage },
      { entityType: "fleet_assignment", entityId: "assignment.test.1", postImageJson: assignmentImage },
      { entityType: "fleet_account", entityId: "account.pylon.codex.0123456789abcdef01234567", postImageJson: accountImage },
    ])

    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.phase === "live" && state.run !== null
    }, "fleet scope live with entities")

    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.enabled).toBe(true)
    expect(state.authState).toBe("connected")
    expect(state.phase).toBe("live")
    expect(state.run).toMatchObject({
      runId: RUN_ID,
      status: "running",
      desiredSlots: 3,
      workerKind: "codex",
    })
    expect(state.workers).toHaveLength(1)
    expect(state.workers[0]).toMatchObject({ workerId: "worker.slot.1", phase: "dispatched" })
    expect(state.assignments).toHaveLength(1)
    expect(state.assignments[0]).toMatchObject({ assignmentRef: "assignment.test.1", issueRef: "#8303" })
    expect(state.accounts).toHaveLength(1)
    expect(state.accounts[0]).toMatchObject({ readiness: "ready" })

    // Live delta: a new confirmed post-image lands without any re-poll of
    // the server REST surface.
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage({ desiredSlots: 5 }) },
    ])
    await waitFor(async () => {
      const next = await service.fleetState({ runId: RUN_ID })
      return next.run?.desiredSlots === 5
    }, "live delta visible")
    await service.close()
  })

  test("mutate: optimistic overlay first, server confirmation after", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    server.holdPushes = true
    const mutate = await service.fleetMutate({ action: "pause", runId: RUN_ID })
    expect(mutate.ok).toBe(true)

    // Optimistic: the view flips immediately while the push queue waits.
    const optimistic = await service.fleetState({ runId: RUN_ID })
    expect(optimistic.run?.status).toBe("paused")
    expect(optimistic.pendingMutations).toBe(1)

    // Server confirms: queue drains, confirmed post-image wins, no residue.
    server.holdPushes = false
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.pendingMutations === 0 && state.run?.status === "paused"
    }, "pause confirmed")
    expect(server.pushedMutations.some(m => m.name === FLEET_PAUSE_RUN_MUTATOR_NAME)).toBe(true)
    await service.close()
  })

  test("mutate set_desired_slots validates and applies", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    expect(
      await service.fleetMutate({ action: "set_desired_slots", runId: RUN_ID }),
    ).toEqual({ ok: false, error: "desired_slots_out_of_range" })
    expect(
      await service.fleetMutate({ action: "set_desired_slots", desiredSlots: 4096, runId: RUN_ID }),
    ).toEqual({ ok: false, error: "desired_slots_out_of_range" })

    const ok = await service.fleetMutate({
      action: "set_desired_slots",
      desiredSlots: 7,
      runId: RUN_ID,
    })
    expect(ok.ok).toBe(true)
    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.pendingMutations === 0 && state.run?.desiredSlots === 7
    }, "desired slots confirmed")
    await service.close()
  })

  test("in-band rejection: surfaced as state, queue drains, server truth wins", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    server.rejectPushes = true
    const mutate = await service.fleetMutate({ action: "pause", runId: RUN_ID })
    expect(mutate.ok).toBe(true) // queued optimistically; rejection is in-band

    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.rejections.length > 0 && state.pendingMutations === 0
    }, "rejection surfaced and queue drained")

    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.rejections[0]).toMatchObject({
      errorCode: "unauthorized_scope",
      mutatorName: FLEET_PAUSE_RUN_MUTATOR_NAME,
      runId: RUN_ID,
    })
    // No optimistic residue: the rejected pause vanished with the ack and
    // the confirmed status stays the server's.
    expect(state.run?.status).toBe("running")
    await service.close()
  })

  // Oracle khala_sync_must_refetch_rebootstraps.service for contract
  // khala_code.fleet.khala_sync_must_refetch_recovers.v1: MustRefetch never
  // strands the consumer — the session re-bootstraps on its own and the view
  // converges on the replaced scope content.
  test("MustRefetch: session re-bootstraps automatically and the view recovers", async () => {
    const { server, service } = makeHarness()
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")

    // Server-side scope reset: versions restart, contents replaced.
    server.replaceScope([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage({ desiredSlots: 9, status: "paused" }) },
    ])
    server.emitMustRefetch()

    await waitFor(async () => {
      const state = await service.fleetState({ runId: RUN_ID })
      return state.phase === "live" && state.run?.desiredSlots === 9
    }, "re-bootstrap recovered the replaced scope")
    const state = await service.fleetState({ runId: RUN_ID })
    expect(state.run?.status).toBe("paused")
    await service.close()
  })

  test("auth token propagation: transport reads the resolved token per request, rotation included", async () => {
    const harness = makeHarness()
    const { server, service, env } = harness
    server.commit([
      { entityType: "fleet_run", entityId: RUN_ID, postImageJson: fleetRunImage() },
    ])
    await waitFor(async () => (await service.fleetState({ runId: RUN_ID })).phase === "live", "live")
    expect(server.seenAuthTokens).toContain("oa_agent_test_token_1")
    expect(server.seenAuthTokens).not.toContain("")

    // Rotate the token: the next RPC entry re-resolves it and the transport
    // picks it up on the next request without a rebuilt session.
    env.OPENAGENTS_AGENT_TOKEN = "oa_agent_test_token_2"
    await service.fleetState({ runId: RUN_ID })
    await service.fleetMutate({ action: "resume", runId: RUN_ID })
    await waitFor(
      () => server.seenAuthTokens.includes("oa_agent_test_token_2"),
      "rotated token observed by transport",
    )
    await service.close()
  })
})
