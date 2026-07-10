import { createHash } from "node:crypto"

import { canonicalJson } from "@openagentsinc/khala-sync"
import { Schema as S } from "effect"

import type {
  FleetRunExecutionOutboxEntry,
  PylonOrchestrationStore,
} from "./store.js"

export const PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA =
  "openagents.pylon.fleet_run_execution_batch.v1" as const
export const PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA =
  "openagents.pylon.fleet_run_execution_event.v1" as const
export const PYLON_FLEET_RUN_EXECUTION_ACK_SCHEMA =
  "openagents.pylon.fleet_run_execution_ack.v1" as const

const RUN_REF = /^fleet_run\.sarah\.[0-9a-f]{20}$/u
const CLAIM_REF = /^claim\.sarah_fleet_run\.[0-9a-f]{24}$/u
const EVENT_REF = /^event\.pylon\.fleet_run\.[0-9a-f]{24}$/u
const DELIVERY_BATCH_REF = /^batch\.pylon\.fleet_run\.[0-9a-f]{24}$/u
const PYLON_REF = /^[a-z0-9][a-z0-9._:-]{2,119}$/u
const PUBLIC_REF = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,179}$/u
const ACCOUNT_REF_HASH = /^account\.pylon\.(?:codex|claude_agent|grok)\.[0-9a-f]{24}$/u
const ISO_TIMESTAMP = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z$/u
const MAX_BATCH_EVENTS = 64
const MAX_BATCH_BYTES = 256 * 1_024
const MAX_SAFE_INTEGER = Number.MAX_SAFE_INTEGER

const RunRef = S.String.check(S.isPattern(RUN_REF))
const ClaimRef = S.String.check(S.isPattern(CLAIM_REF))
const EventRef = S.String.check(S.isPattern(EVENT_REF))
const PublicRef = S.String.check(S.isPattern(PUBLIC_REF))
const BlockerRef = S.String.check(
  S.isPattern(/^blocker\.[A-Za-z0-9][A-Za-z0-9._:/#-]{0,171}$/u),
)
const AccountRefHash = S.String.check(S.isPattern(ACCOUNT_REF_HASH))
const IsoTimestamp = S.String.check(S.isPattern(ISO_TIMESTAMP))
const WorkerKind = S.Literals(["codex", "claude", "grok"])
const BlockerRefs = S.Array(BlockerRef).check(S.isMaxLength(32))
const NonEmptyBlockerRefs = S.Array(BlockerRef).check(S.isMinLength(1), S.isMaxLength(32))
const SafePositiveInt = S.Int.check(
  S.isGreaterThanOrEqualTo(1),
  S.isLessThanOrEqualTo(MAX_SAFE_INTEGER),
)
const SafeNonNegativeInt = S.Int.check(
  S.isGreaterThanOrEqualTo(0),
  S.isLessThanOrEqualTo(MAX_SAFE_INTEGER),
)
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
  sequence: SafePositiveInt,
  eventRef: EventRef,
  kind: S.Literal("run_started"),
  observedAt: IsoTimestamp,
})

export const PylonFleetRunWorkProgressExecutionEvent = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: SafePositiveInt,
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

const PylonFleetRunWorkTerminalExecutionEventBase = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: SafePositiveInt,
  eventRef: EventRef,
  kind: S.Literal("work_terminal"),
  observedAt: IsoTimestamp,
  unitRef: PublicRef,
  workClaimRef: PublicRef,
  workerKind: WorkerKind,
})

export const PylonFleetRunAcceptedWorkTerminalExecutionEvent = S.Struct({
  ...PylonFleetRunWorkTerminalExecutionEventBase.fields,
  terminalState: S.Literal("accepted"),
  assignmentRef: PublicRef,
  accountRefHash: AccountRefHash,
  closeoutRef: PublicRef,
  usageEvidence: PylonFleetRunProjectedUsageEvidenceSchema,
  blockerRefs: S.Tuple([]),
})

export const PylonFleetRunUnprovenWorkTerminalExecutionEvent = S.Struct({
  ...PylonFleetRunWorkTerminalExecutionEventBase.fields,
  terminalState: S.Literals(["failed", "stale"]),
  blockerRefs: NonEmptyBlockerRefs,
})

