// Khala chat-completions -> ATIF trace emitter (openagents #6214, epic #6206).
//
// This module makes the trace primitive universal for the FIRST non-QA surface:
// a completed Khala chat-completions session (the user/assistant turns + any
// tool calls) is mapped to a PUBLIC-SAFE ATIF trajectory (`AtifTrajectory`,
// ATIF-v1.7, the pinned `atif-trace-schema.ts` shape) and, opt-in, persisted via
// the existing trace store so the session becomes a shareable `/trace/{uuid}`.
//
// Design rules (all enforced here):
//   - GATEWAY PROJECTION: `agent.model_name` and the assistant step `model_name`
//     is the public Khala id `openagents/khala`. We NEVER leak the raw served
//     backend (Vertex / Fireworks / Hydralisk adapter ids) into the trace — the
//     same public-safe projection the Khala gateway already applies to responses.
//   - FLAG-GATED, DEFAULT OFF: `emitKhalaChatTrace` is an honest no-op unless the
//     emit flag is on AND the caller explicitly opted in for the request. With
//     the flag off it returns `{ emitted: false, reason: 'disabled' }` and does
//     NO work (no store call), so `/v1/chat/completions` is byte-for-byte
//     unchanged on the default path.
//   - NEVER BYPASS the ingest contract: before persistence we run the SAME
//     structural validator (`validateAtifTrajectory`) and public-safety tripwire
//     (`atifTraceTripwire`) the `POST /api/traces` ingest runs. A trajectory that
//     fails either is rejected (not stored, not redacted) and the emitter returns
//     an honest failure reason. We persist through the shared `TraceStore`
//     (`createTrace`), the same store the ingest route uses.
//   - VISIBILITY default `unlisted`; OWNER is the requesting agent/user when
//     authenticated (the `agent:<id>` / `user:<id>` ref the ingest route uses).
//   - FIRE-AND-FORGET / FAIL-SOFT at the call site: the completion is already
//     delivered to the client; a trace-emit failure must never break or delay it.
//
// A trace is evidence only. Emitting one grants no accepted-work, payout,
// settlement, or public-claim authority. The data-market reward marker stays
// INERT (consent withheld, no reward eligibility) on this gateway path; the
// upload data market (#6221) is the surface that arms rewards behind its own
// owner-gated flag.
//
// Autopilot work orders (#6214) and Pylon worker sessions (#6214) are explicit
// FOLLOW-UPS: each reuses this same emitter shape over its own session record.
// They are intentionally NOT built here (worker-only Khala-chat slice).

import {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifStep,
  AtifTrajectory,
  type TraceVisibility,
  atifTraceTripwire,
  validateAtifTrajectory,
} from '../atif-trace-schema'
import { currentIsoTimestamp, randomUuid } from '../runtime-primitives'
import type { TraceStore, TraceUploadSource } from '../trace-store-d1'
import { isKhalaModel } from './pricing'
import type { InferenceMessage, InferenceResult } from './provider-adapter'
import {
  redactTraceValue,
  type TraceRedactionReport,
} from './trace-redaction'

// The visibility an AUTO-CAPTURED (non-opted-in) free-tier trace is stored at:
// `owner_only` (#6294 storage half). A trace the user never chose to share must
// not be link-reachable by uuid guess/leak — it renders only to the owning
// session/admin until the owner explicitly opts it into sharing. An EXPLICIT
// per-request opt-in keeps the historical `unlisted` default (the caller asked
// for a shareable link).
export const KHALA_AUTO_CAPTURE_VISIBILITY = 'owner_only' as const

// The single public Khala gateway model id projected into every emitted trace.
// We NEVER emit a raw served backend id.
export const KHALA_TRACE_MODEL_NAME = 'openagents/khala' as const

// The agent name surfaced on the trace. Khala is the OpenAgents coding/chat
// agent; this is a stable public label, not a backend id.
const KHALA_TRACE_AGENT_NAME = 'Khala' as const
const KHALA_TRACE_AGENT_VERSION = 'gateway-1' as const

// Parse the emit flag value. Default OFF: anything other than an explicit truthy
// token leaves the emitter inert (mirrors `isInferenceGatewayEnabled`).
export const isKhalaChatTraceEmitEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// Parse the DEFAULT-ON free-tier CAPTURE flag (#6293). SEPARATE from the master
// emit flag above so the flip is staged independently. DEFAULT OFF: anything
// other than an explicit truthy token leaves default-on capture inert (only
// explicitly-opted-in requests are captured). The Worker reads
// `env.KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT`.
export const isKhalaFreeTierTraceCaptureDefaultEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}

