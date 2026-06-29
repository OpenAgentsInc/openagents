import { createHash } from "node:crypto"
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs"
import { dirname } from "node:path"
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

export type IntentListPage = {
  intents: IntentProjection[]
  // Resume token = the latest updatedAt across ALL intents (null when empty).
  // A client passes its last cursor back as sinceCursor to get only intents
  // changed since, then advances to the returned cursor — cursor-resumable.
  cursor: string | null
}

export type IntentQueue = {
  enqueue: (intent: SubmittedWorkIntent) => IntentProjection
  get: (intentId: string) => IntentProjection | null
  // Internal: full intent (title/body plaintext) for the coordinator runtime to
  // build session objectives. NOT exposed over the control API (projections
  // stay refs-only).
  getIntent: (intentId: string) => SubmittedWorkIntent | null
  list: () => IntentProjection[]
  listSince: (sinceCursor?: string) => IntentListPage
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

// Optional durable persistence: when a persistPath is given, the queue loads
// its records on start and rewrites them on every mutation, so submitted intents
// survive a node restart (the in-memory map alone loses them). The file is local
// node state (like the control token) and holds the raw intents; the PROJECTION
// stays refs-only regardless.
export function createIntentQueue(options: { persistPath?: string } = {}): IntentQueue {
  const records = new Map<string, IntentRecord>()
  const persistPath = options.persistPath

  if (persistPath !== undefined && existsSync(persistPath)) {
    try {
      const loaded = JSON.parse(readFileSync(persistPath, "utf8")) as IntentRecord[]
      if (Array.isArray(loaded)) {
        for (const record of loaded) {
          if (record?.intent?.intentId) records.set(record.intent.intentId, record)
        }
      }
    } catch {
      // corrupt/unreadable persistence -> start empty rather than crash the node
    }
  }

  const persist = (): void => {
    if (persistPath === undefined) return
    try {
      mkdirSync(dirname(persistPath), { recursive: true })
      writeFileSync(persistPath, JSON.stringify([...records.values()]))
    } catch {
      // best-effort: persistence failure must never break intake
    }
  }

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
      persist()
      return cloneProjection(projection)
    },

    get(intentId) {
      const record = records.get(intentId)
      return record ? cloneProjection(record.projection) : null
    },

    getIntent(intentId) {
      const record = records.get(intentId)
      return record ? { ...record.intent } : null
    },

    list() {
      return [...records.values()].map((record) => cloneProjection(record.projection))
    },

    listSince(sinceCursor) {
      const all = [...records.values()]
        .map((record) => cloneProjection(record.projection))
        .sort((left, right) => left.updatedAt.localeCompare(right.updatedAt))
      const intents = sinceCursor === undefined ? all : all.filter((p) => p.updatedAt > sinceCursor)
      const cursor = all.length > 0 ? all[all.length - 1].updatedAt : null
      return { intents, cursor }
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
      persist()
      return cloneProjection(projection)
    },
  }
}
