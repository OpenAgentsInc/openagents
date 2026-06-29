import { describe, expect, test } from "bun:test"
import { Effect, Fiber } from "effect"

import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  WORLD_DELTA_SCHEMA_VERSION,
  decodeWorldCommandEnvelope,
  decodeWorldDelta,
  decodeWorldRow,
  decodeWorldSubscriptionPlan,
  type WorldCommandEnvelope,
  type WorldDelta,
  type WorldSubscriptionPlan,
} from "@openagentsinc/world-contract"

import {
  WorldClientError,
  applyDeltaToReadModel,
  applyDeltaToState,
  commandAckFromReceipt,
  createBrowserWorldTransport,
  createStubWorldClientTransport,
  createWorldClient,
  makeEmptyClientWorld,
  makeStubWorldDelta,
  makeStubWorldReadModel,
  projectWorldMinimapReadout,
  type WorldClientTransport,
} from "./index.js"

const regionRef = "region.run.1"
const observedAt = "2026-06-22T00:00:00.000Z"

const safety = {
  publicProjectionAllowed: true,
  sourceRefs: ["source.public.test"],
  blockerRefs: [],
  caveatRefs: [],
}

const plan = (selectedRefs: ReadonlyArray<string> = []): WorldSubscriptionPlan =>
  decodeWorldSubscriptionPlan({
    planRef: "plan.world.test",
    regionRef,
    scope: "region",
    interest: {
      center: { x: 0, y: 0, z: 0 },
      enterRadius: 90,
      dropRadius: 120,
      nearRadius: 32,
      farRadius: 120,
      selectedRefs,
    },
    nearUpdateMs: 100,
    farUpdateMs: 1000,
  })

const rowDelta = (rows: ReadonlyArray<unknown>, cursor: string, extra: Record<string, unknown> = {}): WorldDelta =>
  decodeWorldDelta({
    schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
    deltaRef: `delta.${cursor}`,
    kind: "update",
    regionRef,
    cursor,
    generatedAt: observedAt,
    rows,
    ...extra,
  })

const avatarRow = decodeWorldRow({
  kind: "agent_avatar",
  avatarRef: "avatar.alice",
  characterId: "alice",
  regionRef,
  label: "Alice",
  avatarKind: "guest",
  updatedAt: observedAt,
  safety,
})

const regionRow = decodeWorldRow({
  kind: "world_region",
  regionRef,
  label: "Tassadar Street",
  bounds: { min: { x: -50, y: -4, z: -50 }, max: { x: 50, y: 20, z: 50 } },
  origin: { x: 0, y: 0, z: 0 },
  proximityRadius: 12,
  staleAvatarTtlMs: 30_000,
  updatedAt: observedAt,
  safety,
})

const pylonRow = decodeWorldRow({
  kind: "pylon_station",
  pylonRef: "pylon.alpha",
  regionRef,
  label: "Alpha Pylon",
  position: { x: 25, y: 0, z: -25 },
  status: "working",
  updatedAt: observedAt,
  safety,
})

const settlementRow = decodeWorldRow({
  kind: "settlement_ref",
  settlementRef: "settlement.khala.1",
  runRef: "run.tassadar.executor.20260615",
  label: "Khala mini completion receipt",
  amountSats: 1,
  updatedAt: observedAt,
  safety,
})

const inferenceEventRow = decodeWorldRow({
  kind: "world_event",
  eventRef: "world_event.khala.inference.1",
  regionRef,
  runRef: "run.tassadar.executor.20260615",
  eventKind: "inference.completion",
  text: "Khala mini completion routed through the public proof feed.",
  createdAt: observedAt,
  sourceRefs: ["receipt.openagents.khala.1"],
  inference: {
    requestRef: "request.khala.1",
    receiptRef: "receipt.openagents.khala.1",
    model: "openagents/khala-mini",
    route: "khala/fireworks",
    workers: [
      {
        workerRef: "pylon.alpha",
        workerKind: "pylon",
        label: "Alpha Pylon",
        role: "scene relay",
        sourceRefs: ["pylon.alpha"],
      },
      {
        workerRef: "gateway.fireworks",
        workerKind: "gateway",
        label: "Fireworks Gateway",
        sourceRefs: ["gateway.fireworks"],
      },
    ],
    verification: "test_passed",
    costMsat: 125,
    priceMsat: 250,
    settled: true,
    sourceRefs: ["receipt.openagents.khala.1"],
  },
  safety,
})