// Resolve whether this request opted in to trace emission. Deterministic parse of
// an explicit caller-supplied switch (header `x-oa-emit-trace: on`, or a truthy
// `oa_emit_trace` body field) — never an intent inference. Bounded boolean field.
export const resolveKhalaChatTraceOptIn = (
  input: Readonly<{
    request: Request
    rawBody: Record<string, unknown>
  }>,
): boolean => {
  const header = input.request.headers
    .get('x-oa-emit-trace')
    ?.trim()
    .toLowerCase()
  const headerOptIn =
    header !== undefined && ['1', 'true', 'yes', 'on'].includes(header)
  const bodyField = input.rawBody['oa_emit_trace']
  const bodyOptIn =
    bodyField === true ||
    (typeof bodyField === 'string' &&
      ['1', 'true', 'yes', 'on'].includes(bodyField.trim().toLowerCase()))
  return headerOptIn || bodyOptIn
}

// A completed Khala chat session, expressed in the data the chat handler already
// holds after a non-streaming completion: the request messages it dispatched and
// the (identity-guarded) provider result.
export type CompletedKhalaChatSession = Readonly<{
  // The effective requested model (e.g. `openagents/khala`). Used only to gate
  // that this is a Khala session; the EMITTED model id is always the public
  // gateway projection, never a raw backend.
  requestedModel: string
  // The request messages dispatched to the provider. Gateway-injected system
  // blocks (identity prompt, catalog, acceptance contract) are dropped from the
  // trace — a trace records the CONVERSATION, not our internal prompt scaffolding.
  requestMessages: ReadonlyArray<InferenceMessage>
  // The final, identity-guarded assistant result.
  result: InferenceResult
  // The chat response id (`chatcmpl-...`); used as the ATIF session id.
  responseId: string
}>

// System prompt prefixes the gateway injects (identity / catalog / acceptance
// contract). A trace records the conversation, not our internal scaffolding, so
// we drop any system message that begins with one of these markers. Client-sent
// system messages that don't match a marker are kept (they're real session
// content the caller chose to send).
const GATEWAY_SYSTEM_PROMPT_MARKERS: ReadonlyArray<string> = [
  'You are Khala',
  'Khala is',
  '# Khala',
  // Acceptance-contract guidance and component catalog both lead with these.
  'ACCEPTANCE CONTRACT',
  'COMPONENT CATALOG',
  'oa-component',
]

const isGatewayInjectedSystemMessage = (content: string): boolean => {
  const trimmed = content.trimStart()
  return GATEWAY_SYSTEM_PROMPT_MARKERS.some(marker => trimmed.startsWith(marker))
}

// Map an OpenAI-style chat role onto the ATIF step source enum. `assistant` ->
// `agent`; `system`/`developer` -> `system`; everything else (`user`, `tool`,
// unknown) is treated as `user` (an inbound turn the agent responds to).
const stepSourceForRole = (role: string): AtifStep['source'] => {
  const normalized = role.trim().toLowerCase()
  if (normalized === 'assistant') {
    return 'agent'
  }
  if (normalized === 'system' || normalized === 'developer') {
    return 'system'
  }
  return 'user'
}

/**
 * Pure mapper: a completed Khala chat session -> a public-safe `AtifTrajectory`.
 *
 * - One step per conversation turn (gateway-injected system prompts dropped),
 *   with sequential `step_id` from 1, followed by the final assistant step
 *   carrying the completion content + usage metrics.
 * - `agent.model_name` (and the assistant step `model_name`) is always
 *   `openagents/khala` — the gateway projection, never a raw backend id.
 * - `final_metrics` is derived from the provider usage object.
 *
 * The result is NOT yet validated/tripwired here; `emitKhalaChatTrace` (or the
 * ingest route) does that before persistence. This stays a pure function so it
 * is trivially testable and reusable by the Autopilot/Pylon follow-ups.
 */
export const khalaChatSessionToAtifTrajectory = (
  session: CompletedKhalaChatSession,
  options?: Readonly<{
    trajectoryId?: string | undefined
    visibility?: TraceVisibility | undefined
  }>,
): AtifTrajectory => {
  const conversationSteps: Array<AtifStep> = []
  let stepId = 1

  for (const message of session.requestMessages) {
    const source = stepSourceForRole(message.role)
    // Drop gateway-injected system scaffolding; keep real client system messages.
    if (source === 'system' && isGatewayInjectedSystemMessage(message.content)) {
      continue
    }
    conversationSteps.push(
      new AtifStep({
        step_id: stepId,
        source,
        message: message.content,
      }),
    )
    stepId += 1
  }

  // The final assistant turn: the completion content, attributed to the Khala
  // gateway model, with per-step token metrics from the provider usage.
  const finalStep = new AtifStep({
    step_id: stepId,
    source: 'agent',
    message: session.result.content,
    model_name: KHALA_TRACE_MODEL_NAME,
    metrics: {
      prompt_tokens: session.result.usage.promptTokens,
      completion_tokens: session.result.usage.completionTokens,
    },
  })

  return new AtifTrajectory({
    schema_version: ATIF_PINNED_SCHEMA_VERSION,
    trajectory_id: options?.trajectoryId ?? session.responseId,
    session_id: session.responseId,
    visibility: options?.visibility ?? 'unlisted',
    agent: {
      name: KHALA_TRACE_AGENT_NAME,
      version: KHALA_TRACE_AGENT_VERSION,
      model_name: KHALA_TRACE_MODEL_NAME,
    },
    steps: [...conversationSteps, finalStep],
    final_metrics: {
      total_prompt_tokens: session.result.usage.promptTokens,
      total_completion_tokens: session.result.usage.completionTokens,
      total_steps: conversationSteps.length + 1,
    },
  })
}

