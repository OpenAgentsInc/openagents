import { describe, expect, test } from "bun:test"

import {
  DEFAULT_HANDSHAKE_BUFFER_LIMIT,
  bufferHandshakeFrame,
  configFromEnv,
  hydrateBufferedSession,
  makeDiagnosticResponse,
  makeInitialSessionAttachment,
  makeZeroSnapshotDelta,
  normalizeRegionRef,
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
