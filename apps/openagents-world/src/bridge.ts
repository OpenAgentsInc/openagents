import {
  assertWorldPublicSafety,
  decodeWorldRow,
  deterministicWorldEventRef,
  worldRowKey,
  type WorldRow,
  type WorldSourceRef,
} from "@openagentsinc/world-contract"

import {
  normalizeRegionRef,
  stableWorldRef,
} from "./protocol"

export type ProjectionRowMetadata = Readonly<{
  rowRef: string
  rowKind: WorldRow["kind"]
  regionRef: string | null
  runRef: string | null
  sourceRef: string
  cursor: string | null
  updatedAt: string
}>

export type BridgeRetryDecision =
  | Readonly<{ kind: "retry"; reason: string; attempt: number; maxAttempts: number }>
  | Readonly<{ kind: "terminal"; reason: string; attempt: number; maxAttempts: number }>

export const projectionRowRef = (row: WorldRow): string =>
  `${row.kind}:${worldRowKey(row)}`

export const projectionRowMetadata = (row: WorldRow): ProjectionRowMetadata => ({
  rowRef: projectionRowRef(row),
  rowKind: row.kind,
  regionRef: regionRefForRow(row),
  runRef: runRefForRow(row),
  sourceRef: row.safety.sourceRefs[0] ?? "source.openagents_world.bridge",
  cursor: row.kind === "projection_cursor" ? row.cursor : null,
  updatedAt: timestampForRow(row),
})

export const decodePublicBridgeRows = (rows: ReadonlyArray<unknown>): ReadonlyArray<WorldRow> =>
  rows.map((candidate) => assertWorldPublicSafety(decodeWorldRow(candidate)))

export const dedupeBridgeRows = (rows: ReadonlyArray<WorldRow>): ReadonlyArray<WorldRow> => {
  const byRef = new Map<string, WorldRow>()
  for (const row of rows) {
    byRef.set(projectionRowRef(row), row)
  }
  return [...byRef.values()]
}

export const bridgeHealthRow = (input: {
  readonly sourceRef: string
  readonly status: "current" | "stale" | "failed" | "disabled"
  readonly observedAt: string
  readonly diagnosticRefs?: ReadonlyArray<string>
  readonly lagSeconds?: number
}): WorldRow =>
  decodeWorldRow({
    kind: "bridge_health",
    bridgeRef: stableWorldRef("bridge_health.world", `${input.sourceRef}:${input.status}`),
    sourceRef: input.sourceRef,
    status: input.status,
    observedAt: input.observedAt,
    ...(input.lagSeconds === undefined ? {} : { lagSeconds: input.lagSeconds }),
    diagnosticRefs: [...(input.diagnosticRefs ?? [])],
    safety: publicSafety(input.sourceRef),
  })

export const projectionCursorRow = (input: {
  readonly sourceRef: string
  readonly cursor: string
  readonly observedAt: string
}): WorldRow =>
  decodeWorldRow({
    kind: "projection_cursor",
    cursorRef: stableWorldRef("projection_cursor.world", input.sourceRef),
    sourceRef: input.sourceRef,
    cursor: input.cursor,
    observedAt: input.observedAt,
    safety: publicSafety(input.sourceRef),
  })