export const PylonFleetRunProvenFailedWorkTerminalExecutionEvent = S.Struct({
  ...PylonFleetRunWorkTerminalExecutionEventBase.fields,
  terminalState: S.Literals(["failed", "stale"]),
  assignmentRef: PublicRef,
  accountRefHash: AccountRefHash,
  closeoutRef: PublicRef,
  usageEvidence: PylonFleetRunProjectedUsageEvidenceSchema,
  blockerRefs: NonEmptyBlockerRefs,
})

export const PylonFleetRunWorkTerminalExecutionEvent = S.Union([
  PylonFleetRunAcceptedWorkTerminalExecutionEvent,
  PylonFleetRunUnprovenWorkTerminalExecutionEvent,
  PylonFleetRunProvenFailedWorkTerminalExecutionEvent,
])

export const PylonFleetRunTerminalExecutionEvent = S.Struct({
  schema: S.Literal(PYLON_FLEET_RUN_EXECUTION_EVENT_SCHEMA),
  sequence: SafePositiveInt,
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
  acceptedThroughSequence: SafeNonNegativeInt,
  storedEventCount: SafeNonNegativeInt,
  duplicateEventCount: SafeNonNegativeInt,
  execution: S.Struct({
    state: S.Literals(["pending", "running", "completed", "failed", "stopped"]),
    lastSequence: SafeNonNegativeInt,
    counters: S.Struct({
      workUnitsTotal: SafeNonNegativeInt,
      activeAssignments: SafeNonNegativeInt,
      acceptedAssignments: SafeNonNegativeInt,
      failedAssignments: SafeNonNegativeInt,
      staleAssignments: SafeNonNegativeInt,
    }),
    startedAt: S.NullOr(IsoTimestamp),
    updatedAt: S.NullOr(IsoTimestamp),
    closeouts: S.Array(S.Union([
      S.Struct({
        unitRef: PublicRef,
        workClaimRef: PublicRef,
        workerKind: WorkerKind,
        blockerRefs: S.Tuple([]),
        observedAt: IsoTimestamp,
        eventRef: EventRef,
        terminalState: S.Literal("accepted"),
        assignmentRef: PublicRef,
        accountRefHash: AccountRefHash,
        closeoutRef: PublicRef,
        usageEvidence: PylonFleetRunProjectedUsageEvidenceSchema,
      }),
      S.Struct({
        unitRef: PublicRef,
        workClaimRef: PublicRef,
        workerKind: WorkerKind,
        blockerRefs: NonEmptyBlockerRefs,
        observedAt: IsoTimestamp,
        eventRef: EventRef,
        terminalState: S.Literals(["failed", "stale"]),
      }),
      S.Struct({
        unitRef: PublicRef,
        workClaimRef: PublicRef,
        workerKind: WorkerKind,
        blockerRefs: NonEmptyBlockerRefs,
        observedAt: IsoTimestamp,
        eventRef: EventRef,
        terminalState: S.Literals(["failed", "stale"]),
        assignmentRef: PublicRef,
        accountRefHash: AccountRefHash,
        closeoutRef: PublicRef,
        usageEvidence: PylonFleetRunProjectedUsageEvidenceSchema,
      }),
    ])),
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
  const contentLength = response.headers.get("content-length")
  if (contentLength !== null) {
    const declared = Number(contentLength)
    if (
      !/^\d+$/u.test(contentLength) ||
      !Number.isSafeInteger(declared) ||
      declared < 0 ||
      declared > MAX_BATCH_BYTES
    ) throw unavailable()
  }
  const reader = response.body?.getReader()
  if (reader === undefined) throw unavailable()
  const chunks: Uint8Array[] = []
  let byteLength = 0
  try {
    while (true) {
      const next = await reader.read()
      if (next.done) break
      byteLength += next.value.byteLength
      if (!Number.isSafeInteger(byteLength) || byteLength > MAX_BATCH_BYTES) {
        void reader.cancel().catch(() => undefined)
        throw unavailable()
      }
      chunks.push(next.value)
    }
  } catch {
    throw unavailable()
  }
  const bytes = new Uint8Array(byteLength)
  let offset = 0
  for (const chunk of chunks) {
    bytes.set(chunk, offset)
    offset += chunk.byteLength
  }
  const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes)
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
      try {
        if (!PYLON_REF.test(pylonRef) || !RUN_REF.test(runRef)) throw unavailable()
        const decodedBatch = S.decodeUnknownSync(PylonFleetRunExecutionBatchSchema)(batch, {
          onExcessProperty: "error",
        })
        const first = decodedBatch.events[0]
        const last = decodedBatch.events.at(-1)
        if (first === undefined || last === undefined) throw unavailable()
        for (let index = 0; index < decodedBatch.events.length; index += 1) {
          const event = decodedBatch.events[index]!
          if (index > 0 && event.sequence !== decodedBatch.events[index - 1]!.sequence + 1) {
            throw unavailable()
          }
          if (event.eventRef !== eventRefFor(runRef, decodedBatch.claimRef, eventInputFor(event))) {
            throw unavailable()
          }
        }
        const body = canonicalJson(decodedBatch)
        if (new TextEncoder().encode(body).byteLength > MAX_BATCH_BYTES) throw unavailable()
        // Bind idempotency to the exact canonical request, including its
        // durable first/last sequence boundary. The reporter freezes that
        // boundary locally before this call begins.
        const idempotencyDigest = createHash("sha256")
          .update(canonicalJson({ pylonRef, runRef, batch: decodedBatch }))
          .digest("hex")
          .slice(0, 24)
        const response = await fetchImpl(
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
            redirect: "error",
            signal: AbortSignal.timeout(requestTimeoutMs),
          },
        )
        if (!response.ok) {
          void response.body?.cancel().catch(() => undefined)
          throw unavailable()
        }
        const raw = await readBoundedJson(response)
        const ack = S.decodeUnknownSync(PylonFleetRunExecutionAckSchema)(raw, {
          onExcessProperty: "error",
        })
        if (
          ack.runRef !== runRef ||
          ack.claimRef !== decodedBatch.claimRef ||
          ack.acceptedThroughSequence !== last.sequence ||
          ack.execution.lastSequence !== ack.acceptedThroughSequence
        ) throw unavailable()
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

const eventInputFor = (
  event: PylonFleetRunExecutionEvent,
): PylonFleetRunExecutionEventInput => {
  const { eventRef: _eventRef, sequence: _sequence, ...input } = event
  return input as PylonFleetRunExecutionEventInput
}

const eventRefFor = (
  runRef: string,
  claimRef: string,
  event: PylonFleetRunExecutionEventInput,
): string => `event.pylon.fleet_run.${createHash("sha256")
  .update(canonicalJson({ runRef, claimRef, event }))
  .digest("hex")
  .slice(0, 24)}`

const deliveryBatchRefFor = (
  pylonRef: string,
  runRef: string,
  batch: PylonFleetRunExecutionBatch,
): string => `batch.pylon.fleet_run.${createHash("sha256")
  .update(canonicalJson({ pylonRef, runRef, batch }))
  .digest("hex")
  .slice(0, 24)}`

const decodeOutboxEntries = (
  runRef: string,
  claimRef: string,
  entries: readonly FleetRunExecutionOutboxEntry[],
): PylonFleetRunExecutionEvent[] => {
  const events = entries.map(entry => decodeStoredEvent(entry.eventJson))
  for (let index = 0; index < events.length; index += 1) {
    const entry = entries[index]!
    const event = events[index]!
    if (
      entry.claimRef !== claimRef ||
      !Number.isSafeInteger(entry.sequence) ||
      entry.sequence < 1 ||
      event.sequence !== entry.sequence ||
      event.eventRef !== entry.eventRef ||
      event.eventRef !== eventRefFor(runRef, claimRef, eventInputFor(event)) ||
      canonicalJson(event) !== entry.eventJson ||
      (index > 0 && event.sequence !== events[index - 1]!.sequence + 1)
    ) throw unavailable()
  }
  return events
}

const executionBatch = (
  claimRef: string,
  events: readonly PylonFleetRunExecutionEvent[],
): PylonFleetRunExecutionBatch =>
  S.decodeUnknownSync(PylonFleetRunExecutionBatchSchema)({
    schema: PYLON_FLEET_RUN_EXECUTION_BATCH_SCHEMA,
    claimRef,
    events,
  }, { onExcessProperty: "error" })

const batchByteLength = (batch: PylonFleetRunExecutionBatch): number =>
  new TextEncoder().encode(canonicalJson(batch)).byteLength

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
  let accepting = true
  let closed = false
  let closePromise: Promise<void> | null = null
  let tail = Promise.resolve<PylonFleetRunExecutionAck | null>(null)

  const nextPendingBatch = (): PylonFleetRunExecutionBatch | null => {
    const pending = input.store.listFleetRunExecutionOutbox(input.runRef, {
      pendingOnly: true,
      limit: MAX_BATCH_EVENTS,
    })
    if (pending.length === 0) return null
    const decoded = decodeOutboxEntries(input.runRef, claimRef, pending)
    const existingBatchRef = pending[0]!.deliveryBatchRef

    if (existingBatchRef !== null) {
      if (!DELIVERY_BATCH_REF.test(existingBatchRef)) throw unavailable()
      const boundary = pending.findIndex(entry => entry.deliveryBatchRef !== existingBatchRef)
      const end = boundary === -1 ? pending.length : boundary
      const batch = executionBatch(claimRef, decoded.slice(0, end))
      if (
        batchByteLength(batch) > MAX_BATCH_BYTES ||
        deliveryBatchRefFor(input.pylonRef, input.runRef, batch) !== existingBatchRef
      ) throw unavailable()
      return batch
    }

    let selected: PylonFleetRunExecutionBatch | null = null
    for (let length = 1; length <= decoded.length; length += 1) {
      // A later reservation behind an unreserved head violates the one-prefix
      // delivery discipline and must not be silently coalesced.
      if (pending[length - 1]!.deliveryBatchRef !== null) throw unavailable()
      const candidate = executionBatch(claimRef, decoded.slice(0, length))
      if (batchByteLength(candidate) > MAX_BATCH_BYTES) break
      selected = candidate
    }
    // Store events are capped at 64KiB, so one valid event plus envelope must
    // always fit. Fail closed if database corruption violates that invariant.
    if (selected === null) throw unavailable()
    const firstSequence = selected.events[0]!.sequence
    const lastSequence = selected.events.at(-1)!.sequence
    const deliveryBatchRef = deliveryBatchRefFor(
      input.pylonRef,
      input.runRef,
      selected,
    )
    input.store.reserveFleetRunExecutionOutboxBatch({
      runRef: input.runRef,
      claimRef,
      firstSequence,
      lastSequence,
      deliveryBatchRef,
    })
    return selected
  }

  const deliverOne = async (): Promise<PylonFleetRunExecutionAck | null> => {
    if (closed) return null
    const batch = nextPendingBatch()
    if (batch === null) return null
    const ack = await input.remote.append({
      pylonRef: input.pylonRef,
      runRef: input.runRef,
      batch,
    })
    const lastSequence = batch.events.at(-1)?.sequence ?? 0
    // The server may acknowledge exact replay, but it may not advance this
    // local cursor past bytes that were not in the posted batch.
    if (ack.acceptedThroughSequence !== lastSequence) throw unavailable()
    input.store.markFleetRunExecutionOutboxDelivered(
      input.runRef,
      claimRef,
      ack.acceptedThroughSequence,
      readNow(),
    )
    return ack
  }

  const drain = async (): Promise<PylonFleetRunExecutionAck | null> => {
    let lastAck: PylonFleetRunExecutionAck | null = null
    while (true) {
      const next = await deliverOne()
      if (next === null) return lastAck
      lastAck = next
    }
  }

  const flush = (): Promise<PylonFleetRunExecutionAck | null> => {
    if (closed) return Promise.resolve(null)
    const runDrain = async (): Promise<PylonFleetRunExecutionAck | null> => {
      try {
        return await drain()
      } catch {
        throw unavailable()
      }
    }
    const next = tail.then(runDrain, runDrain)
    tail = next
    return next
  }

  return {
    record: async event => {
      if (!accepting || closed) throw unavailable()
      try {
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
      } catch {
        throw unavailable()
      }
      await flush().catch(() => null)
    },
    flush,
    close: () => {
      if (closePromise !== null) return closePromise
      accepting = false
      closePromise = (async () => {
        await flush().catch(() => null)
        closed = true
      })()
      return closePromise
    },
  }
}
