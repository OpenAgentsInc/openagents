// Onboarding program: session state, the 10-section Output Spec, the D1 store,
// the inference seam, and the pure turn driver (EPIC #6123, issue #6126).
//
// This owns the SERVER-SIDE onboarding program that drives the productized
// intake interview via the Khala inference orchestrator (the OpenAI-compatible
// `/v1/chat/completions` gateway, internal model slug `khala`; the public
// OpenAI-compatible id is `openagents/khala`). It is
// transport-agnostic: a turn is `{ sessionId, userText }` in and an assistant
// reply + persisted session out. Text today; a voice layer (STT -> route -> TTS)
// can wrap the SAME turn driver later (see `OnboardingTurnInput`).
//
// The inference call is taken behind a narrow `OnboardingInferenceClient` seam
// (messages -> Effect<reply>), wired in production to the SAME provider-adapter
// registry + overflow dispatch the gateway uses (no external HTTP hop), and
// stubbed in tests. The system prompt (intake script + live-registry honesty
// contract + vertical overlay) is rebuilt deterministically every turn in
// autopilot-onboarding-system-prompt.ts, so it is never stored and never drifts.

import { Effect, Schema as S } from 'effect'

import {
  type InferenceMessage,
  type InferenceRequest,
} from './inference/provider-adapter'
import {
  buildOnboardingSystemPrompt,
  onboardingVerticalStorageValue,
  resolveOnboardingPromptVertical,
} from './autopilot-onboarding-system-prompt'
import {
  type AutopilotConciergeVertical,
  OA_OUTPUT_SPEC_FENCE_TAG as CONCIERGE_OUTPUT_SPEC_FENCE_TAG,
  OUTPUT_SPEC_FIELDS as CONCIERGE_OUTPUT_SPEC_FIELDS,
} from './inference/autopilot-concierge-model'
import { parseJsonUnknown, parseJsonWithSchema } from './json-boundary'

export const KHALA_ONBOARDING_MODEL = 'khala'

// MODEL -------------------------------------------------------------------

// A single stored transcript turn. Only user/assistant turns persist; the system
// prompt is rebuilt each turn and never stored.
export const OnboardingTranscriptTurn = S.Struct({
  role: S.Literals(['user', 'assistant']),
  content: S.String,
})
export type OnboardingTranscriptTurn = typeof OnboardingTranscriptTurn.Type

// The 10-section Output Spec the intake spec defines. Every field is optional so
// a partial spec is valid mid-interview; later stages consume the structured
// artifact as it fills in.
export const OnboardingOutputSpec = S.Struct({
  business: S.optionalKey(S.String),
  goal: S.optionalKey(S.String),
  chosenOfferings: S.optionalKey(S.String),
  quickWin: S.optionalKey(S.String),
  successMetric: S.optionalKey(S.String),
  scope: S.optionalKey(S.String),
  constraints: S.optionalKey(S.String),
  timeline: S.optionalKey(S.String),
  payment: S.optionalKey(S.String),
  openQuestions: S.optionalKey(S.String),
})
export type OnboardingOutputSpec = typeof OnboardingOutputSpec.Type

// OUTPUT SPEC EXTRACTION (issue #6148) --------------------------------------
//
// The model surfaces the current Output Spec as a structured artifact via a
// fenced `oa-output-spec` JSON block (every field optional; a partial spec is
// valid mid-interview). The program OWNS the schema, so the extractor lives here
// (the inference surface re-exports it) — keeping ONE source of truth and no
// circular dependency. The fenced JSON block is the reliable primary; a markdown
// `Output Spec` section is a bounded best-effort fallback. Pure; never throws.

// The fenced-block language tag the model uses to surface the structured spec
// + the 10 canonical spec field keys. Re-exported from the concierge model
// module (the shared, cycle-free home), so the schema, the parser, and the
// system prompt all derive from ONE field list.
export const OA_OUTPUT_SPEC_FENCE_TAG = CONCIERGE_OUTPUT_SPEC_FENCE_TAG
export const ONBOARDING_OUTPUT_SPEC_FIELDS = CONCIERGE_OUTPUT_SPEC_FIELDS