// The resolved owner of an emitted trace (the requesting agent/user). Mirrors the
// ingest route's `TraceUploader` ownership: an authenticated agent owns
// `agent:<id>`; a user session owns `user:<id>`.
export type KhalaChatTraceOwner = Readonly<{
  ownerUserId: string
  agentRef: string
  uploadSource: TraceUploadSource
}>

export type EmitKhalaChatTraceDeps = Readonly<{
  // Whether the Khala-chat trace emitter is enabled (the parsed flag). Default
  // OFF: the Worker passes
  // `isKhalaChatTraceEmitEnabled(env.KHALA_CHAT_TRACE_EMIT_ENABLED)`.
  enabled: boolean
  // Did THIS request opt in (header / body switch)? Persist when the master flag
  // is on AND (the request opted in OR `captureDefault` is true).
  optedIn: boolean
  // DEFAULT-ON CAPTURE (#6293). When true, this completion is captured even
  // WITHOUT a per-request opt-in — the free-tier-default policy resolved at the
  // chat seam (`captureDefault = freeTier.free && !paidPrivacy`, fail-closed to
  // NOT-captured). Default false: with neither `optedIn` nor `captureDefault`,
  // the emitter is the historical honest no-op (`not_opted_in`). An auto-capture
  // (captureDefault, not optedIn) is stored `owner_only` (private-by-default);
  // an explicit opt-in keeps `unlisted`.
  captureDefault?: boolean | undefined
  // The shared trace store (the SAME `TraceStore` the ingest route persists
  // through). Absent => honest no-op (nothing to persist into).
  store: TraceStore | undefined
  // The resolved owner of the trace. Absent => no-op (we never store an
  // unowned/anonymous gateway trace).
  owner: KhalaChatTraceOwner | undefined
  // Default visibility for an emitted trace. Defaults to `unlisted`.
  visibility?: TraceVisibility | undefined
  // Deterministic id/clock injection for tests.
  makeId?: (() => string) | undefined
  nowIso?: (() => string) | undefined
}>

export type EmitKhalaChatTraceResult =
  | Readonly<{
      emitted: false
      reason:
        | 'disabled'
        | 'not_opted_in'
        | 'not_khala'
        | 'no_store'
        | 'no_owner'
        | 'invalid_trajectory'
        | 'public_safety_rejected'
        | 'redaction_residual_drop'
        | 'store_error'
      detail?: string | undefined
      // Public-safe redaction report (category -> count + total) attached on the
      // capture path so a caller/log can see a trace was scrubbed WITHOUT ever
      // surfacing a redacted VALUE. Present whenever redaction ran.
      redactionReport?: TraceRedactionReport | undefined
    }>
  | Readonly<{
      emitted: true
      uuid: string
      url: string
      redactionReport?: TraceRedactionReport | undefined
    }>

/**
 * Flag-gated persist of a completed Khala chat session as an ATIF trace.
 *
 * Honest no-op (no store call, no work) when the master flag is off, the request
 * neither opted in NOR qualifies for default-on free-tier capture, the session
 * is not a Khala model, or no store/owner is wired. When all gates pass it maps
 * the session, REDACTS the trajectory (the primary scrubber — secrets, wallet,
 * PII, paths), then runs the SAME structural validator + public-safety tripwire
 * the ingest route runs as the FAIL-CLOSED BACKSTOP (a trace that still trips
 * AFTER redaction is dropped, never stored), and persists through the shared
 * `TraceStore`, returning the `{uuid}` + the shareable `/trace/{uuid}` url.
 *
 * Visibility: an EXPLICIT per-request opt-in keeps the historical `unlisted`
 * default (the caller wants a shareable link). An AUTO-CAPTURE (the default-on
 * free-tier path, no explicit opt-in) is stored `owner_only` — private by
 * default (#6294), not even link-reachable until the owner opts to share.
 *
 * This NEVER throws: the call site treats trace emission as fire-and-forget over
 * an already-delivered completion. All failures are returned as a typed reason.
 */
