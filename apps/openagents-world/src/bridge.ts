import {
  assertWorldPublicSafety,
  decodeWorldBridgePayload,
  decodeWorldRow,
  deterministicWorldEventRef,
  worldRowKey,
  type WorldBridgePayload,
  type WorldGatewayLane,
  type WorldRow,
  type WorldSourceRef,
} from "@openagentsinc/world-contract"
import {
  decodePublicActivityTimelineEnvelope,
  type PublicActivityTimelineEnvelope,
} from "@openagentsinc/public-activity-timeline"

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

export const PUBLIC_ACTIVITY_TIMELINE_BRIDGE_SOURCE =
  "https://openagents.com/api/public/activity-timeline"

export const DEFAULT_PUBLIC_ACTIVITY_TIMELINE_BRIDGE_LIMIT = 100

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

export const rowsFromKhalaInferenceReceipt = (
  input: unknown,
  observedAt: string,
  sourceRef: string,
): ReadonlyArray<WorldRow> => {
  const root = record(input)
  const openagents = record(root.openagents)
  const receipt = Object.keys(openagents).length > 0 ? openagents : root
  const requestRef = stringField(root, "requestRef")
    ?? stringField(root, "request_ref")
    ?? stringField(root, "id")
    ?? stableWorldRef("request.khala", sourceRef)
  const receiptRef = stringField(receipt, "receipt")
    ?? stringField(root, "receiptRef")
    ?? stringField(root, "receipt_ref")
    ?? sourceRef
  const model = plainLabel(
    stringField(root, "model")
      ?? stringField(receipt, "model")
      ?? "openagents/khala-mini",
  )
  const route = plainLabel(stringField(receipt, "route") ?? "unknown")
  const regionRef = normalizeRegionRef(
    stringField(root, "regionRef")
      ?? stringField(root, "region_ref")
      ?? `region.${requestRef}`,
  )
  const sourceRefs = uniqueStringRefs([
    sourceRef,
    ...arrayStringField(root, "sourceRefs"),
    ...arrayStringField(root, "source_refs"),
    ...arrayStringField(receipt, "sourceRefs"),
    ...arrayStringField(receipt, "source_refs"),
  ])
  const safety = publicSafety(sourceRefs)
  const gatewayRows = arrayField(root, "gateways").map((item, index) => {
    const gateway = record(item)
    const lane = gatewayLane(stringField(gateway, "lane"))
    const gatewayRef = stringField(gateway, "gatewayRef")
      ?? stringField(gateway, "gateway_ref")
      ?? stableWorldRef("gateway.world", `${sourceRef}:${lane}:${index}`)
    return decodeWorldRow({
      kind: "gateway_station",
      gatewayRef,
      regionRef,
      lane,
      label: plainLabel(stringField(gateway, "label") ?? `${lane} gateway`),
      providerLabel: plainLabel(
        stringField(gateway, "providerLabel")
          ?? stringField(gateway, "provider_label")
          ?? lane,
      ),
      position: vectorField(gateway, "position") ?? { x: 0, y: 0, z: 18 + index * 4 },
      status: stationStatus(stringField(gateway, "status")),
      updatedAt: observedAt,
      safety,
    })
  })

  const priceMsat = finiteNumber(receipt.price_msat) ?? finiteNumber(receipt.priceMsat)
  const costMsat = finiteNumber(receipt.cost_msat) ?? finiteNumber(receipt.costMsat)
  const event = decodeWorldRow({
    kind: "world_event",
    eventRef: stringField(root, "eventRef")
      ?? stringField(root, "event_ref")
      ?? deterministicWorldEventRef(sourceRef, `khala.${requestRef}`, 0),
    regionRef,
    eventKind: "khala_inference_served",
    text: plainText(`${model} served via ${route}`),
    createdAt: observedAt,
    sourceRefs,
    inference: {
      requestRef,
      receiptRef,
      model,
      route,
      workers: inferenceWorkers(receipt, sourceRefs),
      verification: inferenceVerification(stringField(receipt, "verification")),
      ...(costMsat === null ? {} : { costMsat }),
      ...(priceMsat === null ? {} : { priceMsat }),
      settled: booleanField(receipt, "settled") ?? false,
      sourceRefs,
    },
    safety,
  })

  return dedupeBridgeRows([...gatewayRows, event].map(row => assertWorldPublicSafety(row)))
}

