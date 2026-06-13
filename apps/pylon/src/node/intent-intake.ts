import { createHash } from "node:crypto"
import { Schema as S } from "effect"

export const INTENT_INTAKE_SCHEMA = "openagents.pylon.intent_intake.v0.1" as const

export const IntentStatus = S.Literals([
  "received",
  "planning",
  "fanning_out",
  "shipping",
  "shipped",
  "failed",
])
export type IntentStatus = typeof IntentStatus.Type

export const SubmittedWorkIntent = S.Struct({
  intentId: S.String,
  title: S.String,
  body: S.String,
  scopeHint: S.optional(S.String),
  submittedByClientRef: S.String,
  createdAt: S.String,
})
export type SubmittedWorkIntent = typeof SubmittedWorkIntent.Type

export const decodeSubmittedWorkIntent = S.decodeUnknownSync(SubmittedWorkIntent)

export type IntentStatusEvent = {
  status: IntentStatus
  observedAt: string
}

export type IntentProjection = {
  schema: typeof INTENT_INTAKE_SCHEMA
  intentId: string
  titleRef: string
  bodyRef: string
  scopeHintRef: string | null
  submittedByClientRef: string
  status: IntentStatus
  createdAt: string
  updatedAt: string
  statusHistory: IntentStatusEvent[]
}

type IntentRecord = {
  intent: SubmittedWorkIntent
  projection: IntentProjection
}

export type IntentQueue = {
  enqueue: (intent: SubmittedWorkIntent) => IntentProjection
  get: (intentId: string) => IntentProjection | null
  list: () => IntentProjection[]
  advanceStatus: (intentId: string, status: IntentStatus, observedAt?: string) => IntentProjection
}

const LEGAL_TRANSITIONS: Readonly<Record<IntentStatus, ReadonlySet<IntentStatus>>> = {
  received: new Set(["planning", "failed"]),
  planning: new Set(["fanning_out", "failed"]),
  fanning_out: new Set(["shipping", "failed"]),
  shipping: new Set(["shipped", "failed"]),
  shipped: new Set(),
  failed: new Set(),
}

function nowIso() {
  return new Date().toISOString()
}

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function cloneProjection(projection: IntentProjection): IntentProjection {
  return {
    ...projection,
    statusHistory: projection.statusHistory.map((event) => ({ ...event })),
  }
}

export function transitionIntentStatus(current: IntentStatus, next: IntentStatus): IntentStatus {
  if (LEGAL_TRANSITIONS[current].has(next)) {
    return next
  }

  throw new Error(`illegal intent status transition: ${current} -> ${next}`)
}

export function createIntentQueue(): IntentQueue {
  const records = new Map<string, IntentRecord>()

  function projectionFor(intent: SubmittedWorkIntent): IntentProjection {
    const received: IntentStatusEvent = { status: "received", observedAt: intent.createdAt }
    return {
      schema: INTENT_INTAKE_SCHEMA,
      intentId: intent.intentId,
      titleRef: stableRef("intent.title", intent.title),
      bodyRef: stableRef("intent.body", intent.body),
      scopeHintRef: intent.scopeHint === undefined ? null : stableRef("intent.scope_hint", intent.scopeHint),
      submittedByClientRef: intent.submittedByClientRef,
      status: "received",
      createdAt: intent.createdAt,
      updatedAt: intent.createdAt,
      statusHistory: [received],
    }
  }

  return {
    enqueue(intent) {
      const existing = records.get(intent.intentId)
      if (existing) {
        return cloneProjection(existing.projection)
      }

      const projection = projectionFor(intent)
      records.set(intent.intentId, { intent, projection })
      return cloneProjection(projection)
    },

    get(intentId) {
      const record = records.get(intentId)
      return record ? cloneProjection(record.projection) : null
    },

    list() {
      return [...records.values()].map((record) => cloneProjection(record.projection))
    },

    advanceStatus(intentId, status, observedAt = nowIso()) {
      const record = records.get(intentId)
      if (!record) {
        throw new Error(`unknown intent: ${intentId}`)
      }

      const next = transitionIntentStatus(record.projection.status, status)
      const projection: IntentProjection = {
        ...record.projection,
        status: next,
        updatedAt: observedAt,
        statusHistory: [...record.projection.statusHistory, { status: next, observedAt }],
      }
      records.set(intentId, { ...record, projection })
      return cloneProjection(projection)
    },
  }
}