export const rowsFromTassadarRunSummary = (
  input: unknown,
  observedAt: string,
  sourceRef: string,
): ReadonlyArray<WorldRow> => {
  const summary = record(input)
  const runRef = stringField(summary, "runRef")
    ?? stringField(summary, "run_ref")
    ?? stableWorldRef("run.world", sourceRef)
  const label = plainLabel(
    stringField(summary, "label")
      ?? stringField(summary, "title")
      ?? `Tassadar run ${runRef}`,
  )
  const state = trainingRunState(stringField(summary, "state") ?? stringField(summary, "status"))
  const regionRef = normalizeRegionRef(
    stringField(summary, "regionRef")
      ?? stringField(summary, "region_ref")
      ?? `region.${runRef}`,
  )
  const safety = publicSafety(sourceRef)
  const rows: Array<WorldRow> = [
    decodeWorldRow({
      kind: "training_run",
      runRef,
      label,
      state,
      updatedAt: observedAt,
      safety,
    }),
  ]

  rows.push(...arrayField(summary, "pylons").map((item, index) => {
    const pylon = record(item)
    const pylonRef = stringField(pylon, "pylonRef")
      ?? stringField(pylon, "pylon_ref")
      ?? stableWorldRef("pylon.world", `${sourceRef}:${runRef}:${index}`)
    return decodeWorldRow({
      kind: "pylon_station",
      pylonRef,
      regionRef,
      label: plainLabel(stringField(pylon, "label") ?? pylonRef),
      position: vectorField(pylon, "position") ?? { x: index * 4, y: 0, z: 0 },
      status: pylonStatus(stringField(pylon, "status")),
      updatedAt: observedAt,
      safety,
    })
  }))

  rows.push(...arrayField(summary, "entities").map((item, index) => {
    const entity = record(item)
    const entityRef = stringField(entity, "entityRef")
      ?? stringField(entity, "entity_ref")
      ?? stableWorldRef("entity.world", `${sourceRef}:${runRef}:${index}`)
    return decodeWorldRow({
      kind: "run_entity",
      entityRef,
      runRef,
      label: plainLabel(stringField(entity, "label") ?? entityRef),
      entityKind: plainLabel(stringField(entity, "entityKind") ?? stringField(entity, "kind") ?? "entity"),
      updatedAt: observedAt,
      safety,
    })
  }))

  rows.push(...arrayField(summary, "proofRefs").map((item, index) => {
    const proof = record(item)
    const proofRef = stringField(proof, "proofRef")
      ?? stringField(proof, "proof_ref")
      ?? stableWorldRef("proof.world", `${sourceRef}:${runRef}:${index}`)
    return decodeWorldRow({
      kind: "proof_ref",
      proofRef,
      runRef,
      label: plainLabel(stringField(proof, "label") ?? proofRef),
      url: publicUrl(stringField(proof, "url")),
      updatedAt: observedAt,
      safety,
    })
  }))

  rows.push(...arrayField(summary, "settlementRefs").map((item, index) => {
    const settlement = record(item)
    const settlementRef = stringField(settlement, "settlementRef")
      ?? stringField(settlement, "settlement_ref")
      ?? stableWorldRef("settlement.world", `${sourceRef}:${runRef}:${index}`)
    const amountSats = finiteNumber(settlement.amountSats) ?? finiteNumber(settlement.amount_sats)
    return decodeWorldRow({
      kind: "settlement_ref",
      settlementRef,
      runRef,
      label: plainLabel(stringField(settlement, "label") ?? settlementRef),
      ...(amountSats === null ? {} : { amountSats }),
      updatedAt: observedAt,
      safety,
    })
  }))

  rows.push(...arrayField(summary, "events").map((item, index) => {
    const event = record(item)
    const eventKind = plainLabel(stringField(event, "eventKind") ?? stringField(event, "kind") ?? "summary")
    return decodeWorldRow({
      kind: "world_event",
      eventRef: stringField(event, "eventRef")
        ?? stringField(event, "event_ref")
        ?? deterministicWorldEventRef(sourceRef, `${runRef}.${eventKind}`, index),
      regionRef,
      runRef,
      eventKind,
      text: plainText(stringField(event, "text") ?? eventKind),
      createdAt: stringField(event, "createdAt") ?? stringField(event, "created_at") ?? observedAt,
      sourceRefs: [sourceRef],
      safety,
    })
  }))

  return dedupeBridgeRows(rows.map(row => assertWorldPublicSafety(row)))
}

export const planBridgeRetry = (input: {
  readonly reason: string
  readonly attempt: number
  readonly maxAttempts: number
  readonly terminal?: boolean
}): BridgeRetryDecision =>
  input.terminal || input.attempt >= input.maxAttempts
    ? {
        kind: "terminal",
        reason: publicReason(input.reason),
        attempt: input.attempt,
        maxAttempts: input.maxAttempts,
      }
    : {
        kind: "retry",
        reason: publicReason(input.reason),
        attempt: input.attempt,
        maxAttempts: input.maxAttempts,
      }

