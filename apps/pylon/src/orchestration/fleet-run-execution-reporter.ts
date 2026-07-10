import { createHash } from "node:crypto"

import { canonicalJson } from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"

import type { PylonOrchestrationStore } from "./store.js"

export const PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA =
  "openagents.pylon.fleet_run_execution_batch.v1" as const
export const PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA =
  "openagents.pylon.fleet_run_execution_event.v1" as const
export const PYLON_FLEET_RUN_EXECUTION_ACK_SCHEMA =
  "openagents.pylon.fleet_run_execution_ack.v1" as const

const RUN_REF = /^fleet_run\.sarah\.[0-9a-f]{20}$/u
const CLAIM_REF = /^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u
const EVENT_REF = /^event\.pylon\.fleet_run\.[0-9a-f]{24}$/u
const PYLON_REF = /^[a-z0-9][a-z0-9._:-]{2,119}$/u
const PUBLIC_REF = /^[A-Za-z0-9][A-Za-z0-9._:/#=-]{0,199}$/u
const ACCOUNT_REF_HASH = /^account\.pylon\.(?:codex|claude_agent|grok)\.[0-9a-f]{24}$/u
const ISO_TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u
const MAX_BATCH_EVENTS = 64
const MAX_BATCH_BYTES = 256 * 1_024

const RunRef = S.String.check(S.isPattern(RUN_REF))
const ClaimRef = S.String.check(S.isPattern(CLAIM_REF))
const EventRef = S.String.check(S.isPattern(EVENT_REF))
const PublicRef = S.String.check(S.isPattern(PUBLIC_REF))
const AccountRefHash = S.String.check(S.isPattern(ACCOUNT_REF_HASH))
const IsoTimestamp = S.String.check(S.isPattern(ISO_TIMESTAMP))
const WorkerKind = S.Literals(["codex", "claude", "grok"])
const BlockerRefs = S.Array(PublicRef).check(S.isMaxLength(32))
export const PylonFleetRunProjectedUsageEvidenceSchema = S.Union([
  S.Struct({
    truth: S.Literal("exact"),
    tokenUsageRefs: S.Array(PublicRef).check(S.isMinLength(1), S.isMaxLength(100)),
  }),
  S.Struct({
    truth: S.Literal("not_measured"),
    tokenUsageRefs: S.Tuple([]),
  }),
])
export type PylonFleetRunProjectedUsageEvidence =
  typeof PylonFleetRunProjectedUsageEvidenceSchema.Type

export const PylonFleetRunStartedExecutionEvent = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  eventRef: EventRef,
  kind: S.Literal("run_started"),
  observedAt: IsoTimestamp,
})

export const PylonFleetRunWorkProgressExecutionEvent = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  eventRef: EventRef,
  kind: S.Literal("work_progress"),
  observedAt: IsoTimestamp,
  unitRef: PublicRef,
  workClaimRef: PublicRef,
  assignmentRef: S.optionalKey(PublicRef),
  workerKind: WorkerKind,
  accountRefHash: S.optionalKey(AccountRefHash),
  blockerRefs: BlockerRefs,
})

export const PylonFleetRunWorkTerminalExecutionEvent = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  eventRef: EventRef,
  kind: S.Literal("work_terminal"),
  observedAt: IsoTimestamp,
  unitRef: PublicRef,
  workClaimRef: PublicRef,
  assignmentRef: PublicRef,
  workerKind: WorkerKind,
  accountRefHash: AccountRefHash,
  terminalState: S.Literals(["accepted", "failed", "stale"]),
  closeoutRef: PublicRef,
  usageEvidence: PylonFleetRunProjectedUsageEvidenceSchema,
  blockerRefs: BlockerRefs,
})

export const PylonFleetRunTerminalExecutionEvent = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: S.Int.check(S.isGreaterThanOrEqualTo(1)),
  eventRef: EventRef,
  kind: S.Literal("run_terminal"),
  observedAt: IsoTimestamp,
  terminalState: S.Literals(["completed", "failed", "stopped"]),
  blockerRefs: BlockerRefs,
})

export const PylonFleetRunExecutionEventSchema = S.Union([
  PylonFleetRunStartedExecutionEvent,
  PylonFleetRunWorkProgressExecutionEvent,
  PylonFleetRunWorkTerminalExecutionEvent,
  PylonFleetRunTerminalExecutionEvent,
])
export type PylonFleetRunExecutionEvent =
  typeof PylonFleetRunExecutionEventSchema.Type

export type PylonFleetRunExecutionEventInput =
  PylonFleetRunExecutionEvent extends infer Event
    ? Event extends PylonFleetRunExecutionEvent
      ? Omit<Event, "eventRef" | "sequence">
      : never
    : never