export const emitKhalaChatTrace = async (
  session: CompletedKhalaChatSession,
  deps: EmitKhalaChatTraceDeps,
): Promise<EmitKhalaChatTraceResult> => {
  if (!deps.enabled) {
    return { emitted: false, reason: 'disabled' }
  }
  const captureDefault = deps.captureDefault === true
  // DEFAULT-ON CAPTURE (#6293): persist when the master flag is on AND
  // (this request opted in OR it qualifies for the free-tier capture default).
  if (!deps.optedIn && !captureDefault) {
    return { emitted: false, reason: 'not_opted_in' }
  }
  // The emitter only projects Khala sessions (it is a Khala gateway capability).
  if (!isKhalaModel(session.requestedModel)) {
    return { emitted: false, reason: 'not_khala' }
  }
  if (deps.store === undefined) {
    return { emitted: false, reason: 'no_store' }
  }
  if (deps.owner === undefined) {
    return { emitted: false, reason: 'no_owner' }
  }

  // PRIVATE-BY-DEFAULT (#6294): an auto-captured trace (no explicit opt-in) is
  // `owner_only`. An explicit caller opt-in (or a caller-set `deps.visibility`)
  // keeps the historical default.
  const visibility: TraceVisibility =
    deps.visibility ??
    (deps.optedIn ? 'unlisted' : KHALA_AUTO_CAPTURE_VISIBILITY)
  const mapped = khalaChatSessionToAtifTrajectory(session, { visibility })

  // SAME structural validation the ingest route runs. A malformed projection is
  // rejected, never stored. (Run on the mapped trajectory; redaction below only
  // rewrites string LEAVES, never structure, so validity is preserved.)
  const structuralIssues = validateAtifTrajectory(mapped)
  if (structuralIssues.length > 0) {
    return {
      emitted: false,
      reason: 'invalid_trajectory',
      detail: structuralIssues.map(issue => issue.code).join(','),
    }
  }

  // REDACTION (#6219, the PRIMARY scrubber). For default-on capture of real
  // free-tier traffic, "drop the whole trace if it contains an email or a
  // secret-shaped token" would silently lose a large fraction of real
  // conversations. So we deterministically SCRUB secrets / wallet+payment / PII
  // / local paths into typed placeholders BEFORE the tripwire. The redacted
  // value is the one we store. The report (counts only, never values) is
  // returned for public-safe observability.
  const { value: redacted, report: redactionReport } = redactTraceValue(mapped)
  const trajectory = redacted as AtifTrajectory

  // SAME public-safety tripwire the ingest route runs — now the FAIL-CLOSED
  // BACKSTOP after redaction. If anything STILL trips post-redaction, drop the
  // trace (never store a residual leak); the completion is unaffected. We surface
  // finding CODES only, never the offending value.
  const tripwireFindings = atifTraceTripwire(trajectory)
  if (tripwireFindings.length > 0) {
    return {
      emitted: false,
      reason: 'redaction_residual_drop',
      detail: tripwireFindings.map(finding => finding.code).join(','),
      redactionReport,
    }
  }

  const nowIso = (deps.nowIso ?? currentIsoTimestamp)()
  const traceUuid = (deps.makeId ?? randomUuid)()

  try {
    const stored = await deps.store.createTrace({
      traceUuid,
      ownerUserId: deps.owner.ownerUserId,
      agentRef: deps.owner.agentRef,
      schemaVersion: trajectory.schema_version,
      trajectoryId: trajectory.trajectory_id,
      sessionId: trajectory.session_id ?? null,
      visibility,
      stepCount: trajectory.steps.length,
      trajectory,
      trajectoryR2Key: null,
      blobRefs: [],
      // Idempotency keyed by the chat response id: re-emitting the same session
      // returns the existing trace instead of duplicating it.
      idempotencyKey: session.responseId,
      // Data-market reward stays INERT on the gateway emit path: consent is
      // never assumed/granted here. The upload data market (#6221) is the surface
      // that arms rewards behind its own owner-gated flag.
      trainingConsent: false,
      license: null,
      contentDigest: null,
      rewardEligible: false,
      rewardAmountSats: null,
      uploadSource: deps.owner.uploadSource,
      nowIso,
    })
    return {
      emitted: true,
      uuid: stored.record.traceUuid,
      url: `/trace/${stored.record.traceUuid}`,
      redactionReport,
    }
  } catch (error) {
    return {
      emitted: false,
      reason: 'store_error',
      detail: error instanceof Error ? error.message : String(error),
      redactionReport,
    }
  }
}