export const rowsFromPublicActivityTimelineEvent = (
  input: unknown,
  observedAt: string,
  sourceRef: string,
): ReadonlyArray<WorldRow> => {
  const event = record(input)
  if (
    stringField(event, "kind") !== "khala_inference_served" ||
    stringField(event, "sourceKind") !== "inference_receipt"
  ) {
    return []
  }

  const sourceRefs = uniqueStringRefs([
    sourceRef,
    ...arrayStringField(event, "sourceRefs"),
    ...arrayStringField(event, "source_refs"),
  ])
  const receiptRef =
    sourceRefs.find(ref => receiptSourceRef(ref)) ??
    stringField(event, "targetRef") ??
    stringField(event, "target_ref") ??
    sourceRef
  if (!receiptSourceRef(receiptRef)) {
    return []
  }

  const model = stringField(event, "state") ?? "openagents/khala-mini"
  const createdAt = stringField(event, "ts") ?? observedAt
  const requestRef = inferenceRequestRefFromReceiptRef(receiptRef)
  const actorRef = stringField(event, "actorRef") ?? stringField(event, "actor_ref")
  const providerGatewayRef = actorRef?.startsWith("gateway.")
    ? actorRef
    : stableWorldRef("gateway.khala", receiptRef)
  const lane = gatewayLaneFromGatewayRef(providerGatewayRef)
  const gatewayRef = stableWorldRef(`${providerGatewayRef}.request`, requestRef)

  return rowsFromKhalaInferenceReceipt({
    eventRef: stringField(event, "eventRef")
      ?? stringField(event, "event_ref")
      ?? deterministicWorldEventRef(sourceRef, `khala.${receiptRef}`, 0),
    requestRef,
    model,
    regionRef: `region.${requestRef}`,
    sourceRefs,
    openagents: {
      receipt: receiptRef,
      route: "public_activity_timeline",
      verification: "unknown",
      settled: false,
      sourceRefs,
      workers: [
        {
          workerRef: `worker.${requestRef}`,
          workerKind: "coding_agent",
          label: "Khala request",
          role: "request",
          sourceRefs,
        },
      ],
    },
    gateways: [
      {
        gatewayRef,
        lane,
        label: "Khala gateway",
        providerLabel: gatewayProviderLabel(lane),
        position: { x: 0, y: 0, z: 18 },
        status: "working",
      },
    ],
  }, createdAt, receiptRef)
}

export const publicActivityTimelineBridgePollUrl = (input: {
  readonly sourceRef?: string
  readonly cursor?: string | null
  readonly limit?: number
}): string => {
  const url = new URL(input.sourceRef ?? PUBLIC_ACTIVITY_TIMELINE_BRIDGE_SOURCE)
  url.searchParams.set("kind", "khala_inference_served")
  url.searchParams.set("source", "inference_receipt")
  url.searchParams.set(
    "limit",
    String(
      Number.isFinite(input.limit)
        ? Math.max(1, Math.min(200, Math.trunc(input.limit ?? DEFAULT_PUBLIC_ACTIVITY_TIMELINE_BRIDGE_LIMIT)))
        : DEFAULT_PUBLIC_ACTIVITY_TIMELINE_BRIDGE_LIMIT,
    ),
  )
  if (input.cursor !== undefined && input.cursor !== null && input.cursor.trim().length > 0) {
    url.searchParams.set("since", input.cursor.trim())
  }
  return url.toString()
}

export const rowsFromPublicActivityTimelineEnvelope = (
  input: unknown,
  sourceRef = PUBLIC_ACTIVITY_TIMELINE_BRIDGE_SOURCE,
): ReadonlyArray<WorldRow> => {
  const envelope = decodePublicActivityTimelineEnvelope(input)
  return dedupeBridgeRows(
    envelope.events.flatMap(event =>
      rowsFromPublicActivityTimelineEvent(event, envelope.generatedAt, sourceRef)
    ),
  )
}

export const cursorFromPublicActivityTimelineEnvelope = (
  envelope: PublicActivityTimelineEnvelope,
): string | undefined =>
  envelope.nextCursor ?? envelope.events.at(-1)?.cursor

