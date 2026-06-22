import { describe, expect, test } from "bun:test"

import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  WORLD_DELTA_SCHEMA_VERSION,
  WORLD_READ_MODEL_SCHEMA_VERSION,
  WorldCommandEnvelope,
  WorldDelta,
  WorldReadModel,
  assertWorldCommandActorAllowed,
  assertWorldPublicSafety,
  browserWorldCommandNames,
  decodeWorldCommandEnvelope,
  decodeWorldCommandReceipt,
  decodeWorldDelta,
  decodeWorldReadModel,
  decodeWorldRow,
  decodeWorldSubscriptionPlan,
  deterministicWorldEventRef,
  isBrowserWorldCommand,
  isServiceWorldCommand,
  sanitizeWorldCharacterId,
  serviceWorldCommandNames,
  sparseWorldPatchChangesOnly,
  worldActorClasses,
  worldAvatarRefForCharacter,
  worldErrorTags,
  worldRowKey,
  worldRowKinds,
  worldVectorInsideBounds,
} from "./index.js"

const safety = {
  publicProjectionAllowed: true,
  sourceRefs: ["github:OpenAgentsInc/openagents#5960"],
  blockerRefs: [],
  caveatRefs: [],
}

const now = "2026-06-22T00:00:00.000Z"

const bounds = {
  min: { x: -160, y: 0, z: -160 },
  max: { x: 160, y: 40, z: 160 },
}

const regionRow = {
  kind: "world_region",
  regionRef: "region.run.tassadar.street",
  label: "Tassadar Street",
  bounds,
  origin: { x: 0, y: 0, z: 0 },
  proximityRadius: 28,
  staleAvatarTtlMs: 30_000,
  updatedAt: now,
  safety,
}

const avatarRow = {
  kind: "agent_avatar",
  avatarRef: "avatar.account.main",
  accountRef: "account.public.main",
  characterId: "main",
  regionRef: "region.run.tassadar.street",
  label: "Main",
  avatarKind: "human",
  updatedAt: now,
  safety,
}

const positionRow = {
  kind: "avatar_position",
  avatarRef: "avatar.account.main",
  regionRef: "region.run.tassadar.street",
  position: { x: 1, y: 0, z: 2 },
  rotationY: 0,
  animation: "walk",
  observedAt: now,
  seq: 7,
  safety,
}

const gatewayRow = {
  kind: "gateway_station",
  gatewayRef: "gateway.vertex.primary",
  regionRef: "region.run.tassadar.street",
  lane: "vertex",
  label: "Vertex Gateway",
  providerLabel: "Vertex Gemini",
  position: { x: 12, y: 0, z: 18 },
  status: "online",
  updatedAt: now,
  safety,
}