const OUTPUT_SPEC_FENCE_RE = new RegExp(
  '```' + OA_OUTPUT_SPEC_FENCE_TAG + '\\s*\\n([\\s\\S]*?)\\n?```',
  'g',
)

// The LAST fenced `oa-output-spec` block's raw JSON body (the freshest snapshot),
// or undefined when none is present.
const lastFencedOutputSpecJson = (completion: string): string | undefined => {
  OUTPUT_SPEC_FENCE_RE.lastIndex = 0
  let match: RegExpExecArray | null
  let last: string | undefined
  while ((match = OUTPUT_SPEC_FENCE_RE.exec(completion)) !== null) {
    last = match[1] ?? ''
  }
  return last
}

// Narrow an arbitrary record to the closed set of known string fields, then
// validate against the schema. Returns undefined on parse/validation failure or
// when no known field survives. Never throws.
const decodeOutputSpecObject = (
  value: unknown,
): OnboardingOutputSpec | undefined => {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    return undefined
  }
  const record = value as Record<string, unknown>
  const narrowed: Record<string, string> = {}
  for (const field of ONBOARDING_OUTPUT_SPEC_FIELDS) {
    const fieldValue = record[field]
    if (typeof fieldValue === 'string' && fieldValue.trim() !== '') {
      narrowed[field] = fieldValue.trim()
    }
  }
  if (Object.keys(narrowed).length === 0) {
    return undefined
  }
  try {
    return S.decodeUnknownSync(OnboardingOutputSpec)(narrowed)
  } catch {
    return undefined
  }
}

const MARKDOWN_OUTPUT_SPEC_LABELS: ReadonlyArray<
  Readonly<{ field: keyof OnboardingOutputSpec; labels: ReadonlyArray<string> }>
> = [
  { field: 'business', labels: ['business'] },
  { field: 'goal', labels: ['goal'] },
  { field: 'chosenOfferings', labels: ['chosen offerings', 'offerings'] },
  { field: 'quickWin', labels: ['quick win'] },
  { field: 'successMetric', labels: ['success metric'] },
  { field: 'scope', labels: ['scope'] },
  { field: 'constraints', labels: ['constraints'] },
  { field: 'timeline', labels: ['timeline'] },
  { field: 'payment', labels: ['payment'] },
  { field: 'openQuestions', labels: ['open questions'] },
]

// Text AFTER the last `Output Spec` heading, or undefined when absent.
const outputSpecMarkdownSection = (completion: string): string | undefined => {
  const headingRe = /(?:^|\n)[#*\s]*output spec[#*:\s]*\n/gi
  let match: RegExpExecArray | null
  let lastEnd: number | undefined
  while ((match = headingRe.exec(completion)) !== null) {
    lastEnd = match.index + match[0].length
  }
  return lastEnd === undefined ? undefined : completion.slice(lastEnd)
}

const parseMarkdownOutputSpec = (
  section: string,
): OnboardingOutputSpec | undefined => {
  const collected: Record<string, string> = {}
  for (const rawLine of section.split('\n')) {
    const line = rawLine
      .replace(/^[\s>]*[-*]?\s*/, '')
      .replace(/^\d+[.)]\s*/, '')
      .replace(/\*\*/g, '')
      .trim()
    if (line === '') continue
    const sep = line.search(/\s—\s|\s–\s|:\s/u)
    if (sep === -1) continue
    const label = line
      .slice(0, sep)
      .trim()
      .toLowerCase()
      .replace(/^\d+[.)]?\s*/, '')
      .trim()
    const value = line.slice(sep).replace(/^\s*[—–:]\s*/u, '').trim()
    if (value === '') continue
    if (/^\(?(none|n\/a|na|tbd|unknown|—|-)\)?$/i.test(value)) continue
    const binding = MARKDOWN_OUTPUT_SPEC_LABELS.find(b =>
      b.labels.some(l => label === l || label.startsWith(l)),
    )
    if (binding === undefined) continue
    if (collected[binding.field] === undefined) {
      collected[binding.field] = value
    }
  }
  return decodeOutputSpecObject(collected)
}