export const bridgePayloadFromPublicActivityTimelineEnvelope = (
  input: unknown,
  sourceRef = PUBLIC_ACTIVITY_TIMELINE_BRIDGE_SOURCE,
): WorldBridgePayload => {
  const envelope = decodePublicActivityTimelineEnvelope(input)
  const rows = rowsFromPublicActivityTimelineEnvelope(envelope, sourceRef)
  const cursor = cursorFromPublicActivityTimelineEnvelope(envelope)
  return decodeWorldBridgePayload({
    payloadRef: stableWorldRef(
      "bridge_payload.public_activity_timeline",
      `${sourceRef}:${cursor ?? envelope.generatedAt}:${rows.map(row => projectionRowRef(row)).join(",")}`,
    ),
    sourceRef,
    observedAt: envelope.generatedAt,
    rows,
    ...(cursor === undefined ? {} : { cursor }),
  })
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
    case "gateway_station":
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

const publicSafety = (sourceRef: string | ReadonlyArray<string>) => ({
  publicProjectionAllowed: true,
  sourceRefs: (Array.isArray(sourceRef)
    ? uniqueStringRefs(sourceRef)
    : [sourceRef]) as Array<WorldSourceRef>,
  blockerRefs: [],
  caveatRefs: [],
})

const record = (input: unknown): Record<string, unknown> =>
  typeof input === "object" && input !== null && !Array.isArray(input)
    ? input as Record<string, unknown>
    : {}

const arrayField = (input: Record<string, unknown>, key: string): ReadonlyArray<unknown> =>
  Array.isArray(input[key]) ? input[key] : []

const arrayStringField = (
  input: Record<string, unknown>,
  key: string,
): ReadonlyArray<string> =>
  arrayField(input, key).filter((value): value is string =>
    typeof value === "string" && value.trim().length > 0
  )

const uniqueStringRefs = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  values
    .map(value => value.trim())
    .filter((value, index, refs) => value.length > 0 && refs.indexOf(value) === index)

const stringField = (input: Record<string, unknown>, key: string): string | null =>
  typeof input[key] === "string" && input[key].trim().length > 0
    ? input[key].trim()
    : null

const finiteNumber = (input: unknown): number | null =>
  typeof input === "number" && Number.isFinite(input) ? input : null

const booleanField = (input: Record<string, unknown>, key: string): boolean | null =>
  typeof input[key] === "boolean" ? input[key] : null

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

const stationStatus = (input: string | null) => {
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

const pylonStatus = (input: string | null) => stationStatus(input)

const gatewayLane = (input: string | null): WorldGatewayLane => {
  switch (input) {
    case "vertex":
    case "fireworks":
    case "openrouter":
    case "passthrough":
      return input
    default:
      return "passthrough"
  }
}

const gatewayLaneFromGatewayRef = (gatewayRef: string): WorldGatewayLane => {
  const normalized = gatewayRef.toLowerCase()
  if (normalized.includes("fireworks")) return "fireworks"
  if (normalized.includes("openrouter")) return "openrouter"
  if (normalized.includes("vertex")) return "vertex"
  return "passthrough"
}

const gatewayProviderLabel = (lane: WorldGatewayLane): string => {
  switch (lane) {
    case "fireworks":
      return "Fireworks"
    case "openrouter":
      return "OpenRouter"
    case "vertex":
      return "Vertex"
    case "passthrough":
      return "OpenAgents"
  }
}

const receiptSourceRef = (ref: string): boolean =>
  /^receipt\./i.test(ref) || /\/receipts\//i.test(ref)

const inferenceRequestRefFromReceiptRef = (receiptRef: string): string => {
  const prefix = "receipt.inference.charge."
  return receiptRef.startsWith(prefix) && receiptRef.length > prefix.length
    ? `request.khala.${plainLabel(receiptRef.slice(prefix.length))}`
    : stableWorldRef("request.khala", receiptRef)
}

const inferenceVerification = (
  input: string | null,
): "none" | "seeded" | "test_passed" | "exact_trace_replay" | "failed" | "unknown" => {
  switch (input) {
    case "none":
    case "seeded":
    case "test_passed":
    case "exact_trace_replay":
    case "failed":
      return input
    case "seeded_replication":
      return "seeded"
    default:
      return "unknown"
  }
}

const inferenceWorkerKind = (
  input: string | null,
): "coordinator" | "pylon" | "gateway" | "coding_agent" | "verifier" => {
  switch (input) {
    case "coordinator":
    case "pylon":
    case "gateway":
    case "coding_agent":
    case "verifier":
      return input
    default:
      return "gateway"
  }
}

const inferenceWorkers = (
  receipt: Record<string, unknown>,
  sourceRefs: ReadonlyArray<string>,
) =>
  arrayField(receipt, "workers").map((item, index) => {
    const worker = record(item)
    const label = typeof item === "string"
      ? item
      : stringField(worker, "label") ?? stringField(worker, "workerRef") ?? stringField(worker, "worker_ref") ?? `worker-${index}`
    const workerKind = typeof item === "string"
      ? inferenceWorkerKind(label === "validator" || label === "verifier" ? "verifier" : null)
      : inferenceWorkerKind(stringField(worker, "workerKind") ?? stringField(worker, "worker_kind"))
    const role = typeof item === "string" ? null : stringField(worker, "role")
    return {
      workerRef: stringField(worker, "workerRef")
        ?? stringField(worker, "worker_ref")
        ?? stableWorldRef(`worker.khala.${workerKind}`, label),
      workerKind,
      label: plainLabel(label),
      ...(role === null ? {} : { role: plainLabel(role) }),
      sourceRefs,
    }
  })

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