export const PylonFleetRunExecutionBatchSchema = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA),
  claimRef: ClaimRef,
  events: S.Array(PylonFleetRunExecutionEventSchema).check(
    S.isMinLength(1),
    S.isMaxLength(MAX_BATCH_EVENTS),
  ),
})
export type PylonFleetRunExecutionBatch =
  typeof PylonFleetRunExecutionBatchSchema.Type

export const PylonFleetRunExecutionAckSchema = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_ACK_SCHEMA),
  runRef: RunRef,
  claimRef: ClaimRef,
  acceptedThroughSequence: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  storedEventCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  duplicateEventCount: S.Int.check(S.isGreaterThanOrEqualTo(0)),
  execution: S.Struct({
    state: S.Literals(["pending", "running", "completed", "failed", "stopped"]),
    counters: S.Struct({
      workUnitsTotal: S.Int.check(S.isGreaterThanOrEqualTo(0)),
      activeAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
      acceptedAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
      failedAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
      staleAssignments: S.Int.check(S.isGreaterThanOrEqualTo(0)),
    }),
    updatedAt: IsoTimestamp,
  }),
})
export type PylonFleetRunExecutionAck =
  typeof PylonFleetRunExecutionAckSchema.Type

export type PylonFleetRunExecutionHttpPort = {
  readonly append: (input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly batch: PylonFleetRunExecutionBatch
  }) => Promise<PylonFleetRunExecutionAck>
}

export type MakePylonFleetRunExecutionHttpPortOptions = {
  readonly agentToken: string
  readonly baseUrl: string
  readonly fetchImpl?: typeof globalThis.fetch | undefined
  readonly requestTimeoutMs?: number | undefined
}

export type OpenPylonFleetRunExecutionReporterInput = {
  readonly store: PylonOrchestrationStore
  readonly pylonRef: string
  readonly runRef: string
  readonly remote: PylonFleetRunExecutionHttpPort
  readonly now?: (() => Date) | undefined
}

export type PylonFleetRunExecutionReporter = {
  readonly record: (event: PylonFleetRunExecutionEventInput) => Promise<void>
  readonly flush: () => Promise<PylonFleetRunExecutionAck | null>
  readonly close: () => Promise<void>
}

const unavailable = (): Error =>
  new Error("Pylon FleetRun execution projection is unavailable")

const validatedBaseUrl = (value: string): URL => {
  try {
    const parsed = new URL(value)
    const loopbackHttp = parsed.protocol === "http:" &&
      (parsed.hostname === "localhost" ||
        parsed.hostname === "127.0.0.1" ||
        parsed.hostname === "[::1]")
    if (
      (parsed.protocol !== "https:" && !loopbackHttp) ||
      parsed.username !== "" ||
      parsed.password !== ""
    ) throw unavailable()
    return parsed
  } catch {
    throw unavailable()
  }
}

const readBoundedJson = async (response: Response): Promise<unknown> => {
  const declared = Number(response.headers.get("content-length"))
  if (Number.isFinite(declared) && declared > MAX_BATCH_BYTES) throw unavailable()
  const text = await response.text()
  if (new TextEncoder().encode(text).byteLength > MAX_BATCH_BYTES) throw unavailable()
  try {
    return JSON.parse(text) as unknown
  } catch {
    throw unavailable()
  }
}

export function makePylonFleetRunExecutionHttpPort(
  options: MakePylonFleetRunExecutionHttpPortOptions,
): PylonFleetRunExecutionHttpPort {
  const baseUrl = validatedBaseUrl(options.baseUrl)
  if (options.agentToken.trim() === "") throw unavailable()
  const fetchImpl = options.fetchImpl ?? globalThis.fetch
  const requestTimeoutMs = options.requestTimeoutMs ?? 15_000
  if (
    !Number.isInteger(requestTimeoutMs) ||
    requestTimeoutMs < 1_000 ||
    requestTimeoutMs > 60_000
  ) throw unavailable()

  return {
    append: async ({ pylonRef, runRef, batch }) => {
      if (!PYLON_REF.test(pylonRef) || !RUN_REF.test(runRef)) throw unavailable()
      const decodedBatch = S.decodeUnknownSync(PylonFleetRunExecutionBatchSchema)(batch, {
        onExcessProperty: "error",
      })
      const body = canonicalJson(decodedBatch)
      if (new TextEncoder().encode(body).byteLength > MAX_BATCH_BYTES) throw unavailable()
      const first = decodedBatch.events[0]
      const last = decodedBatch.events.at(-1)
      if (first === undefined || last === undefined) throw unavailable()
      const idempotencyDigest = createHash("sha256")
        .update(`${runRef}\0${first.eventRef}\0${last.eventRef}`)
        .digest("hex")
        .slice(0, 24)
      let response: Response
      try {
        response = await fetchImpl(
          new URL(
            `/api/pylons/${encodeURIComponent(pylonRef)}/fleet-runs/${encodeURIComponent(runRef)}/events`,
            baseUrl,
          ),
          {
            method: "POST",
            headers: {
              Accept: "application/json",
              Authorization: `Bearer ${options.agentToken}`,
              "Content-Type": "application/json",
              "Idempotency-Key": `pylon.fleet-run.events.${idempotencyDigest}`,
            },
            body,
            signal: AbortSignal.timeout(requestTimeoutMs),
          },
        )
      } catch {
        throw unavailable()
      }
      const raw = await readBoundedJson(response)
      if (!response.ok) throw unavailable()
      try {
        const ack = S.decodeUnknownSync(PylonFleetRunExecutionAckSchema)(raw, {
          onExcessProperty: "error",
        })
        if (ack.runRef !== runRef || ack.claimRef !== batch.claimRef) throw unavailable()
        return ack
      } catch {
        throw unavailable()
      }
    },
  }
}