// Extract the structured Output Spec from a completion: the LAST fenced
// `oa-output-spec` JSON block first (reliable), then a markdown `Output Spec`
// section (best-effort). Returns undefined when neither yields a known field.
export const extractOnboardingOutputSpec = (
  completion: string,
): OnboardingOutputSpec | undefined => {
  const fenced = lastFencedOutputSpecJson(completion)
  if (fenced !== undefined) {
    let parsed: unknown
    try {
      // Parse at the named JSON boundary helper (zero-debt: raw JSON.parse stays
      // inside json-boundary.ts).
      parsed = parseJsonUnknown(fenced)
    } catch {
      parsed = undefined
    }
    const spec = decodeOutputSpecObject(parsed)
    if (spec !== undefined) {
      return spec
    }
  }
  const section = outputSpecMarkdownSection(completion)
  return section === undefined ? undefined : parseMarkdownOutputSpec(section)
}

// Merge a freshly-extracted spec over a prior accumulated spec so a session's
// spec only GROWS across turns (a later non-empty field overrides; an omitted
// field does not erase the earlier value). Pure.
export const mergeOnboardingOutputSpec = (
  prior: OnboardingOutputSpec,
  next: OnboardingOutputSpec | undefined,
): OnboardingOutputSpec => (next === undefined ? prior : { ...prior, ...next })

export const OnboardingSessionStatus = S.Literals(['interviewing', 'complete'])
export type OnboardingSessionStatus = typeof OnboardingSessionStatus.Type

// The persisted onboarding session.
export const OnboardingSession = S.Struct({
  id: S.String,
  verticalOverlay: S.NullOr(S.String),
  status: OnboardingSessionStatus,
  transcript: S.Array(OnboardingTranscriptTurn),
  outputSpec: OnboardingOutputSpec,
  turnCount: S.Int,
  createdAt: S.String,
  updatedAt: S.String,
})
export type OnboardingSession = typeof OnboardingSession.Type

// ROUTE I/O ---------------------------------------------------------------

// Turn request body. `userText` is the human's message for this turn.
// `vertical` is the server-owned Concierge vertical enum. `verticalOverlay` is a
// deprecated compatibility field: callers may still send it, but the server only
// uses it to recover the bounded `legal` enum from the old client. Raw overlay
// text is never injected into prompts.
export const OnboardingTurnRequest = S.Struct({
  userText: S.String,
  vertical: S.optionalKey(S.NullOr(S.String)),
  verticalOverlay: S.optionalKey(S.NullOr(S.String)),
})
export type OnboardingTurnRequest = typeof OnboardingTurnRequest.Type

// The transport-agnostic turn input the driver consumes. A voice front-end
// produces this from STT and renders `reply` via TTS; today the HTTP route
// produces it from the JSON body. This is the documented voice hook.
export type OnboardingTurnInput = Readonly<{
  sessionId: string
  userText: string
  vertical: AutopilotConciergeVertical
}>

export const OnboardingTurnResponse = S.Struct({
  sessionId: S.String,
  reply: S.String,
  status: OnboardingSessionStatus,
  turnCount: S.Int,
  outputSpec: OnboardingOutputSpec,
})
export type OnboardingTurnResponse = typeof OnboardingTurnResponse.Type

// INFERENCE SEAM ----------------------------------------------------------

// The narrow inference seam. Given the fully-assembled message list (system +
// transcript + new user turn), return the assistant reply text. Production wires
// this to the provider-adapter registry + overflow dispatch; tests inject a stub.
// Errors surface as a tagged failure so the route maps them to a stable response.
export type OnboardingInferenceClient = (
  request: InferenceRequest,
) => Effect.Effect<string, OnboardingInferenceError>

export class OnboardingInferenceError extends S.TaggedErrorClass<OnboardingInferenceError>()(
  'OnboardingInferenceError',
  {
    reason: S.String,
  },
) {}

// STREAMING INFERENCE SEAM ------------------------------------------------

