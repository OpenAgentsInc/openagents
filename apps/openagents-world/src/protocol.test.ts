import { describe, expect, test } from "bun:test"

import { decodeWorldRow } from "@openagentsinc/world-contract"

import {
  DEFAULT_HANDSHAKE_BUFFER_LIMIT,
  WorldWireCache,
  bufferHandshakeFrame,
  configFromEnv,
  cursorForSequence,
  enqueueTransportFrame,
  hydrateBufferedSession,
  makeDiagnosticResponse,
  makeHeartbeatFrame,
  makeInitialSessionAttachment,
  makeReconnectPlan,
  makeSnapshotFrame,
  makeZeroSnapshotDelta,
  normalizeRegionRef,
  planSightDelta,
  regionDurableObjectMigrationStatements,
  regionRefFromSocketPath,
  socketUrlForRegion,
} from "./protocol"

describe("openagents-world protocol helpers", () => {
  test("normalizes unsafe region refs into bounded Durable Object names", () => {
    expect(normalizeRegionRef(" region /run/alpha ")).toBe("region-run-alpha")
    expect(normalizeRegionRef("")).toBe("region.run.tassadar.executor.20260615.street")
  })

  test("builds config from Cloudflare string vars", () => {
    const config = configFromEnv({
      OPENAGENTS_WORLD_ENV: "staging",
      OPENAGENTS_WORLD_MAX_HANDSHAKE_BUFFER: "128",
    })

    expect(config.envName).toBe("staging")
    expect(config.maxHandshakeBuffer).toBe(64)
    expect(config.schemaVersion).toBe("openagents.world_contract.v1")
  })

  test("detects socket routes and produces websocket URLs", () => {
    const request = new Request("https://world.openagents.com/connect?region=ignored")
    expect(regionRefFromSocketPath("/regions/region.run.1/socket")).toBe("region.run.1")
    expect(socketUrlForRegion(request, "region.run.1")).toBe("wss://world.openagents.com/regions/region.run.1/socket")
  })

  test("produces a typed zero-row snapshot delta", () => {
    const delta = makeZeroSnapshotDelta("region.run.1", "2026-06-22T00:00:00.000Z")

    expect(delta.schemaVersion).toBe("openagents.world_delta.v1")
    expect(delta.kind).toBe("snapshot")
    expect(delta.rows).toEqual([])
  })

  test("produces typed snapshot envelopes with server-owned read models", () => {
    const region = decodeWorldRow({
      kind: "world_region",
      regionRef: "region.run.1",
      label: "Run 1",
      bounds: {
        min: { x: -10, y: 0, z: -10 },
        max: { x: 10, y: 10, z: 10 },
      },
      origin: { x: 0, y: 0, z: 0 },
      proximityRadius: 24,
      staleAvatarTtlMs: 30000,
      updatedAt: "2026-06-22T00:00:00.000Z",
      safety: {
        publicProjectionAllowed: true,
        sourceRefs: ["source.public.test"],
        blockerRefs: [],
        caveatRefs: [],
      },
    })
    const frame = makeSnapshotFrame(
      "region.run.1",
      "2026-06-22T00:00:00.000Z",
      [region],
      cursorForSequence("region.run.1", 7),
    )

    expect(frame.frameKind).toBe("snapshot")
    if (frame.frameKind !== "snapshot") {
      throw new Error("expected snapshot frame")
    }
    expect(frame.delta.kind).toBe("snapshot")
    expect(frame.readModel.regions["region.run.1"]?.label).toBe("Run 1")
  })

  test("produces typed heartbeat delta frames", () => {
    const frame = makeHeartbeatFrame(
      "region.run.1",
      cursorForSequence("region.run.1", 3),
      "2026-06-22T00:00:00.000Z",
    )

    expect(frame.frameKind).toBe("delta")
    expect(frame.delta.kind).toBe("heartbeat")
    expect(String(frame.delta.cursor)).toBe("cursor.region.run.1.3")
  })

  test("plans reconnect resume, stale cursor diagnostic, and fresh snapshot fallback", () => {
    const valid = makeReconnectPlan(
      "region.run.1",
      "cursor.region.run.1.2",
      { currentSeq: 4, minReplaySeq: 1 },
      "2026-06-22T00:00:00.000Z",
    )
    const stale = makeReconnectPlan(
      "region.run.1",
      "cursor.region.run.1.0",
      { currentSeq: 4, minReplaySeq: 1 },
      "2026-06-22T00:00:00.000Z",
    )
    const fresh = makeReconnectPlan(
      "region.run.1",
      null,
      { currentSeq: 4, minReplaySeq: 1 },
      "2026-06-22T00:00:00.000Z",
    )

    expect(valid.kind).toBe("resume")
    expect(valid.frames[0]?.delta.kind).toBe("heartbeat")
    expect(stale.kind).toBe("fresh_snapshot")
    expect(stale.frames[0]?.frameKind).toBe("diagnostic")
    expect(fresh.frames[0]?.frameKind).toBe("snapshot")
  })

  test("plans sparse sight deltas where absent refs mean unchanged and leaving interest prunes", () => {
    const first = planSightDelta([], ["avatar.a", "pylon.a"])
    const second = planSightDelta(first.nextSeenRefs, ["avatar.a"])
    const reenter = planSightDelta(second.nextSeenRefs, ["avatar.a", "pylon.a"])

    expect(first.fullRefs).toEqual(["avatar.a", "pylon.a"])
    expect(second.liteRefs).toEqual(["avatar.a"])
    expect(second.prunedRefs).toEqual(["pylon.a"])
    expect(reenter.fullRefs).toEqual(["pylon.a"])
  })

  test("reuses per-entity wire encodings for the same cursor", () => {
    const row = decodeWorldRow({
      kind: "world_region",
      regionRef: "region.run.1",
      label: "Run 1",
      bounds: {
        min: { x: -10, y: 0, z: -10 },
        max: { x: 10, y: 10, z: 10 },
      },
      origin: { x: 0, y: 0, z: 0 },
      proximityRadius: 24,
      staleAvatarTtlMs: 30000,
      updatedAt: "2026-06-22T00:00:00.000Z",
      safety: {
        publicProjectionAllowed: true,
        sourceRefs: ["source.public.test"],
        blockerRefs: [],
        caveatRefs: [],
      },
    })
    const cache = new WorldWireCache()
    const first = cache.encodeEntity(row, "cursor.region.run.1.1")
    const second = cache.encodeEntity(row, "cursor.region.run.1.1")

    expect(second).toBe(first)
    expect(cache.size).toBe(1)
  })

  test("bounds session queues and reports public-safe backpressure diagnostics", () => {
    const attachment = {
      ...makeInitialSessionAttachment({
        regionRef: "region.run.1",
        connectedAt: "2026-06-22T00:00:00.000Z",
      }),
      queuedFrames: ["frame"],
    }
    const frame = makeHeartbeatFrame(
      "region.run.1",
      "cursor.region.run.1.1",
      "2026-06-22T00:00:00.000Z",
    )
    const decision = enqueueTransportFrame(attachment, frame, { maxQueuedFrames: 1 })

    expect(decision.kind).toBe("disconnect")
    if (decision.kind === "disconnect") {
      expect(decision.diagnostic.sourceRefs.map(String)).toEqual([attachment.sessionRef])
      expect(decision.diagnostic.severity).toBe("warn")
    }
  })

  test("buffers frames until hydration and then clears them", () => {
    const attachment = makeInitialSessionAttachment({
      regionRef: "region.run.1",
      connectedAt: "2026-06-22T00:00:00.000Z",
    })
    const buffered = bufferHandshakeFrame(attachment, "{\"kind\":\"join\"}", DEFAULT_HANDSHAKE_BUFFER_LIMIT)

    expect(buffered.ok).toBe(true)
    if (buffered.ok) {
      expect(buffered.attachment.bufferedFrames).toHaveLength(1)
      expect(hydrateBufferedSession(buffered.attachment).bufferedFrames).toHaveLength(0)
    }
  })

  test("rejects frames when the bounded handshake buffer is full", () => {
    const attachment = {
      ...makeInitialSessionAttachment({
        regionRef: "region.run.1",
        connectedAt: "2026-06-22T00:00:00.000Z",
      }),
      bufferedFrames: ["a"],
    }
    const result = bufferHandshakeFrame(attachment, "b", 1)

    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(result.diagnostic.tag).toBe("auth")
      expect(result.diagnostic.severity).toBe("warn")
    }
  })

  test("keeps Durable Object SQLite migrations explicit", () => {
    expect(regionDurableObjectMigrationStatements.join("\n")).toContain("_sql_schema_migrations")
    expect(regionDurableObjectMigrationStatements.join("\n")).toContain("region_socket_sessions")
    expect(regionDurableObjectMigrationStatements.join("\n")).toContain("region_projection_checkpoints")
  })

  test("returns typed diagnostics for plain HTTP socket requests", async () => {
    const response = makeDiagnosticResponse(426, {
      tag: "validation",
      severity: "warn",
      message: "World region socket route requires a WebSocket upgrade.",
      observedAt: "2026-06-22T00:00:00.000Z",
      sourceRefs: ["region.run.1"],
    })
    const payload = await response.json() as { ok: boolean; diagnostic: { tag: string } }

    expect(response.status).toBe(426)
    expect(payload.ok).toBe(false)
    expect(payload.diagnostic.tag).toBe("validation")
  })
})
