import { createHash } from "node:crypto"
import {
  encodeKhalaFleetIntent,
  FleetSteeringFollowUpCompletionAck,
  FleetSteeringOutcomeAck,
  FleetSteeringPage,
  fleetSteeringOutcomeRefContent,
  type FleetSteeringFollowUpCompletion,
  type FleetSteeringFollowUpCompletionAck as FleetSteeringFollowUpCompletionAckType,
  type FleetSteeringOutcome,
  type KhalaFleetIntent,
} from "@openagentsinc/khala-fleet-intents"
import { Schema as S } from "effect"

import type {
  FleetRunSteeringApplication,
  FleetRunSteeringOutcomeOutboxEntry,
  FleetRunSteeringOutcomeRecord,
  PylonOrchestrationStore,
  WorkClaim,
} from "./store.js"

const MAX_RESPONSE_BYTES = 256 * 1_024
const MAX_PAGE_SIZE = 100
const MAX_PENDING_ACKS = 128
const MAX_PENDING_FOLLOW_UPS = 128
const DEFAULT_INTERVAL_MS = 2_000
const PUBLIC_REF = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u

export type FleetRunSteeringTransportFailure =
  | "not_authorized"
  | "claim_conflict"
  | "invalid_request"
  | "bad_response"
  | "network_failed"
  | "unavailable"

export class PylonFleetRunSteeringTransportError extends Error {
  readonly failure: FleetRunSteeringTransportFailure

  constructor(failure: FleetRunSteeringTransportFailure) {
    super(`Pylon FleetRun steering transport failed: ${failure}`)
    this.name = "PylonFleetRunSteeringTransportError"
    this.failure = failure
  }
}

export type PylonFleetRunSteeringTransport = {
  readonly read: (input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly after: number
    readonly limit: number
  }) => Promise<FleetSteeringPage>
  readonly postOutcomes: (input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly outcomes: ReadonlyArray<FleetSteeringOutcome>
  }) => Promise<FleetSteeringOutcomeAck>
  readonly postCompletions?: (input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly completions: ReadonlyArray<FleetSteeringFollowUpCompletion>
  }) => Promise<FleetSteeringFollowUpCompletionAckType>
}

export type PylonFleetRunSteeringHttpTransport =
  PylonFleetRunSteeringTransport & {
    readonly postCompletions: NonNullable<
      PylonFleetRunSteeringTransport["postCompletions"]
    >
  }

export type MakePylonFleetRunSteeringHttpTransportOptions = {
  readonly agentToken: string
  readonly baseUrl: string
  readonly fetchImpl?: typeof globalThis.fetch | undefined
  readonly requestTimeoutMs?: number | undefined
}

const transportError = (
  failure: FleetRunSteeringTransportFailure,
): PylonFleetRunSteeringTransportError =>
  new PylonFleetRunSteeringTransportError(failure)

const validateBaseUrl = (value: string): URL => {
  try {
    const parsed = new URL(value)
    const loopbackHttp =
      parsed.protocol === "http:" &&
      (parsed.hostname === "localhost" ||
        parsed.hostname === "127.0.0.1" ||
        parsed.hostname === "[::1]")
    if (
      (parsed.protocol !== "https:" && !loopbackHttp) ||
      parsed.username !== "" ||
      parsed.password !== ""
    ) {
      throw transportError("invalid_request")
    }
    return parsed
  } catch (error) {
    if (error instanceof PylonFleetRunSteeringTransportError) throw error
    throw transportError("invalid_request")
  }
}

const readBoundedText = async (response: Response): Promise<string> => {
  const declaredLength = Number(response.headers.get("content-length"))
  if (Number.isFinite(declaredLength) && declaredLength > MAX_RESPONSE_BYTES) {
    throw transportError("bad_response")
  }
  const body = await response.text()
  if (new TextEncoder().encode(body).byteLength > MAX_RESPONSE_BYTES) {
    throw transportError("bad_response")
  }
  return body
}