describe("@openagentsinc/world-contract", () => {
  test("decodes every actor class, error tag, row kind, and command partition", () => {
    expect(worldActorClasses).toEqual(["browser", "agent", "service", "operator"])
    expect(worldErrorTags).toEqual([
      "validation",
      "auth",
      "redaction",
      "command",
      "storage",
      "cursor",
      "bridge",
    ])
    expect(worldRowKinds).toHaveLength(17)
    expect(worldRowKinds).toContain("gateway_station")
    expect(browserWorldCommandNames.every(isBrowserWorldCommand)).toBe(true)
    expect(serviceWorldCommandNames.every(isServiceWorldCommand)).toBe(true)
    expect(serviceWorldCommandNames).toContain("upsert_gateway_station")
    expect(browserWorldCommandNames.some(isServiceWorldCommand)).toBe(false)
  })

  test("sanitizes character ids and derives stable avatar refs", () => {
    expect(sanitizeWorldCharacterId(" Main Character!! ")).toBe("main-character")
    expect(sanitizeWorldCharacterId("")).toBe("guest")
    expect(sanitizeWorldCharacterId("x".repeat(80))).toHaveLength(48)
    expect(worldAvatarRefForCharacter("Account.Public/Main", "Mäin Character!!"))
      .toBe("avatar.account-public-main.m-in-character")
  })

  test("validates region bounds and row keys", () => {
    const decodedRegion = decodeWorldRow(regionRow)
    const decodedGateway = decodeWorldRow(gatewayRow)
    const decodedAvatar = decodeWorldRow(avatarRow)
    const decodedPosition = decodeWorldRow(positionRow)
    const outOfBoundsPosition = decodeWorldRow({
      ...positionRow,
      position: { x: 200, y: 1, z: 0 },
    })
    if (decodedRegion.kind !== "world_region") throw new Error("expected region")
    if (decodedPosition.kind !== "avatar_position") throw new Error("expected position")
    if (outOfBoundsPosition.kind !== "avatar_position") throw new Error("expected position")
    expect(worldVectorInsideBounds(decodedPosition.position, decodedRegion.bounds)).toBe(true)
    expect(worldVectorInsideBounds(outOfBoundsPosition.position, decodedRegion.bounds)).toBe(false)

    expect(worldRowKey(decodedRegion)).toBe("region.run.tassadar.street")
    expect(worldRowKey(decodedGateway)).toBe("gateway.vertex.primary")
    expect(worldRowKey(decodedAvatar)).toBe("avatar.account.main")
    expect(worldRowKey(decodedPosition)).toBe("avatar.account.main")
  })

  test("requires public source refs and rejects raw/private material in rows", () => {
    expect(assertWorldPublicSafety(decodeWorldRow(regionRow))).toMatchObject({
      kind: "world_region",
    })
    expect(() =>
      assertWorldPublicSafety(decodeWorldRow({
        ...regionRow,
        safety: { ...safety, sourceRefs: [] },
      })),
    ).toThrow("requires at least one public source ref")
    expect(() =>
      assertWorldPublicSafety(decodeWorldRow({
        ...regionRow,
        label: "raw_prompt: /Users/example/private",
      })),
    ).toThrow("raw/private material")
  })

  test("decodes command envelopes and enforces actor classes", () => {
    const browserCommand = decodeWorldCommandEnvelope({
      schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
      commandRef: "command.test.pose.1",
      command: "set_avatar_position",
      actorClass: "browser",
      actorRef: "account.public.main",
      regionRef: "region.run.tassadar.street",
      seq: 41,
      issuedAt: now,
      payload: { position: { x: 1, y: 0, z: 2 } },
    })
    expect(assertWorldCommandActorAllowed(browserCommand)).toBe(browserCommand)

    const serviceCommand = decodeWorldCommandEnvelope({
      schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
      commandRef: "command.test.bridge.1",
      command: "upsert_gateway_station",
      actorClass: "service",
      actorRef: "service.world.bridge",
      seq: 5,
      issuedAt: now,
      payload: { row: gatewayRow },
    })
    expect(assertWorldCommandActorAllowed(serviceCommand)).toBe(serviceCommand)

    expect(() =>
      assertWorldCommandActorAllowed(WorldCommandEnvelope.make({
        ...serviceCommand,
        actorClass: "browser",
      })),
    ).toThrow("service-only command")
  })

  test("decodes command receipts with accepted/applied/rejected sequence echoes", () => {
    expect(decodeWorldCommandReceipt({
      receiptRef: "receipt.command.test.pose.1",
      commandRef: "command.test.pose.1",
      command: "set_avatar_position",
      status: "applied",
      actorClass: "browser",
      acceptedSeq: 11,
      appliedSeq: 11,
      observedAt: now,
      changedRefs: ["avatar.account.main"],
    })).toMatchObject({ acceptedSeq: 11, appliedSeq: 11 })

    expect(decodeWorldCommandReceipt({
      receiptRef: "receipt.command.test.pose.2",
      commandRef: "command.test.pose.2",
      command: "set_avatar_position",
      status: "rejected",
      actorClass: "browser",
      rejectedSeq: 12,
      observedAt: now,
      changedRefs: [],
      error: {
        tag: "validation",
        message: "outside_region_bounds",
        retryable: false,
        publicSafe: true,
        sourceRefs: ["github:OpenAgentsInc/openagents#5960"],
      },
    })).toMatchObject({ rejectedSeq: 12, status: "rejected" })
  })

  test("decodes subscription plans with hysteresis, selected targets, tiers, and resume cursors", () => {
    const plan = decodeWorldSubscriptionPlan({
      planRef: "subscription.region.tassadar.main",
      regionRef: "region.run.tassadar.street",
      scope: "region",
      interest: {
        center: { x: 0, y: 0, z: 0 },
        enterRadius: 32,
        dropRadius: 40,
        nearRadius: 32,
        farRadius: 120,
        selectedRefs: ["avatar.account.main"],
      },
      nearUpdateMs: 100,
      farUpdateMs: 1000,
      resumeCursor: "cursor.region.tassadar.10",
    })
    expect(plan.interest.enterRadius).toBeLessThan(plan.interest.dropRadius)
    expect(plan.interest.selectedRefs.map(String)).toEqual(["avatar.account.main"])
    expect(String(plan.resumeCursor)).toBe("cursor.region.tassadar.10")
  })

  test("decodes sparse deltas and read models without backend transport fields", () => {
    const delta = decodeWorldDelta({
      schemaVersion: WORLD_DELTA_SCHEMA_VERSION,
      deltaRef: "delta.region.tassadar.1",
      kind: "snapshot",
      regionRef: "region.run.tassadar.street",
      cursor: "cursor.region.tassadar.1",
      generatedAt: now,
      rows: [regionRow, gatewayRow, avatarRow, positionRow],
    })
    expect(delta.rows?.map((row) => row.kind)).toEqual([
      "world_region",
      "gateway_station",
      "agent_avatar",
      "avatar_position",
    ])

    expect(sparseWorldPatchChangesOnly({ x: 1 })).toBe(true)
    expect(sparseWorldPatchChangesOnly({ x: undefined })).toBe(false)

    const readModel = decodeWorldReadModel({
      schemaVersion: WORLD_READ_MODEL_SCHEMA_VERSION,
      regionRef: "region.run.tassadar.street",
      cursor: "cursor.region.tassadar.1",
      generatedAt: now,
      regions: { "region.run.tassadar.street": regionRow },
      pylons: {},
      gateways: { "gateway.vertex.primary": gatewayRow },
      avatars: { "avatar.account.main": avatarRow },
      positions: { "avatar.account.main": positionRow },
      chatMessages: {},
      chatBubbles: {},
      emotes: {},
      intents: {},
      runs: {},
      entities: {},
      edges: {},
      proofRefs: {},
      settlementRefs: {},
      events: {},
      diagnostics: [],
    })
    expect(readModel.gateways["gateway.vertex.primary"]?.providerLabel).toBe("Vertex Gemini")
    expect(readModel.avatars["avatar.account.main"]?.label).toBe("Main")

    const schemaText = JSON.stringify([WorldDelta.ast, WorldReadModel.ast])
    expect(schemaText).not.toContain("backend runtime")
    expect(schemaText).not.toContain("DbConnection")
    expect(schemaText).not.toContain("WebSocket")
    expect(schemaText).not.toContain("DurableObject")
  })

  test("creates deterministic world event refs from public source coordinates", () => {
    expect(deterministicWorldEventRef("github:OpenAgentsInc/openagents#5960", "proof accepted", 7))
      .toBe("world_event.github-openagentsinc-openagents-5960.proof-accepted.7")
  })

  test("decodes Khala inference world event payloads without exposing private internals", () => {
    const event = decodeWorldRow({
      kind: "world_event",
      eventRef: "world_event.khala.request.1",
      regionRef: "region.run.tassadar.street",
      eventKind: "khala_inference_served",
      text: "openagents/khala-mini served via cheap",
      createdAt: now,
      sourceRefs: ["https://openagents.com/api/public/inference/receipts/oa_receipt_1"],
      inference: {
        requestRef: "request.khala.1",
        receiptRef: "https://openagents.com/api/public/inference/receipts/oa_receipt_1",
        model: "openagents/khala-mini",
        route: "cheap",
        workers: [
          {
            workerRef: "gateway.vertex.primary",
            workerKind: "gateway",
            label: "Vertex Gemini",
            role: "worker",
            sourceRefs: ["https://openagents.com/api/public/inference/receipts/oa_receipt_1"],
          },
          {
            workerRef: "worker.validator.primary",
            workerKind: "verifier",
            label: "Validator",
            role: "verify",
            sourceRefs: ["https://openagents.com/api/public/inference/receipts/oa_receipt_1"],
          },
        ],
        verification: "none",
        costMsat: 123,
        priceMsat: 170,
        settled: false,
        sourceRefs: ["https://openagents.com/api/public/inference/receipts/oa_receipt_1"],
      },
      safety: {
        publicProjectionAllowed: true,
        sourceRefs: ["https://openagents.com/api/public/inference/receipts/oa_receipt_1"],
        blockerRefs: [],
        caveatRefs: [],
      },
    })
    expect(event.kind === "world_event" ? event.inference?.workers.map(worker => worker.workerKind) : [])
      .toEqual(["gateway", "verifier"])
    expect(assertWorldPublicSafety(event)).toBe(event)
  })
})