const runRow = decodeWorldRow({
  kind: "training_run",
  runRef: "run.tassadar.executor.20260615",
  label: "Tassadar Executor",
  state: "tracing",
  updatedAt: observedAt,
  safety,
})

const assignmentRow = decodeWorldRow({
  kind: "run_entity",
  entityRef: "assignment.trace.1",
  runRef: "run.tassadar.executor.20260615",
  label: "Trace Batch 1",
  entityKind: "assignment",
  updatedAt: observedAt,
  safety,
})

const positionRow = (animation: "idle" | "walk" = "walk") => decodeWorldRow({
  kind: "avatar_position",
  avatarRef: "avatar.alice",
  regionRef,
  position: { x: 1, y: 0, z: 2 },
  rotationY: 0,
  animation,
  observedAt,
  safety,
})

describe("world-client", () => {
  test("applies deltas with absent-means-unchanged read-model semantics", () => {
    const base = applyDeltaToReadModel(
      makeEmptyClientWorld(regionRef, observedAt),
      rowDelta([avatarRow, positionRow()], "cursor.region.run.1.1"),
    )
    const next = applyDeltaToReadModel(
      base,
      rowDelta([], "cursor.region.run.1.2"),
    )

    expect(next.avatars["avatar.alice"]?.label).toBe("Alice")
    expect(next.positions["avatar.alice"]?.animation).toBe("walk")
    expect(String(next.cursor)).toBe("cursor.region.run.1.2")
  })

  test("projects minimap markers from the shared WorldReadModel", () => {
    const readModel = applyDeltaToReadModel(
      makeEmptyClientWorld(regionRef, observedAt),
      rowDelta(
        [regionRow, pylonRow, avatarRow, positionRow(), runRow, assignmentRow],
        "cursor.region.run.1.minimap",
      ),
    )
    const readout = projectWorldMinimapReadout({
      readModel,
      localPosition: { x: 25, y: 0, z: -25 },
      sizePx: 200,
    })

    expect(readout.regionLabel).toBe("Tassadar Street")
    expect(readout.coordinate).toEqual({ x: 25, y: 0, z: -25 })
    expect(readout.subzone.subzoneRef).toBe(`${regionRef}:east`)
    expect(readout.markers.map(marker => `${marker.kind}:${marker.ref}`)).toEqual([
      "assignment:assignment.trace.1",
      "avatar:avatar.alice",
      "pylon:pylon.alpha",
      "run_core:run.tassadar.executor.20260615",
    ])
    expect(readout.markers.find(marker => marker.ref === "pylon.alpha")).toMatchObject({
      minimap: { x: 150, y: 50 },
      state: "working",
      worldPosition: { x: 25, y: 0, z: -25 },
    })
    expect(readout.markers.find(marker => marker.ref === "avatar.alice")).toMatchObject({
      minimap: { x: 102, y: 104 },
      state: "walk",
      worldPosition: { x: 1, y: 0, z: 2 },
    })
    expect(readout.markers.find(marker => marker.kind === "run_core")).toMatchObject({
      minimap: { x: 100, y: 100 },
      state: "tracing",
    })
  })

  test("keeps subzone labels stable inside the hysteresis band", () => {
    const readModel = applyDeltaToReadModel(
      makeEmptyClientWorld(regionRef, observedAt),
      rowDelta([regionRow], "cursor.region.run.1.subzone"),
    )
    const stable = projectWorldMinimapReadout({
      readModel,
      localPosition: { x: -1, z: 1 },
      previousSubzoneRef: `${regionRef}:east`,
      subzoneHysteresisMeters: 4,
    })
    const crossed = projectWorldMinimapReadout({
      readModel,
      localPosition: { x: -6, z: 1 },
      previousSubzoneRef: stable.subzone.subzoneRef,
      subzoneHysteresisMeters: 4,
    })

    expect(stable.subzone.subzoneRef).toBe(`${regionRef}:east`)
    expect(crossed.subzone.subzoneRef).toBe(`${regionRef}:west`)
  })

  test("prunes interest exits from read model and selected-target state", () => {
    const readModel = applyDeltaToReadModel(
      makeEmptyClientWorld(regionRef, observedAt),
      rowDelta([avatarRow, positionRow()], "cursor.region.run.1.1"),
    )
    const state = {
      connected: true,
      regionRef,
      socketUrl: "wss://world.test/regions/region.run.1/socket",
      readModel,
      subscriptionPlan: plan(["avatar.alice"]),
      selectedRefs: ["avatar.alice"],
      interestTierByRef: { "avatar.alice": "near" as const },
      diagnostics: [],
      commandAcks: {},
    }
    const next = applyDeltaToState(
      state,
      rowDelta([], "cursor.region.run.1.2", {
        kind: "delete",
      deletedRefs: ["avatar.alice"],
      }),
    )

    expect(next.readModel.avatars["avatar.alice"]).toBeUndefined()
    expect(next.readModel.positions["avatar.alice"]).toBeUndefined()
    expect(next.selectedRefs).toEqual([])
    expect(next.interestTierByRef["avatar.alice"]).toBeUndefined()
  })

  test("applies settle patches without clearing position rows", () => {
    const readModel = applyDeltaToReadModel(
      makeEmptyClientWorld(regionRef, observedAt),
      rowDelta([avatarRow, positionRow("walk")], "cursor.region.run.1.1"),
    )
    const settled = applyDeltaToReadModel(
      readModel,
      rowDelta([], "cursor.region.run.1.2", {
        patches: [{ ref: "avatar.alice", movement: "settled" }],
      }),
    )

    expect(settled.positions["avatar.alice"]?.animation).toBe("idle")
    expect(Number(settled.positions["avatar.alice"]?.position.x)).toBe(1)
  })

  test("retains selected-target promotion state across ordinary deltas", () => {
    const state = {
      connected: true,
      regionRef,
      socketUrl: "wss://world.test/regions/region.run.1/socket",
      readModel: makeEmptyClientWorld(regionRef, observedAt),
      subscriptionPlan: plan(["avatar.alice"]),
      selectedRefs: ["avatar.alice"],
      interestTierByRef: { "avatar.alice": "near" as const },
      diagnostics: [],
      commandAcks: {},
    }
    const next = applyDeltaToState(state, rowDelta([avatarRow], "cursor.region.run.1.1"))

    expect(next.selectedRefs).toEqual(["avatar.alice"])
    expect(next.interestTierByRef["avatar.alice"]).toBe("near")
  })

  test("connect, subscribe, reconnect, diagnostics, and disconnect use fake transport effects", async () => {
    const staleDiagnostic = decodeWorldDelta({
      schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
      deltaRef: "delta.cursor.stale",
      kind: "diagnostic",
      regionRef,
      cursor: "cursor.region.run.1.0",
      generatedAt: observedAt,
      diagnostic: {
        diagnosticRef: "diagnostic.cursor.stale",
        tag: "cursor",
        severity: "warn",
        message: "Stale cursor; fresh snapshot required.",
        observedAt,
        sourceRefs: ["cursor.region.run.1.999"],
      },
    })
    const calls: Array<string> = []
    const transport: WorldClientTransport = {
      connect: (request: Parameters<WorldClientTransport["connect"]>[0]) => Effect.sync(() => {
        calls.push(`connect:${request.resumeCursor ?? "fresh"}`)
        return {
          regionRef,
          socketUrl: "wss://world.test/regions/region.run.1/socket",
          subscriptionPlan: plan(["avatar.alice"]),
          deltas: request.resumeCursor === "cursor.region.run.1.999"
            ? [staleDiagnostic]
            : [rowDelta([avatarRow], request.resumeCursor ?? "cursor.region.run.1.1")],
        }
      }),
      command: () => Effect.fail(new WorldClientError({
        phase: "command",
        reason: "unused",
        retryable: false,
        sourceRefs: ["test"],
      })),
      disconnect: () => Effect.sync(() => {
        calls.push("disconnect")
      }),
    }
    const client = createWorldClient({ transport, initialRegionRef: regionRef, now: () => observedAt })
    const connected = await Effect.runPromise(client.connect())
    const subscribed = await Effect.runPromise(client.subscribe({ selectedRefs: ["avatar.alice"] }))
    await Effect.runPromise(client.applyDelta(rowDelta([], "cursor.region.run.1.999")))
    const reconnected = await Effect.runPromise(client.reconnect())
    await Effect.runPromise(client.disconnect())

    expect(connected.readModel.avatars["avatar.alice"]?.label).toBe("Alice")
    expect(subscribed.interest.selectedRefs.map(String)).toEqual(["avatar.alice"])
    expect(reconnected.diagnostics[0]?.tag).toBe("cursor")
    expect((await Effect.runPromise(client.state())).connected).toBe(false)
    expect(calls).toContain("connect:fresh")
    expect(calls).toContain("connect:cursor.region.run.1.1")
    expect(calls).toContain("connect:cursor.region.run.1.999")
    expect(calls).toContain("disconnect")
  })

  test("browser transport closes an opening socket when connect is interrupted", async () => {
    class InterruptibleSocket {
      static readonly instances: Array<InterruptibleSocket> = []

      readonly listeners = new Map<string, Set<(event: unknown) => void>>()
      readonly sent: Array<string> = []
      readonly url: string
      readyState = 0
      closed = false

      constructor(url: string) {
        this.url = url
        InterruptibleSocket.instances.push(this)
      }

      send(data: string): void {
        this.sent.push(data)
      }

      close(): void {
        this.closed = true
        this.readyState = 3
      }

      addEventListener(type: string, listener: (event: unknown) => void): void {
        const listeners = this.listeners.get(type) ?? new Set<(event: unknown) => void>()
        listeners.add(listener)
        this.listeners.set(type, listeners)
      }

      removeEventListener(type: string, listener: (event: unknown) => void): void {
        this.listeners.get(type)?.delete(listener)
      }
    }

    const fetchFn = (async () =>
      new Response(JSON.stringify({
        ok: true,
        schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
        regionRef,
        socketUrl: "wss://world.test/regions/region.run.1/socket",
        subscriptionPlan: plan(),
      }), {
        status: 200,
        headers: { "content-type": "application/json" },
      })) as unknown as typeof fetch

    const transport = createBrowserWorldTransport({
      worldUrl: "https://world.test",
      actorRef: "actor.alice",
      fetchFn,
      webSocketCtor: InterruptibleSocket,
    })

    const fiber = Effect.runFork(transport.connect({}))
    for (let attempt = 0; attempt < 10 && InterruptibleSocket.instances.length === 0; attempt += 1) {
      await Effect.runPromise(Effect.yieldNow)
    }

    expect(InterruptibleSocket.instances).toHaveLength(1)
    expect(InterruptibleSocket.instances[0]?.closed).toBe(false)

    await Effect.runPromise(Fiber.interrupt(fiber))

    expect(InterruptibleSocket.instances[0]?.closed).toBe(true)
    expect(InterruptibleSocket.instances[0]?.listeners.get("open")?.size ?? 0).toBe(0)
    expect(InterruptibleSocket.instances[0]?.listeners.get("error")?.size ?? 0).toBe(0)
  })

  test("callCommand exposes receipt sequence ack state", async () => {
    const command: WorldCommandEnvelope = decodeWorldCommandEnvelope({
      schemaVersion: "openagents.world_contract.v1",
      commandRef: "command.move.1",
      command: "set_avatar_position",
      actorClass: "browser",
      actorRef: "actor.alice",
      regionRef,
      seq: 7,
      issuedAt: observedAt,
      payload: {},
    })
    const delta = rowDelta([positionRow()], "cursor.region.run.1.7", {
      receipt: {
        receiptRef: "receipt.move.1",
        commandRef: command.commandRef,
        command: command.command,
        status: "applied",
        actorClass: command.actorClass,
        acceptedSeq: 7,
        appliedSeq: 7,
        observedAt,
        changedRefs: ["avatar.alice"],
      },
    })
    const transport: WorldClientTransport = {
      connect: () => Effect.sync(() => ({
        regionRef,
        socketUrl: "wss://world.test/regions/region.run.1/socket",
        subscriptionPlan: plan(),
      })),
      command: () => Effect.succeed(delta),
      disconnect: () => Effect.void,
    }
    const client = createWorldClient({ transport, initialRegionRef: regionRef, now: () => observedAt })
    const ack = await Effect.runPromise(client.callCommand(command))
    const state = await Effect.runPromise(client.state())

    expect(ack.status).toBe("applied")
    expect(ack.acceptedSeq).toBe(7)
    expect(ack.appliedSeq).toBe(7)
    expect(state.commandAcks["command.move.1"]?.appliedSeq).toBe(7)
    expect(commandAckFromReceipt(delta.receipt!).rejectedSeq).toBeUndefined()
  })

  test("stub read-model fixtures hydrate pylon, payment, and inference proof rows", () => {
    const readModel = makeStubWorldReadModel({
      regionRef,
      generatedAt: observedAt,
      rows: [regionRow, pylonRow, runRow, settlementRow, inferenceEventRow],
    })

    expect(readModel.pylons["pylon.alpha"]?.status).toBe("working")
    expect(readModel.settlementRefs["settlement.khala.1"]?.amountSats).toBe(1)
    expect(readModel.events["world_event.khala.inference.1"]?.inference).toMatchObject({
      model: "openagents/khala-mini",
      route: "khala/fireworks",
      verification: "test_passed",
      settled: true,
    })
  })

  test("stub transport feeds fixture deltas through createWorldClient without backend services", async () => {
    const transport = createStubWorldClientTransport({
      regionRef,
      generatedAt: observedAt,
      rows: [regionRow, pylonRow, runRow, settlementRow, inferenceEventRow],
      selectedRefs: ["pylon.alpha", "world_event.khala.inference.1"],
    })
    const client = createWorldClient({ transport, initialRegionRef: regionRef, now: () => observedAt })
    const state = await Effect.runPromise(client.connect())
    const plan = await Effect.runPromise(client.subscribe({
      selectedRefs: ["settlement.khala.1"],
    }))

    expect(state.socketUrl).toBe(`stub://world-client/${regionRef}`)
    expect(state.readModel.pylons["pylon.alpha"]?.label).toBe("Alpha Pylon")
    expect(state.readModel.settlementRefs["settlement.khala.1"]?.label).toBe(
      "Khala mini completion receipt",
    )
    expect(state.readModel.events["world_event.khala.inference.1"]?.inference?.priceMsat).toBe(250)
    expect(plan.interest.selectedRefs.map(String)).toEqual(["settlement.khala.1"])
  })

  test("stub transport command handlers can append contract-checked fixture deltas", async () => {
    const command: WorldCommandEnvelope = decodeWorldCommandEnvelope({
      schemaVersion: "openagents.world_contract.v1",
      commandRef: "command.focus.pylon.1",
      command: "focus_pylon",
      actorClass: "browser",
      actorRef: "actor.alice",
      regionRef,
      seq: 9,
      issuedAt: observedAt,
      payload: { pylonRef: "pylon.alpha" },
    })
    const transport = createStubWorldClientTransport({
      regionRef,
      generatedAt: observedAt,
      rows: [regionRow, pylonRow],
      onCommand: (received, context) =>
        makeStubWorldDelta({
          regionRef,
          cursor: `cursor.${regionRef}.stub.command.${context.sequence}`,
          generatedAt: observedAt,
          rows: [
            {
              kind: "agent_intent",
              intentRef: "intent.focus.pylon.1",
              avatarRef: "avatar.alice",
              regionRef,
              intent: "focus_pylon:pylon.alpha",
              targetRef: "pylon.alpha",
              createdAt: observedAt,
              expiresAt: "2026-06-22T00:00:05.000Z",
              safety,
            },
          ],
          receipt: {
            receiptRef: "receipt.focus.pylon.1",
            commandRef: received.commandRef,
            command: received.command,
            status: "applied",
            actorClass: received.actorClass,
            acceptedSeq: context.sequence,
            appliedSeq: context.sequence,
            observedAt,
            changedRefs: ["intent.focus.pylon.1"],
          },
        }),
    })
    const client = createWorldClient({ transport, initialRegionRef: regionRef, now: () => observedAt })
    const ack = await Effect.runPromise(client.callCommand(command))
    const readModel = await Effect.runPromise(client.readModel())

    expect(ack.status).toBe("applied")
    expect(String(readModel.intents["intent.focus.pylon.1"]?.targetRef)).toBe("pylon.alpha")
  })

  // durable-stream Rank-2 (#6059): when the Region DO serves an in-window
  // reconnect as TRUE gap replay, the connect-result carries the exact missing
  // delta suffix (NOT a fresh snapshot). The client must resume from its cursor
  // and apply every gap delta into the WorldReadModel in order.
  test("resumes from its cursor and applies a gap-replay delta suffix into the read model (no re-snapshot)", async () => {
    const avatarAt = (seq: number, label: string) =>
      decodeWorldRow({
        kind: "agent_avatar",
        avatarRef: `avatar.gap.${seq}`,
        characterId: `char.${seq}`,
        regionRef,
        label,
        avatarKind: "human",
        updatedAt: observedAt,
        safety,
      })

    let resumeCursorSeen: string | undefined
    const transport: WorldClientTransport = {
      // First connect: a snapshot establishing the client at sequence 2.
      // Reconnect from `cursor.region.run.1.2`: gap replay of sequences 3 and 4
      // as plain delta payloads — the server never re-snapshots.
      connect: (request) =>
        Effect.sync(() => {
          resumeCursorSeen = request.resumeCursor
          if (request.resumeCursor === undefined) {
            return {
              regionRef,
              socketUrl: "wss://world.test/regions/region.run.1/socket",
              subscriptionPlan: plan(),
              deltas: [
                makeStubWorldDelta({
                  regionRef,
                  cursor: "cursor.region.run.1.2",
                  generatedAt: observedAt,
                  kind: "snapshot",
                  rows: [avatarAt(2, "Avatar 2")],
                }),
              ],
            }
          }
          return {
            regionRef,
            socketUrl: "wss://world.test/regions/region.run.1/socket",
            subscriptionPlan: plan(),
            deltas: [
              rowDelta([avatarAt(3, "Avatar 3")], "cursor.region.run.1.3"),
              rowDelta([avatarAt(4, "Avatar 4")], "cursor.region.run.1.4"),
            ],
          }
        }),
      command: () =>
        Effect.fail(new WorldClientError({ phase: "command", reason: "unused", retryable: false, sourceRefs: ["test"] })),
      disconnect: () => Effect.void,
    }

    const client = createWorldClient({ transport, initialRegionRef: regionRef, now: () => observedAt })
    const connected = await Effect.runPromise(client.connect())
    expect(connected.readModel.avatars["avatar.gap.2"]?.label).toBe("Avatar 2")
    expect(String(connected.readModel.cursor)).toBe("cursor.region.run.1.2")

    const reconnected = await Effect.runPromise(client.reconnect())

    // The client resumed from the cursor it had applied through.
    expect(resumeCursorSeen).toBe("cursor.region.run.1.2")
    // Every gap delta landed; the pre-gap avatar was preserved (additive, not a
    // replacing snapshot).
    expect(reconnected.readModel.avatars["avatar.gap.2"]?.label).toBe("Avatar 2")
    expect(reconnected.readModel.avatars["avatar.gap.3"]?.label).toBe("Avatar 3")
    expect(reconnected.readModel.avatars["avatar.gap.4"]?.label).toBe("Avatar 4")
    // The read model advanced to the live tail via the last applied delta.
    expect(String(reconnected.readModel.cursor)).toBe("cursor.region.run.1.4")
  })
})