// The streaming inference seam. Given the assembled message list, return a
// source whose `deltas` async-iterable yields assistant TEXT increments as the
// upstream produces them, and whose `final()` resolves the full reply once the
// stream is drained. Production wires this to the provider-adapter `streamSse`
// (or buffered `stream`) path; tests inject a deterministic stub. This is the
// streaming complement to `OnboardingInferenceClient` — same input, incremental
// output. Errors surface as the same tagged failure the route already maps.
export type OnboardingStreamSource = Readonly<{
  deltas: AsyncIterable<string>
  // Resolves AFTER `deltas` is exhausted, with the full accumulated reply.
  final: () => string
}>

export type OnboardingStreamClient = (
  request: InferenceRequest,
) => Effect.Effect<OnboardingStreamSource, OnboardingInferenceError>

export class OnboardingStorageError extends S.TaggedErrorClass<OnboardingStorageError>()(
  'OnboardingStorageError',
  {
    operation: S.String,
    reason: S.String,
  },
) {}

export class OnboardingValidationError extends S.TaggedErrorClass<OnboardingValidationError>()(
  'OnboardingValidationError',
  {
    reason: S.String,
  },
) {}

// STORE -------------------------------------------------------------------

// Persistence seam over the `autopilot_onboarding_sessions` D1 table. The turn
// driver reads the (possibly absent) session, then upserts the advanced session.
export type OnboardingSessionStore = Readonly<{
  read: (
    sessionId: string,
  ) => Effect.Effect<OnboardingSession | undefined, OnboardingStorageError>
  upsert: (
    session: OnboardingSession,
  ) => Effect.Effect<void, OnboardingStorageError>
}>

type OnboardingSessionRow = Readonly<{
  id: string
  vertical_overlay: string | null
  status: string
  transcript_json: string
  output_spec_json: string
  turn_count: number
  created_at: string
  updated_at: string
}>

const decodeTranscript = (
  raw: string,
): ReadonlyArray<OnboardingTranscriptTurn> => {
  try {
    return parseJsonWithSchema(S.Array(OnboardingTranscriptTurn), raw)
  } catch {
    return []
  }
}

const decodeOutputSpec = (raw: string): OnboardingOutputSpec => {
  try {
    return parseJsonWithSchema(OnboardingOutputSpec, raw)
  } catch {
    return {}
  }
}

const rowToSession = (row: OnboardingSessionRow): OnboardingSession => ({
  id: row.id,
  verticalOverlay: row.vertical_overlay,
  status: row.status === 'complete' ? 'complete' : 'interviewing',
  transcript: decodeTranscript(row.transcript_json),
  outputSpec: decodeOutputSpec(row.output_spec_json),
  turnCount: row.turn_count,
  createdAt: row.created_at,
  updatedAt: row.updated_at,
})

export const makeD1OnboardingSessionStore = (
  db: D1Database,
): OnboardingSessionStore => ({
  read: sessionId =>
    Effect.tryPromise({
      try: async () => {
        const row = await db
          .prepare(
            `SELECT id, vertical_overlay, status, transcript_json,
                    output_spec_json, turn_count, created_at, updated_at
             FROM autopilot_onboarding_sessions
             WHERE id = ?`,
          )
          .bind(sessionId)
          .first<OnboardingSessionRow>()

        return row === null ? undefined : rowToSession(row)
      },
      catch: error =>
        new OnboardingStorageError({
          operation: 'onboarding.session.read',
          reason: error instanceof Error ? error.message : String(error),
        }),
    }),
  upsert: session =>
    Effect.tryPromise({
      try: async () => {
        await db
          .prepare(
            `INSERT INTO autopilot_onboarding_sessions
               (id, vertical_overlay, status, transcript_json, output_spec_json,
                turn_count, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               vertical_overlay = excluded.vertical_overlay,
               status = excluded.status,
               transcript_json = excluded.transcript_json,
               output_spec_json = excluded.output_spec_json,
               turn_count = excluded.turn_count,
               updated_at = excluded.updated_at`,
          )
          .bind(
            session.id,
            session.verticalOverlay,
            session.status,
            JSON.stringify(session.transcript),
            JSON.stringify(session.outputSpec),
            session.turnCount,
            session.createdAt,
            session.updatedAt,
          )
          .run()
      },
      catch: error =>
        new OnboardingStorageError({
          operation: 'onboarding.session.upsert',
          reason: error instanceof Error ? error.message : String(error),
        }),
    }),
})