const decodeStoredEvent = (value: string): PylonFleetRunExecutionEvent =>
  S.decodeUnknownSync(S.fromJsonString(PylonFleetRunExecutionEventSchema))(value, {
    onExcessProperty: "error",
  })

const eventRefFor = (
  runRef: string,
  claimRef: string,
  event: PylonFleetRunExecutionEventInput,
): string => `event.pylon.fleet_run.${createHash("sha256")
  .update(canonicalJson({ runRef, claimRef, event }))
  .digest("hex")
  .slice(0, 24)}`

/**
 * Durable local outbox + authenticated server append loop.
 *
 * `record` commits the normalized event to Pylon SQLite before attempting the
 * network. Delivery failure is fail-soft for the executor: the undelivered
 * row remains pending and a reopened reporter retries the exact same sequence
 * and bytes. `flush` stays strict for explicit diagnostics and tests.
 */
export function openPylonFleetRunExecutionReporter(
  input: OpenPylonFleetRunExecutionReporterInput,
): PylonFleetRunExecutionReporter {
  if (!PYLON_REF.test(input.pylonRef) || !RUN_REF.test(input.runRef)) throw unavailable()
  const run = input.store.getFleetRun(input.runRef)
  const claimRef = run?.authorityBinding?.phase === "accepted"
    ? run.authorityBinding.claimRef
    : null
  if (claimRef === null || !CLAIM_REF.test(claimRef)) throw unavailable()
  const readNow = (): Date => {
    const value = input.now?.() ?? new Date()
    if (!Number.isFinite(value.getTime())) throw unavailable()
    return value
  }
  let closed = false
  let tail = Promise.resolve<PylonFleetRunExecutionAck | null>(null)

  const flushOnce = async (): Promise<PylonFleetRunExecutionAck | null> => {
    if (closed) return null
    const pending = input.store.listFleetRunExecutionOutbox(input.runRef, {
      pendingOnly: true,
      limit: MAX_BATCH_EVENTS,
    })
    if (pending.length === 0) return null
    if (pending.some(entry => entry.claimRef !== claimRef)) throw unavailable()
    const events = pending.map(entry => decodeStoredEvent(entry.eventJson))
    for (let index = 1; index < events.length; index += 1) {
      if (events[index]!.sequence !== events[index - 1]!.sequence + 1) throw unavailable()
    }
    const batch = S.decodeUnknownSync(PylonFleetRunExecutionBatchSchema)({
      schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
      claimRef,
      events,
    }, { onExcessProperty: "error" })
    const ack = await input.remote.append({
      pylonRef: input.pylonRef,
      runRef: input.runRef,
      batch,
    })
    const lastSequence = events.at(-1)?.sequence ?? 0
    if (
      ack.acceptedThroughSequence < lastSequence ||
      ack.acceptedThroughSequence < events[0]!.sequence
    ) throw unavailable()
    input.store.markFleetRunExecutionOutboxDelivered(
      input.runRef,
      claimRef,
      ack.acceptedThroughSequence,
      readNow(),
    )
    return ack
  }

  const flush = (): Promise<PylonFleetRunExecutionAck | null> => {
    const next = tail.then(flushOnce, flushOnce)
    tail = next
    return next
  }

  return {
    record: async event => {
      if (closed) throw unavailable()
      const observedAt = new Date(event.observedAt)
      if (!Number.isFinite(observedAt.getTime())) throw unavailable()
      const eventRef = eventRefFor(input.runRef, claimRef, event)
      input.store.enqueueFleetRunExecutionOutbox({
        runRef: input.runRef,
        claimRef,
        eventRef,
        eventJsonForSequence: sequence => canonicalJson(
          S.decodeUnknownSync(PylonFleetRunExecutionEventSchema)({
            ...event,
            sequence,
            eventRef,
          }, { onExcessProperty: "error" }),
        ),
        now: observedAt,
      })
      await flush().catch(() => null)
    },
    flush,
    close: async () => {
      if (closed) return
      await flush().catch(() => null)
      closed = true
    },
  }
}