const mapResponseFailure = (status: number): PylonFleetRunSteeringTransportError => {
  if (status === 400) return transportError("invalid_request")
  if (status === 401 || status === 403 || status === 404) {
    return transportError("not_authorized")
  }
  if (status === 409 || status === 410) return transportError("claim_conflict")
  return transportError("unavailable")
}

/** Agent-bearer HTTP adapter for the accepted FleetRun steering channel. */
export const makePylonFleetRunSteeringHttpTransport = (
  options: MakePylonFleetRunSteeringHttpTransportOptions,
): PylonFleetRunSteeringHttpTransport => {
  const baseUrl = validateBaseUrl(options.baseUrl)
  if (options.agentToken.trim() === "") throw transportError("not_authorized")
  const requestTimeoutMs = options.requestTimeoutMs ?? 15_000
  if (
    !Number.isInteger(requestTimeoutMs) ||
    requestTimeoutMs < 1_000 ||
    requestTimeoutMs > 60_000
  ) {
    throw transportError("invalid_request")
  }
  const fetchImpl = options.fetchImpl ?? globalThis.fetch

  const request = async (
    input: Readonly<{
      pylonRef: string
      runRef: string
      suffix?: "/outcomes" | "/completions"
      query?: URLSearchParams
      init: RequestInit
    }>,
  ): Promise<string> => {
    if (!PUBLIC_REF.test(input.pylonRef) || !PUBLIC_REF.test(input.runRef)) {
      throw transportError("invalid_request")
    }
    const url = new URL(
      `/api/pylons/${encodeURIComponent(input.pylonRef)}/fleet-runs/${encodeURIComponent(input.runRef)}/steering${input.suffix ?? ""}`,
      baseUrl,
    )
    if (input.query !== undefined) url.search = input.query.toString()
    let response: Response
    try {
      response = await fetchImpl(url, {
        ...input.init,
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${options.agentToken}`,
          ...(input.init.headers ?? {}),
        },
        signal: AbortSignal.timeout(requestTimeoutMs),
      })
    } catch {
      throw transportError("network_failed")
    }
    if (!response.ok) throw mapResponseFailure(response.status)
    return await readBoundedText(response)
  }

  return {
    read: async ({ pylonRef, runRef, claimRef, after, limit }) => {
      const query = new URLSearchParams({
        claimRef,
        after: String(after),
        limit: String(limit),
      })
      let raw: string
      try {
        raw = await request({
          pylonRef,
          runRef,
          query,
          init: { method: "GET" },
        })
      } catch (error) {
        if (error instanceof PylonFleetRunSteeringTransportError) throw error
        throw transportError("unavailable")
      }
      try {
        return S.decodeUnknownSync(S.fromJsonString(FleetSteeringPage))(raw, {
          onExcessProperty: "error",
        })
      } catch {
        throw transportError("bad_response")
      }
    },
    postOutcomes: async ({ pylonRef, runRef, claimRef, outcomes }) => {
      let raw: string
      try {
        raw = await request({
          pylonRef,
          runRef,
          suffix: "/outcomes",
          init: {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ claimRef, outcomes }),
          },
        })
      } catch (error) {
        if (error instanceof PylonFleetRunSteeringTransportError) throw error
        throw transportError("unavailable")
      }
      try {
        return S.decodeUnknownSync(S.fromJsonString(FleetSteeringOutcomeAck))(raw, {
          onExcessProperty: "error",
        })
      } catch {
        throw transportError("bad_response")
      }
    },
    postCompletions: async ({ pylonRef, runRef, claimRef, completions }) => {
      let raw: string
      try {
        raw = await request({
          pylonRef,
          runRef,
          suffix: "/completions",
          init: {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({ claimRef, completions }),
          },
        })
      } catch (error) {
        if (error instanceof PylonFleetRunSteeringTransportError) throw error
        throw transportError("unavailable")
      }
      try {
        const ack = S.decodeUnknownSync(
          S.fromJsonString(FleetSteeringFollowUpCompletionAck),
        )(raw, { onExcessProperty: "error" })
        if (
          ack.runRef !== runRef ||
          ack.claimRef !== claimRef ||
          ack.completions.length !== completions.length ||
          ack.storedCompletionCount + ack.duplicateCompletionCount !==
            completions.length ||
          ack.completions.some((completion, index) => {
            const expected = completions[index]
            return expected === undefined ||
              completion.seq !== expected.seq ||
              completion.intentId !== expected.intentId ||
              completion.state !== expected.state ||
              completion.completionRef !== expected.completionRef ||
              completion.completedAt !== expected.completedAt
          })
        ) {
          throw transportError("bad_response")
        }
        return ack
      } catch (error) {
        if (error instanceof PylonFleetRunSteeringTransportError) throw error
        throw transportError("bad_response")
      }
    },
  }
}

const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== "object") return JSON.stringify(value) ?? "null"
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`
  const record = value as Record<string, unknown>
  return `{${Object.keys(record).sort().map(key =>
    `${JSON.stringify(key)}:${canonicalJson(record[key])}`
  ).join(",")}}`
}

const digestIntent = (intent: KhalaFleetIntent): string =>
  createHash("sha256")
    .update(canonicalJson(encodeKhalaFleetIntent(intent)))
    .digest("hex")

const outcomeRefFor = (input: {
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly seq: number
  readonly intentId: string
  readonly outcome: FleetRunSteeringOutcomeRecord["outcome"]
  readonly observedAt: string
}): string =>
  `outcome.pylon.fleet_steering.${createHash("sha256")
    .update(canonicalJson(fleetSteeringOutcomeRefContent({
      runRef: input.runRef,
      claimRef: input.claimRef,
      pylonRef: input.pylonRef,
      seq: input.seq,
      intentId: input.intentId,
      outcome: input.outcome,
      observedAt: input.observedAt,
    })))
    .digest("hex")
    .slice(0, 24)}`

type ExactTarget = {
  readonly workUnitRef: string
  readonly workClaimRef: string
  readonly assignmentRef: string
  readonly targetRef: string
}

const exactTargetFromClaim = (
  claim: WorkClaim,
  targetRef: string,
): ExactTarget | "terminal" | "incomplete" => {
  if (claim.state === "released" || claim.state === "expired" || claim.state === "closeout") {
    return "terminal"
  }
  if (claim.assignmentRef === null || claim.assignmentRef.trim() === "") {
    return "incomplete"
  }
  return {
    workUnitRef: claim.workUnitRef,
    workClaimRef: claim.claimRef,
    assignmentRef: claim.assignmentRef,
    targetRef,
  }
}

const resolveExactTarget = (
  store: PylonOrchestrationStore,
  runRef: string,
  targetRef: string,
): ExactTarget | "missing" | "ambiguous" | "terminal" | "incomplete" => {
  const matches = store.listWorkClaims({ runRef }).filter(claim =>
    claim.claimRef === targetRef ||
    claim.assignmentRef === targetRef
  )
  if (matches.length === 0) return "missing"
  // Never choose "latest" when a work-unit ref names multiple historical
  // claims. Sarah must send the exact claim or assignment ref instead.
  if (matches.length !== 1) return "ambiguous"
  return exactTargetFromClaim(matches[0]!, targetRef)
}

const targetApplication = (
  target: ReturnType<typeof resolveExactTarget>,
  intent: Extract<KhalaFleetIntent, { kind: "approval_decision" | "steer_message" }>,
): FleetRunSteeringApplication => {
  if (target === "terminal") {
    return { outcome: "skipped_stale" }
  }
  if (target === "missing" || target === "ambiguous" || target === "incomplete") {
    return { outcome: "rejected" }
  }
  return {
    outcome: "queued_follow_up",
    queuedFollowUp: {
      ...target,
      intentKind: intent.kind,
      approvalRef: intent.kind === "approval_decision" ? intent.approvalRef : null,
      decision: intent.kind === "approval_decision" ? intent.decision : null,
      body: intent.kind === "steer_message" ? (intent.body ?? null) : null,
      bodyRef: intent.kind === "steer_message" ? (intent.bodyRef ?? null) : null,
      residualRefs: [],
    },
  }
}

const approvalApplication = (
  store: PylonOrchestrationStore,
  runRef: string,
  intent: Extract<KhalaFleetIntent, { kind: "approval_decision" }>,
): FleetRunSteeringApplication => {
  const binding = store.getFleetRunSteeringApprovalBinding(intent.approvalRef)
  if (binding === null) return { outcome: "rejected" }
  if (binding.runRef !== runRef) return { outcome: "rejected" }
  if (binding.state !== "pending") return { outcome: "skipped_stale" }
  const claim = store.getWorkClaim(binding.workClaimRef)
  if (
    claim === null ||
    claim.runRef !== binding.runRef ||
    claim.workUnitRef !== binding.workUnitRef ||
    claim.assignmentRef !== binding.assignmentRef
  ) return { outcome: "rejected" }
  if (claim.state === "released" || claim.state === "expired" || claim.state === "closeout") {
    return { outcome: "skipped_stale" }
  }
  return {
    outcome: "queued_follow_up",
    queuedFollowUp: {
      workUnitRef: binding.workUnitRef,
      workClaimRef: binding.workClaimRef,
      assignmentRef: binding.assignmentRef,
      targetRef: intent.approvalRef,
      intentKind: "approval_decision",
      approvalRef: intent.approvalRef,
      decision: intent.decision,
      body: null,
      bodyRef: null,
      residualRefs: [],
    },
  }
}

const applyIntent = (
  store: PylonOrchestrationStore,
  runRef: string,
  intent: KhalaFleetIntent,
  now: Date,
): FleetRunSteeringApplication => {
  if (intent.runRef !== runRef) return { outcome: "rejected" }
  if (intent.kind === "fleet_run_control") {
    const run = store.getFleetRun(runRef)
    if (run === null) return { outcome: "skipped_stale" }
    if (intent.action === "stop") {
      if (run.state === "completed") return { outcome: "skipped_stale" }
      const liveClaims = store.listLiveWorkClaims(now)
        .filter(claim => claim.runRef === runRef && claim.state !== "closeout")
      const residualRefs = liveClaims.flatMap(claim => [
        claim.claimRef,
        ...(claim.assignmentRef === null ? [] : [claim.assignmentRef]),
      ])
      if (residualRefs.length > 128) {
        throw new Error("fleet run steering private follow-up backpressure")
      }
      if (run.state !== "stopped") {
        store.updateFleetRunState(runRef, "stopped", now, "operator")
      }
      return liveClaims.length === 0
        ? { outcome: "applied" }
        : {
            outcome: "queued_follow_up",
            queuedFollowUp: {
              workUnitRef: null,
              workClaimRef: null,
              assignmentRef: null,
              targetRef: null,
              intentKind: "fleet_run_control",
              approvalRef: null,
              decision: null,
              body: null,
              bodyRef: null,
              residualRefs,
            },
          }
    }
    {
      const desired =
        intent.action === "pause" ? "paused" :
        intent.action === "resume" ? "running" :
        "draining"
      if (run.state === desired) return { outcome: "applied" }
      if (run.state === "completed" || run.state === "stopped") {
        return { outcome: "skipped_stale" }
      }
      const valid =
        (intent.action === "pause" && run.state === "running") ||
        (intent.action === "resume" && run.state === "paused") ||
        (intent.action === "drain" && (run.state === "running" || run.state === "paused"))
      if (!valid) return { outcome: "skipped_stale" }
      store.updateFleetRunState(runRef, desired, now, "operator")
      return { outcome: "applied" }
    }
  }
  if (intent.kind === "steer_message") {
    if (intent.targetRef === undefined) return { outcome: "rejected" }
    return targetApplication(
      resolveExactTarget(store, runRef, intent.targetRef),
      intent,
    )
  }
  if (intent.kind === "approval_decision") {
    return approvalApplication(store, runRef, intent)
  }
  return { outcome: "rejected" }
}

const sameOutcome = (
  left: FleetSteeringOutcome,
  right: FleetSteeringOutcome,
): boolean =>
  left.seq === right.seq &&
  left.intentId === right.intentId &&
  left.outcome === right.outcome &&
  left.outcomeRef === right.outcomeRef &&
  left.observedAt === right.observedAt

const toWireOutcome = (
  entry: FleetRunSteeringOutcomeRecord,
): FleetSteeringOutcome => ({
  seq: entry.seq,
  intentId: entry.intentId,
  outcome: entry.outcome,
  outcomeRef: entry.outcomeRef,
  observedAt: entry.observedAt,
})

export type PylonFleetRunSteeringConsumerOptions = {
  readonly store: PylonOrchestrationStore
  readonly transport: PylonFleetRunSteeringTransport
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly now?: (() => Date) | undefined
  readonly limit?: number | undefined
  readonly intervalMs?: number | undefined
  readonly startImmediately?: boolean | undefined
}

export type PylonFleetRunSteeringTickResult = {
  readonly ok: boolean
  readonly applied: number
  readonly acknowledged: number
  readonly pendingAcknowledgements: number
  readonly watermark: number
  readonly failure:
    | FleetRunSteeringTransportFailure
    | "invalid_page"
    | "local_store_failed"
    | "backpressure"
    | null
}

const flushOutbox = async (
  options: PylonFleetRunSteeringConsumerOptions,
): Promise<number> => {
  const pending = options.store.listFleetRunSteeringOutcomeOutbox({
    pylonRef: options.pylonRef,
    runRef: options.runRef,
    claimRef: options.claimRef,
    limit: Math.min(64, options.limit ?? MAX_PAGE_SIZE),
  })
  if (pending.length === 0) return 0
  const outcomes = pending.map(toWireOutcome)
  const ack = await options.transport.postOutcomes({
    pylonRef: options.pylonRef,
    runRef: options.runRef,
    claimRef: options.claimRef,
    outcomes,
  })
  if (
    ack.runRef !== options.runRef ||
    ack.claimRef !== options.claimRef ||
    ack.outcomes.length !== outcomes.length ||
    ack.outcomes.some((outcome, index) =>
      outcomes[index] === undefined || !sameOutcome(outcome, outcomes[index]!)
    ) ||
    ack.storedOutcomeCount + ack.duplicateOutcomeCount !== outcomes.length
  ) {
    throw transportError("bad_response")
  }
  options.store.markFleetRunSteeringOutcomeOutboxDelivered(
    pending,
    options.now?.() ?? new Date(),
  )
  return pending.length
}

export const tickPylonFleetRunSteeringConsumer = async (
  options: PylonFleetRunSteeringConsumerOptions,
): Promise<PylonFleetRunSteeringTickResult> => {
  const limit = Math.max(1, Math.min(MAX_PAGE_SIZE, Math.trunc(options.limit ?? MAX_PAGE_SIZE)))
  let acknowledged = 0
  try {
    acknowledged += await flushOutbox(options)
  } catch (error) {
    // An ACK outage retains the durable outbox. Never ingest later outcomes
    // while an older ACK remains pending.
    if (!(error instanceof PylonFleetRunSteeringTransportError)) {
      return {
        ok: false,
        applied: 0,
        acknowledged: 0,
        pendingAcknowledgements: options.store.listFleetRunSteeringOutcomeOutbox({
          pylonRef: options.pylonRef,
          runRef: options.runRef,
          claimRef: options.claimRef,
        }).length,
        watermark: options.store.getFleetRunSteeringWatermark(
          options.pylonRef,
          options.runRef,
          options.claimRef,
        ),
        failure: "local_store_failed",
      }
    }
  }

  const pendingBeforeRead = options.store.listFleetRunSteeringOutcomeOutbox({
    pylonRef: options.pylonRef,
    runRef: options.runRef,
    claimRef: options.claimRef,
    limit: MAX_PENDING_ACKS,
  }).length
  const queuedBeforeRead = options.store.listFleetRunSteeringQueuedFollowUps({
    pylonRef: options.pylonRef,
    runRef: options.runRef,
    claimRef: options.claimRef,
  }).length
  const pendingCompletionsBeforeRead = options.store
    .listFleetRunSteeringFollowUpCompletionOutbox({
      pylonRef: options.pylonRef,
      runRef: options.runRef,
      claimRef: options.claimRef,
      limit: MAX_PENDING_FOLLOW_UPS,
    }).length
  if (
    pendingBeforeRead > 0 ||
    queuedBeforeRead >= MAX_PENDING_FOLLOW_UPS ||
    pendingCompletionsBeforeRead >= MAX_PENDING_FOLLOW_UPS
  ) {
    return {
      ok: false,
      applied: 0,
      acknowledged,
      pendingAcknowledgements: pendingBeforeRead,
      watermark: options.store.getFleetRunSteeringWatermark(
        options.pylonRef,
        options.runRef,
        options.claimRef,
      ),
      failure: "backpressure",
    }
  }

  const watermark = options.store.getFleetRunSteeringWatermark(
    options.pylonRef,
    options.runRef,
    options.claimRef,
  )
  let page: FleetSteeringPage
  try {
    page = await options.transport.read({
      pylonRef: options.pylonRef,
      runRef: options.runRef,
      claimRef: options.claimRef,
      after: watermark,
      limit,
    })
  } catch (error) {
    return {
      ok: false,
      applied: 0,
      acknowledged,
      pendingAcknowledgements: options.store.listFleetRunSteeringOutcomeOutbox({
        pylonRef: options.pylonRef,
        runRef: options.runRef,
        claimRef: options.claimRef,
      }).length,
      watermark,
      failure: error instanceof PylonFleetRunSteeringTransportError
        ? error.failure
        : "unavailable",
    }
  }

  const ordered = page.intents
  const seenIntentIds = new Set<string>()
  const invalidDelivery = ordered.some((item, index) => {
    if (seenIntentIds.has(item.intentId)) return true
    seenIntentIds.add(item.intentId)
    return item.seq <= watermark ||
      item.intent.intentId !== item.intentId ||
      item.intent.runRef !== options.runRef ||
      (index > 0 && ordered[index - 1]!.seq >= item.seq)
  })
  const invalidPage =
    page.runRef !== options.runRef ||
    page.claimRef !== options.claimRef ||
    ordered.length > limit ||
    page.nextAfter < watermark ||
    invalidDelivery ||
    (ordered.at(-1)?.seq ?? watermark) !== page.nextAfter ||
    (!page.upToDate && ordered.length === 0 && page.nextAfter === watermark)
  if (invalidPage) {
    return {
      ok: false,
      applied: 0,
      acknowledged,
      pendingAcknowledgements: options.store.listFleetRunSteeringOutcomeOutbox({
        pylonRef: options.pylonRef,
        runRef: options.runRef,
        claimRef: options.claimRef,
      }).length,
      watermark,
      failure: "invalid_page",
    }
  }

  let applied = 0
  try {
    for (const delivery of ordered) {
      const intentDigest = digestIntent(delivery.intent)
      const result = options.store.applyFleetRunSteeringIntent(
        {
          pylonRef: options.pylonRef,
          runRef: options.runRef,
          claimRef: options.claimRef,
          seq: delivery.seq,
          intentId: delivery.intentId,
          intentKind: delivery.intent.kind,
          intentDigest,
          observedAt: options.now?.() ?? new Date(),
          outcomeRefFor: (outcome, observedAt) => outcomeRefFor({
            pylonRef: options.pylonRef,
            runRef: options.runRef,
            claimRef: options.claimRef,
            seq: delivery.seq,
            intentId: delivery.intentId,
            outcome,
            observedAt,
          }),
        },
        () => applyIntent(
          options.store,
          options.runRef,
          delivery.intent,
          options.now?.() ?? new Date(),
        ),
      )
      if (result.recorded) applied += 1
    }
  } catch (error) {
    return {
      ok: false,
      applied,
      acknowledged,
      pendingAcknowledgements: options.store.listFleetRunSteeringOutcomeOutbox({
        pylonRef: options.pylonRef,
        runRef: options.runRef,
        claimRef: options.claimRef,
      }).length,
      watermark: options.store.getFleetRunSteeringWatermark(
        options.pylonRef,
        options.runRef,
        options.claimRef,
      ),
      failure: error instanceof Error && error.message.includes("backpressure")
        ? "backpressure"
        : "local_store_failed",
    }
  }

  try {
    acknowledged += await flushOutbox(options)
  } catch {
    // Durable pending ACKs are an honest degraded result, not lost work.
  }
  const pendingAcknowledgements = options.store.listFleetRunSteeringOutcomeOutbox({
    pylonRef: options.pylonRef,
    runRef: options.runRef,
    claimRef: options.claimRef,
  }).length
  return {
    ok: pendingAcknowledgements === 0,
    applied,
    acknowledged,
    pendingAcknowledgements,
    watermark: options.store.getFleetRunSteeringWatermark(
      options.pylonRef,
      options.runRef,
      options.claimRef,
    ),
    failure: pendingAcknowledgements === 0 ? null : "backpressure",
  }
}

export type PylonFleetRunSteeringConsumer = {
  readonly tick: () => Promise<PylonFleetRunSteeringTickResult>
  readonly close: () => Promise<void>
}

export const openPylonFleetRunSteeringConsumer = (
  options: PylonFleetRunSteeringConsumerOptions,
): PylonFleetRunSteeringConsumer => {
  const intervalMs = options.intervalMs ?? DEFAULT_INTERVAL_MS
  if (!Number.isInteger(intervalMs) || intervalMs < 250 || intervalMs > 60_000) {
    throw new Error("Pylon FleetRun steering interval must be between 250 and 60000ms")
  }
  let closed = false
  let timer: ReturnType<typeof setInterval> | null = null
  let inFlight: Promise<PylonFleetRunSteeringTickResult> | null = null
  const tick = (): Promise<PylonFleetRunSteeringTickResult> => {
    if (closed) {
      return Promise.resolve({
        ok: false,
        applied: 0,
        acknowledged: 0,
        pendingAcknowledgements: options.store.listFleetRunSteeringOutcomeOutbox({
          pylonRef: options.pylonRef,
          runRef: options.runRef,
          claimRef: options.claimRef,
        }).length,
        watermark: options.store.getFleetRunSteeringWatermark(
          options.pylonRef,
          options.runRef,
          options.claimRef,
        ),
        failure: "unavailable",
      })
    }
    if (inFlight !== null) return inFlight
    const current = tickPylonFleetRunSteeringConsumer(options)
    inFlight = current
    void current.finally(() => {
      if (inFlight === current) inFlight = null
    })
    return current
  }
  timer = setInterval(() => void tick(), intervalMs)
  timer.unref?.()
  if (options.startImmediately !== false) void tick()
  return {
    tick,
    close: async () => {
      closed = true
      if (timer !== null) clearInterval(timer)
      timer = null
      await inFlight?.catch(() => undefined)
    },
  }
}

export type PylonFleetRunSteeringConsumerFactory = (input: Readonly<{
  store: PylonOrchestrationStore
  pylonRef: string
  runRef: string
  claimRef: string
}>) => PylonFleetRunSteeringConsumer | Promise<PylonFleetRunSteeringConsumer>