// PROGRAM -----------------------------------------------------------------

// Assemble the message list for a turn: the deterministic system prompt (intake
// script + live-registry honesty contract + overlay), the prior transcript, then
// the new user turn. Pure and transport-agnostic.
export const buildOnboardingMessages = (
  session: OnboardingSession,
  userText: string,
): ReadonlyArray<InferenceMessage> => [
  { role: 'system', content: buildOnboardingSystemPrompt(session.verticalOverlay) },
  ...session.transcript.map(turn => ({ role: turn.role, content: turn.content })),
  { role: 'user', content: userText },
]

const MAX_USER_TEXT_LENGTH = 16_000

export type OnboardingTurnDeps = Readonly<{
  store: OnboardingSessionStore
  infer: OnboardingInferenceClient
  nowIso: () => string
}>

// Drive one onboarding turn. Reads (or creates) the session, calls the inference
// seam with the assembled messages, appends the exchange, and persists. The
// Output Spec accumulation is performed by the model into its replies; this
// driver persists the transcript that carries it and exposes the current spec
// snapshot. (Structured spec extraction is intentionally left to the consuming
// stage; the transcript is the durable source of truth.)
export const runOnboardingTurn = (
  input: OnboardingTurnInput,
  deps: OnboardingTurnDeps,
): Effect.Effect<
  OnboardingTurnResponse,
  OnboardingInferenceError | OnboardingStorageError | OnboardingValidationError
> =>
  Effect.gen(function* () {
    const userText = input.userText.trim()
    if (userText === '') {
      return yield* new OnboardingValidationError({
        reason: 'userText must not be empty',
      })
    }
    if (userText.length > MAX_USER_TEXT_LENGTH) {
      return yield* new OnboardingValidationError({
        reason: `userText exceeds ${MAX_USER_TEXT_LENGTH} characters`,
      })
    }

    const now = deps.nowIso()
    const existing = yield* deps.store.read(input.sessionId)

    const session: OnboardingSession =
      existing ??
      ({
        id: input.sessionId,
        verticalOverlay: onboardingVerticalStorageValue(input.vertical),
        status: 'interviewing',
        transcript: [],
        outputSpec: {},
        turnCount: 0,
        createdAt: now,
        updatedAt: now,
      } satisfies OnboardingSession)

    const messages = buildOnboardingMessages(session, userText)

    const request: InferenceRequest = {
      model: KHALA_ONBOARDING_MODEL,
      messages,
      stream: false,
      passthroughParams: {},
    }

    const reply = yield* deps.infer(request)

    const userTurn: OnboardingTranscriptTurn = { role: 'user', content: userText }
    const assistantTurn: OnboardingTranscriptTurn = {
      role: 'assistant',
      content: reply,
    }

    const advanced: OnboardingSession = {
      id: session.id,
      verticalOverlay: onboardingVerticalStorageValue(
        resolveOnboardingPromptVertical(session.verticalOverlay),
      ),
      status: session.status,
      transcript: [...session.transcript, userTurn, assistantTurn],
      // Accumulate the structured Output Spec from this reply over the prior
      // session spec (issue #6148), so the persisted spec grows across turns and
      // the page + programmatic consumers read the same structured artifact.
      outputSpec: mergeOnboardingOutputSpec(
        session.outputSpec,
        extractOnboardingOutputSpec(reply),
      ),
      turnCount: session.turnCount + 1,
      createdAt: session.createdAt,
      updatedAt: now,
    }

    yield* deps.store.upsert(advanced)

    return {
      sessionId: advanced.id,
      reply,
      status: advanced.status,
      turnCount: advanced.turnCount,
      outputSpec: advanced.outputSpec,
    } satisfies OnboardingTurnResponse
  })

// STREAMING TURN DRIVER ---------------------------------------------------