const regionRefForRow = (row: WorldRow): string | null => {
  switch (row.kind) {
    case "world_region":
      return row.regionRef
    case "pylon_station":
    case "agent_avatar":
    case "avatar_position":
    case "local_chat_message":
    case "chat_bubble":
    case "local_emote":
    case "agent_intent":
      return row.regionRef
    case "world_event":
      return row.regionRef ?? null
    case "training_run":
    case "run_entity":
    case "world_edge":
    case "proof_ref":
    case "settlement_ref":
    case "projection_cursor":
    case "bridge_health":
      return null
  }
}

const runRefForRow = (row: WorldRow): string | null => {
  switch (row.kind) {
    case "training_run":
    case "run_entity":
    case "proof_ref":
    case "settlement_ref":
      return row.runRef
    case "world_event":
      return row.runRef ?? null
    default:
      return null
  }
}

const timestampForRow = (row: WorldRow): string => {
  switch (row.kind) {
    case "avatar_position":
    case "projection_cursor":
    case "bridge_health":
      return row.observedAt
    case "local_chat_message":
    case "chat_bubble":
    case "local_emote":
    case "agent_intent":
    case "world_event":
      return row.createdAt
    default:
      return row.updatedAt
  }
}

const publicSafety = (sourceRef: string) => ({
  publicProjectionAllowed: true,
  sourceRefs: [sourceRef as WorldSourceRef],
  blockerRefs: [],
  caveatRefs: [],
})

const record = (input: unknown): Record<string, unknown> =>
  typeof input === "object" && input !== null && !Array.isArray(input)
    ? input as Record<string, unknown>
    : {}

const arrayField = (input: Record<string, unknown>, key: string): ReadonlyArray<unknown> =>
  Array.isArray(input[key]) ? input[key] : []

const stringField = (input: Record<string, unknown>, key: string): string | null =>
  typeof input[key] === "string" && input[key].trim().length > 0
    ? input[key].trim()
    : null

const finiteNumber = (input: unknown): number | null =>
  typeof input === "number" && Number.isFinite(input) ? input : null

const vectorField = (
  input: Record<string, unknown>,
  key: string,
): { readonly x: number; readonly y: number; readonly z: number } | null => {
  const value = record(input[key])
  const x = finiteNumber(value.x)
  const y = finiteNumber(value.y)
  const z = finiteNumber(value.z)
  return x === null || y === null || z === null ? null : { x, y, z }
}

const trainingRunState = (input: string | null) => {
  switch (input) {
    case "pending":
    case "assigned":
    case "tracing":
    case "replaying":
    case "accepted":
    case "rejected":
    case "settled":
    case "blocked":
      return input
    default:
      return "pending"
  }
}

const pylonStatus = (input: string | null) => {
  switch (input) {
    case "online":
    case "working":
    case "offline":
    case "blocked":
      return input
    default:
      return "unknown"
  }
}

const publicUrl = (input: string | null): string =>
  input !== null && /^https:\/\//.test(input) ? input : "https://openagents.com/docs/product-promises"

const plainLabel = (input: string): string =>
  publicReason(input).slice(0, 96) || "public projection"

const plainText = (input: string): string =>
  publicReason(input).slice(0, 280) || "Public world event"

const publicReason = (input: string): string =>
  input
    .replace(/raw_prompt/gi, "redacted")
    .replace(/raw_shell_log/gi, "redacted")
    .replace(/provider_payload/gi, "redacted")
    .replace(/secret/gi, "redacted")
    .replace(/\/Users\/[^\s]+/g, "[local-path]")
    .replace(/sk-[a-z0-9_-]+/gi, "[redacted-token]")
    .replace(/[\u0000-\u001f\u007f<>]/g, "")
    .replace(/\s+/g, " ")
    .trim()
