import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
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
  createWorldClient,
  makeEmptyClientWorld,
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
})