// The validated, in-flight state of a streaming turn: the session it advances,
// the user text for this turn, and the live stream source. The route pumps the
// source's `deltas` to the client, then calls `finalizeOnboardingStreamTurn`
// with the same session + the full reply to append + persist + build the final
// response. Splitting prepare/finalize lets the route own the SSE pump while the
// program keeps the read/build/persist logic and the single validation point.
export type OnboardingStreamTurn = Readonly<{
  session: OnboardingSession
  userText: string
  source: OnboardingStreamSource
}>

export type OnboardingStreamTurnDeps = Readonly<{
  store: OnboardingSessionStore
  stream: OnboardingStreamClient
  nowIso: () => string
}>

// Prepare a streaming turn: validate the user text, read (or create) the
// session, and open the stream. Mirrors the non-streaming driver's validation +
// session-resolution, so the streaming and buffered paths cannot diverge.
export const prepareOnboardingStreamTurn = (
  input: OnboardingTurnInput,
  deps: OnboardingStreamTurnDeps,
): Effect.Effect<
  OnboardingStreamTurn,
  OnboardingInferenceError | OnboardingStorageError | OnboardingValidationError
> =>
  Effect.gen(function* () {
    const userText = input.userText.trim()
    if (userText === '') {
      return yield* new OnboardingValidationError({
        reason: 'userText must not be empty',
      })
    }
    if (userText.length > MAX_USER_TEXT_LENGTH) {
      return yield* new OnboardingValidationError({
        reason: `userText exceeds ${MAX_USER_TEXT_LENGTH} characters`,
      })
    }

    const now = deps.nowIso()
    const existing = yield* deps.store.read(input.sessionId)

    const session: OnboardingSession =
      existing ??
      ({
        id: input.sessionId,
        verticalOverlay: onboardingVerticalStorageValue(input.vertical),
        status: 'interviewing',
        transcript: [],
        outputSpec: {},
        turnCount: 0,
        createdAt: now,
        updatedAt: now,
      } satisfies OnboardingSession)

    const messages = buildOnboardingMessages(session, userText)

    const request: InferenceRequest = {
      model: KHALA_ONBOARDING_MODEL,
      messages,
      stream: true,
      passthroughParams: {},
    }

    const source = yield* deps.stream(request)

    return { session, userText, source }
  })

// Finalize a streaming turn once the deltas are drained: append the exchange,
// persist, and build the same `OnboardingTurnResponse` the buffered path returns.
// Only needs the store + clock (the stream is already drained), so it takes the
// narrower deps shape — the route drives it at the streaming boundary.
export const finalizeOnboardingStreamTurn = (
  turn: OnboardingStreamTurn,
  reply: string,
  deps: Readonly<{ store: OnboardingSessionStore; nowIso: () => string }>,
): Effect.Effect<OnboardingTurnResponse, OnboardingStorageError> =>
  Effect.gen(function* () {
    const { session, userText } = turn
    const now = deps.nowIso()

    const userTurn: OnboardingTranscriptTurn = { role: 'user', content: userText }
    const assistantTurn: OnboardingTranscriptTurn = {
      role: 'assistant',
      content: reply,
    }

    const advanced: OnboardingSession = {
      id: session.id,
      verticalOverlay: onboardingVerticalStorageValue(
        resolveOnboardingPromptVertical(session.verticalOverlay),
      ),
      status: session.status,
      transcript: [...session.transcript, userTurn, assistantTurn],
      // Accumulate the structured Output Spec from this reply over the prior
      // session spec (issue #6148) — same as the buffered driver.
      outputSpec: mergeOnboardingOutputSpec(
        session.outputSpec,
        extractOnboardingOutputSpec(reply),
      ),
      turnCount: session.turnCount + 1,
      createdAt: session.createdAt,
      updatedAt: now,
    }

    yield* deps.store.upsert(advanced)

    return {
      sessionId: advanced.id,
      reply,
      status: advanced.status,
      turnCount: advanced.turnCount,
      outputSpec: advanced.outputSpec,
    } satisfies OnboardingTurnResponse
  })
