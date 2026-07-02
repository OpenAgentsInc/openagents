// OpenAgents Business conversational intake — the POST route at
// /api/public/business-intake-chat (BUSINESS_INTAKE_CHAT_ENDPOINT below).
//
// A bounded, public, server-side chat turn for the /business dynamic intake:
// the browser holds the whole transcript and sends it on every call; this route
// replies as "Khala" running the interview from
// docs/business/2026-06-20-openagents-business-intake-spec.md. The system
// prompt is a module constant below encoding the offerings menu with honest
// availability labels, the A-G interview with branching, the quick-win ->
// Autopilot ladder, and the Output Spec Template. When the model emits the
// completed spec inside `<intake-spec>...</intake-spec>` the route extracts it
// into `spec`, sets `done: true`, and strips the sentinel from `reply`.
//
// Serving lane: the SAME conditions as the free gateway (gateway flag on +
// Fireworks key present), a single non-streaming completion against the fixed
// Khala Fireworks backing model with fixed params (temperature 0.4,
// max_tokens 700). No tool use, no streaming, no client-controlled
// model/params, and the client can never supply a `system` role.
//
// Accounting (INVARIANTS.md "Canonical Token Usage Ledger"): every served
// completion records one exact `token_usage_events` row through the existing
// served-tokens recorder (`usage_truth='exact'`, `demand_kind='internal'`,
// `demand_source='business_intake_chat'`). Recording is fail-soft — a ledger
// fault never breaks the customer's already-served reply — and estimated
// tokens are never counted (no usage => no row).
//
// Abuse bounds: per-IP fixed windows (<= 8 requests/minute and <= 60/day per
// CF-Connecting-IP), best-effort in-memory per isolate like the sibling
// public /api/khala/chat limiter, failing closed to 429.
import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  FIREWORKS_ADAPTER_ID,
  KHALA_FIREWORKS_BACKING_MODEL_ID,
} from './inference/fireworks-adapter'
import { KHALA_MODEL_ID } from './inference/pricing'
import {
  type InferenceAdapterError,
  type InferenceRequest,
  type InferenceResult,
} from './inference/provider-adapter'
import {
  type ServedTokensRecorder,
  meterServedTokensFailSoft,
} from './inference/served-tokens-recorder'
import { isRecord, parseJsonUnknown } from './json-boundary'
import { liveAtReadStaleness } from './public-projection-staleness'
import {
  compactRandomId,
  currentEpochMillis,
  currentIsoTimestamp,
} from './runtime-primitives'

export const BUSINESS_INTAKE_CHAT_ENDPOINT = '/api/public/business-intake-chat'

// Request bounds — fail closed with typed 400s. The browser resends the whole
// transcript each turn, so these bound the total interview surface.
export const BUSINESS_INTAKE_CHAT_MAX_MESSAGES = 24
export const BUSINESS_INTAKE_CHAT_MAX_MESSAGE_CHARS = 2_000
export const BUSINESS_INTAKE_CHAT_MAX_TOTAL_CHARS = 24_000
const MAX_REQUEST_BODY_CHARS = 128_000

// Fixed serving params. Never client-controlled.
export const BUSINESS_INTAKE_CHAT_TEMPERATURE = 0.4
export const BUSINESS_INTAKE_CHAT_MAX_TOKENS = 700

// Demand attribution for the canonical token ledger: this is a first-party
// acquisition surface, so its rows classify as internal demand with a
// distinguishable source, never as external customer usage.
export const BUSINESS_INTAKE_CHAT_ACCOUNT_REF = 'public:business-intake-chat'
export const BUSINESS_INTAKE_CHAT_DEMAND_SOURCE = 'business_intake_chat'
export const BUSINESS_INTAKE_CHAT_DEMAND_CLIENT = 'business-intake-web'

// The completed-spec sentinel the system prompt instructs the model to emit.
export const BUSINESS_INTAKE_SPEC_OPEN_TAG = '<intake-spec>'
export const BUSINESS_INTAKE_SPEC_CLOSE_TAG = '</intake-spec>'

// Deterministic opening turn for an empty transcript: no model call, no usage.
export const BUSINESS_INTAKE_CHAT_OPENING_REPLY =
  "Hi — I'm Khala, the intake assistant for OpenAgents Business. OpenAgents sells machine work with receipts: AI agents and compute that do real work for your business, starting with one fast quick win and growing into recurring work on Autopilot. I'll ask a few short questions, one area at a time, and at the end hand you a filled intake spec that gets attached to your submission. To start: in one or two sentences, what does your business do, and who are your customers?"

const REPLY_WHEN_SPEC_ONLY =
  'Your intake spec is complete. It will be attached to your intake submission for the OpenAgents team to review.'

export const BUSINESS_INTAKE_CHAT_SYSTEM_PROMPT = `You are Khala, the intake assistant for OpenAgents Business (openagents.com/business). You run a short, honest buying interview for a potential customer, then produce a filled intake spec. You are talking to a human in a chat widget; keep every reply short (a few sentences), warm, and concrete. Ask about ONE interview area at a time, and briefly summarize back what you heard before moving to the next area.

WHAT OPENAGENTS IS
OpenAgents sells machine work with receipts: AI agents and compute that do real, useful work, where every accepted outcome is tied to verifiable evidence. The model: start with a fast quick win deliverable in days, then put recurring work on Autopilot as trust builds, with payment scoped up front (including Bitcoin where the rails are proven) and collected only as work is accepted.

OFFERINGS MENU (the ONLY things you may offer, with honest availability labels)
1. Coding & agent work — a coding agent takes a written objective, works in a repo, runs your verification command, and returns a reviewable change with evidence. Availability: Operator-assisted. Typical quick win: fix a failing test suite, refactor a messy module, or ship one small feature with passing tests.
2. Inference / AI (models on tap) — open-weight model inference through OpenAgents with a free taste and credit-funded metered usage after. Availability: Operator-assisted (the full paid card/Bitcoin-to-credit spend path is not end-to-end yet; a single compatible credit-balance gateway is Roadmap). Typical quick win: run a batch of summaries/classifications/extractions and hand back results.
3. Sites + commerce — an Autopilot Site at a stable URL with optional custom hostnames, native email sequences, and referral links. Availability: Operator-assisted (partial/flag-gated today). Typical quick win: a branded landing page plus a welcome-email sequence.
4. Autopilot business automation — recurring work run by agents through a factory pipeline (Signal -> Triage -> Build -> Validate -> Release -> Document -> Monitor -> Deploy) with prefilled workspaces for e-commerce, legal (review-gated, no legal advice), and marketing agencies. Every stage keeps a human-review gate before anything publishes or spends. Availability: Operator-assisted (the fully self-serve all-in-one system is Roadmap). Typical quick win: one prefilled vertical workspace with a first real work item drafted, never auto-published.
5. Distributed compute / training — scoped, verified training runs on the Pylon contributor network. Availability: Operator-assisted (fine-tuning-as-a-service and metered sandbox compute are not finished buy-it-now offerings; the public device-capability dataset is Roadmap). Typical quick win: a bounded, verified training or compute task with a reported result and receipt.
6. Forum / community — a registered agent identity that posts on the OpenAgents Forum, requests/fulfills labor jobs, and sends/receives tips. Availability: Available now. Typical quick win: stand up your own Forum agent.
7. Payments rails — Bitcoin-native payments: reliable tips with BOLT 12 + offline fallback, and USD-credit funding for usage. Availability: Operator-assisted (reliable tips are live; broader self-custodial wallet flow, card purchase of credits, and native-sat settlement for general payouts are not broadly ready). Typical quick win: fund an account and run paid work end-to-end with a dereferenceable receipt.

HONESTY RULES (hard)
- "Available now" means shipped and green; "Operator-assisted" means useful pieces are live but delivery needs a human/operator path, a flag, or a caveat; "Roadmap" means not shipped. Always state the availability label when recommending an offering.
- NEVER promise anything beyond this menu. If the human asks for something not on it, say so plainly and capture it as an open question in the spec.
- NEVER ask for or accept passwords, API keys, tokens, wallet seeds, or any credentials. If access to a system is needed, only record WHICH systems in the spec; access setup happens later with the OpenAgents team.
- Do not give legal, tax, or financial advice.

INTERVIEW (run these areas in order, ONE area per turn; skip questions that clearly don't apply)
A. Business & goals — what the business does, customers/main product, and the single most important outcome for the next month. Branch: if they can't name an outcome, ask what took too much of their team's time last week and use that.
B. Painful/repetitive work — what they'd happily hand to an agent, and whether a one-off task is blocking them right now. Branch: a one-off blocker steers toward a quick win in Coding (1), Inference (2), or Sites (3); a recurring grind steers toward Autopilot business automation (4).
C. Success metric — one concrete measure the quick win worked (hours saved, shipped fix, launched page, N items processed, deadline met), and what would make them continue onto Autopilot.
D. Budget & payment — rough quick-win budget (small fixed amounts are fine); preference for credit card / USD credits or Bitcoin (Lightning/sats — both supported); ongoing model: usage-metered, fixed monthly, or pay-per-accepted-outcome. Branch: if they want Bitcoin, note it and set expectations honestly — reliable tips are live, but broader wallet/credit-purchase/native-sat settlement paths are operator-assisted or roadmap depending on the work.
E. Data & access constraints — which systems an agent would need to touch (repo, site/DNS, ad/email accounts, documents, CRM), any privacy/compliance constraints, and confirm they are OK with a human-review gate before anything is published, sent, deployed, or spent (our default; required for legal, commerce, and external delivery).
F. Timeline — when they want the quick win, and whether it's tied to a launch, deadline, or event.
G. Fit — recommend ONE or TWO offerings from the menu with their availability labels, confirm the human wants to start with the quick win, and then produce the filled spec.

THE LADDER (show this arc when it helps the human see where it goes)
Day 1 — Quick win: one small, well-scoped task delivered with evidence; low budget, fast turnaround, no big commitment. Week 1 — Repeatable lane: turn the quick win into a repeatable workflow the human reviews while agents do the legwork. Ongoing — On Autopilot: hand a slice of the business to background agents, always with a human-review gate, accepted outcomes with receipts, and the option to pay in Bitcoin.

FINISHING (the spec sentinel)
When the interview is complete and the human confirms the quick win (end of area G), emit the filled Output Spec as markdown wrapped EXACTLY in <intake-spec> and </intake-spec> tags, using the template below with every section filled from the interview (write "unknown" or "none" where the human had no answer). Outside the tags, tell the human in one or two sentences that their intake spec is complete and will be attached to their intake submission. Do NOT emit the tags or the template before the interview is complete.

Output Spec Template (fill and wrap in the tags):
# OpenAgents Business — Customer Intake Spec

## 1. Business
- Company / what we do:
- Customers / main product:
- Primary contact (name, email):
- Preferred contact channel (email / shared Slack / Forum agent):

## 2. Goal
- The outcome we want in the next month:
- Why it matters now:

## 3. Chosen offerings (1-2)
- Offering A: <name> — availability: <available now / operator-assisted / roadmap>
- Offering B (optional): <name> — availability: <available now / operator-assisted / roadmap>

## 4. Quick win (Day 1)
- The first small task to deliver:
- What "done" looks like:
- Target delivery date:

## 5. Success metric
- We'll know the quick win worked when:
- What would make us continue onto Autopilot:

## 6. Scope
- In scope:
- Explicitly out of scope (for now):
- Systems/accounts the agent will need access to:

## 7. Constraints
- Privacy / compliance / regulated constraints:
- Human-review gate required before publish/send/deploy/spend? (yes/no — default yes):
- Anything off-limits:

## 8. Timeline
- Quick win by:
- Tied to a launch/deadline/event? (describe):

## 9. Payment
- Quick-win budget (rough):
- Payment preference: <credit card / USD credits / Bitcoin (Lightning/sats)>
- Ongoing model: <usage-metered / fixed monthly / pay-per-accepted-outcome>

## 10. Open questions / requests beyond the menu
- Anything the human asked for that isn't in the offerings menu:
- Things OpenAgents needs to confirm before starting:`

export type BusinessIntakeChatMessage = Readonly<{
  role: 'assistant' | 'user'
  content: string
}>

export type BusinessIntakeChatDeps = Readonly<{
  // Gateway flag (env.INFERENCE_GATEWAY_ENABLED parsed) — same as /v1/chat.
  enabled: boolean
  // Fireworks lane armed (FIREWORKS_API_KEY present) — same presence check the
  // gateway's supply-lane arming uses. Both must hold or the route 503s.
  fireworksArmed: boolean
  // Non-streaming completion over the provider-adapter seam. The Worker wires
  // the Fireworks adapter's `complete`; tests inject a fake.
  complete: (
    request: InferenceRequest,
  ) => Effect.Effect<InferenceResult, InferenceAdapterError>
  // Canonical exact token-usage recording (served-tokens recorder seam).
  recordTokensServed: ServedTokensRecorder
  // Per-IP admission. Defaults to the module-level in-memory window limiter.
  rateLimit?: ((request: Request) => boolean) | undefined
  // Stable per-completion request id (idempotency for the ledger row).
  makeRequestId?: (() => string) | undefined
  // Injectable clock for the response's generatedAt (staleness contract).
  nowIso?: (() => string) | undefined
}>

// --- request decoding (fail closed, typed 400 reasons) ----------------------

type DecodedMessages =
  | Readonly<{ messages: ReadonlyArray<BusinessIntakeChatMessage> }>
  | Readonly<{ reason: string }>

const decodeMessages = (value: unknown): DecodedMessages => {
  if (!isRecord(value)) {
    return { reason: 'body must be a JSON object' }
  }
  const raw = value['messages']
  if (!Array.isArray(raw)) {
    return { reason: 'messages must be an array' }
  }
  if (raw.length > BUSINESS_INTAKE_CHAT_MAX_MESSAGES) {
    return {
      reason: `messages must contain at most ${BUSINESS_INTAKE_CHAT_MAX_MESSAGES} entries`,
    }
  }
  const decoded: Array<BusinessIntakeChatMessage> = []
  for (const entry of raw) {
    if (!isRecord(entry)) {
      return { reason: 'each message must be an object' }
    }
    const role = entry['role']
    if (role !== 'user' && role !== 'assistant') {
      return { reason: 'message roles must be user or assistant' }
    }
    const content = entry['content']
    if (typeof content !== 'string' || content.trim() === '') {
      return { reason: 'each message content must be a non-empty string' }
    }
    if (content.length > BUSINESS_INTAKE_CHAT_MAX_MESSAGE_CHARS) {
      return {
        reason: `each message content must be at most ${BUSINESS_INTAKE_CHAT_MAX_MESSAGE_CHARS} characters`,
      }
    }
    decoded.push({ content, role })
  }
  const totalChars = decoded.reduce(
    (total, message) => total + message.content.length,
    0,
  )
  if (totalChars > BUSINESS_INTAKE_CHAT_MAX_TOTAL_CHARS) {
    return {
      reason: `total message content must be at most ${BUSINESS_INTAKE_CHAT_MAX_TOTAL_CHARS} characters`,
    }
  }
  if (decoded.length > 0 && decoded[0]?.role !== 'user') {
    return { reason: 'the first message must be from the user' }
  }
  return { messages: decoded }
}

const safeJsonParse = (text: string): unknown => {
  try {
    return parseJsonUnknown(text)
  } catch {
    return undefined
  }
}

// --- per-IP window rate limiting (best-effort, per isolate) -----------------

const RATE_MINUTE_LIMIT = 8
const RATE_MINUTE_WINDOW_MS = 60_000
const RATE_DAY_LIMIT = 60
const RATE_DAY_WINDOW_MS = 86_400_000
// Bound the counter maps so a scan across many IPs cannot grow isolate memory
// without limit; clearing resets windows, which only ever ADMITS more traffic
// (the durable bound stays the model-call cost per window under normal load).
const RATE_MAX_TRACKED_IPS = 10_000

type WindowCounter = { count: number; windowStartedAt: number }

const admitWindow = (
  counters: Map<string, WindowCounter>,
  key: string,
  limit: number,
  windowMs: number,
  now: number,
): boolean => {
  if (counters.size > RATE_MAX_TRACKED_IPS) {
    counters.clear()
  }
  const existing = counters.get(key)
  if (existing === undefined || now - existing.windowStartedAt >= windowMs) {
    counters.set(key, { count: 1, windowStartedAt: now })
    return true
  }
  if (existing.count >= limit) {
    return false
  }
  existing.count = existing.count + 1
  return true
}

const clientIp = (request: Request): string =>
  request.headers.get('cf-connecting-ip') ??
  request.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ??
  'unknown'

// Build a per-IP limiter over an injectable clock (tests drive time; the
// Worker default uses the runtime-primitives clock). Both windows must admit.
export const makeBusinessIntakeChatRateLimiter = (
  now: () => number = currentEpochMillis,
): ((request: Request) => boolean) => {
  const minuteCounters = new Map<string, WindowCounter>()
  const dayCounters = new Map<string, WindowCounter>()
  return request => {
    const ip = clientIp(request)
    const at = now()
    const minuteOk = admitWindow(
      minuteCounters,
      ip,
      RATE_MINUTE_LIMIT,
      RATE_MINUTE_WINDOW_MS,
      at,
    )
    const dayOk = admitWindow(
      dayCounters,
      ip,
      RATE_DAY_LIMIT,
      RATE_DAY_WINDOW_MS,
      at,
    )
    return minuteOk && dayOk
  }
}

const defaultRateLimit = makeBusinessIntakeChatRateLimiter()

// --- sentinel extraction -----------------------------------------------------

export type BusinessIntakeSpecExtraction = Readonly<{
  reply: string
  done: boolean
  spec: string | null
}>

// Extract the completed Output Spec from a model completion. Only a completion
// carrying BOTH sentinel tags with non-empty content counts as done; a dangling
// open tag (e.g. a max_tokens truncation mid-spec) strips the partial block so
// the sentinel never leaks to the human, but does not mark the interview done.
export const extractBusinessIntakeSpec = (
  content: string,
): BusinessIntakeSpecExtraction => {
  const openIndex = content.indexOf(BUSINESS_INTAKE_SPEC_OPEN_TAG)
  if (openIndex === -1) {
    return { done: false, reply: content.trim(), spec: null }
  }
  const closeIndex = content.indexOf(
    BUSINESS_INTAKE_SPEC_CLOSE_TAG,
    openIndex + BUSINESS_INTAKE_SPEC_OPEN_TAG.length,
  )
  if (closeIndex === -1) {
    return { done: false, reply: content.slice(0, openIndex).trim(), spec: null }
  }
  const spec = content
    .slice(openIndex + BUSINESS_INTAKE_SPEC_OPEN_TAG.length, closeIndex)
    .trim()
  const reply = (
    content.slice(0, openIndex) +
    ' ' +
    content.slice(closeIndex + BUSINESS_INTAKE_SPEC_CLOSE_TAG.length)
  ).trim()
  if (spec === '') {
    return { done: false, reply, spec: null }
  }
  return {
    done: true,
    reply: reply === '' ? REPLY_WHEN_SPEC_ONLY : reply,
    spec,
  }
}

// --- route handler -----------------------------------------------------------

// The 200 turn is a live-at-write reply over the single model completion (or
// the deterministic opening constant); nothing is stored or replayed, so the
// declared staleness composition is live at read of this one request.
const intakeChatStaleness = liveAtReadStaleness([
  'business_intake_chat.completion',
])

const unavailableResponse = () =>
  noStoreJsonResponse(
    { error: 'business_intake_chat_unavailable' },
    { status: 503 },
  )

const validationResponse = (reason: string) =>
  noStoreJsonResponse(
    { error: 'business_intake_chat_validation_error', reason },
    { status: 400 },
  )

export const handleBusinessIntakeChatApi = (
  request: Request,
  deps: BusinessIntakeChatDeps,
) =>
  Effect.gen(function* () {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    // Serving-lane gate: the SAME conditions the free gateway serves under
    // (gateway flag on + Fireworks key present). No new owner flag.
    if (!deps.enabled || !deps.fireworksArmed) {
      return unavailableResponse()
    }

    const rateLimit = deps.rateLimit ?? defaultRateLimit
    if (!rateLimit(request)) {
      return noStoreJsonResponse(
        { error: 'business_intake_rate_limited' },
        { status: 429 },
      )
    }

    const text = yield* Effect.promise(() => request.text().catch(() => ''))
    if (text.length > MAX_REQUEST_BODY_CHARS) {
      return validationResponse('request body is too large')
    }
    const parsed = safeJsonParse(text)
    if (parsed === undefined) {
      return noStoreJsonResponse(
        { error: 'business_intake_chat_invalid_json' },
        { status: 400 },
      )
    }
    const decoded = decodeMessages(parsed)
    if ('reason' in decoded) {
      return validationResponse(decoded.reason)
    }

    const nowIso = deps.nowIso ?? currentIsoTimestamp

    // Empty transcript: Khala's deterministic opening greeting + first
    // question. No model call, so nothing was served and nothing is recorded.
    if (decoded.messages.length === 0) {
      return noStoreJsonResponse({
        done: false,
        generatedAt: nowIso(),
        ok: true,
        reply: BUSINESS_INTAKE_CHAT_OPENING_REPLY,
        spec: null,
        staleness: intakeChatStaleness,
      })
    }

    const inferenceRequest: InferenceRequest = {
      messages: [
        { content: BUSINESS_INTAKE_CHAT_SYSTEM_PROMPT, role: 'system' },
        ...decoded.messages.map(message => ({
          content: message.content,
          role: message.role,
        })),
      ],
      model: KHALA_FIREWORKS_BACKING_MODEL_ID,
      passthroughParams: {
        max_tokens: BUSINESS_INTAKE_CHAT_MAX_TOKENS,
        temperature: BUSINESS_INTAKE_CHAT_TEMPERATURE,
      },
      stream: false,
    }

    const outcome = yield* deps.complete(inferenceRequest).pipe(
      Effect.map(result => ({ _tag: 'served' as const, result })),
      Effect.catch(() => Effect.succeed({ _tag: 'failed' as const })),
    )
    if (outcome._tag === 'failed') {
      return unavailableResponse()
    }

    // Exact usage row via the canonical served-tokens recorder — fail-soft:
    // the reply below returns even when the ledger write fails or dies.
    const makeRequestId =
      deps.makeRequestId ?? (() => compactRandomId('business_intake_chat'))
    yield* meterServedTokensFailSoft(deps.recordTokensServed, {
      accountRef: BUSINESS_INTAKE_CHAT_ACCOUNT_REF,
      adapterId: FIREWORKS_ADAPTER_ID,
      requestAttribution: {
        demandClient: BUSINESS_INTAKE_CHAT_DEMAND_CLIENT,
        demandKind: 'internal',
        demandSource: BUSINESS_INTAKE_CHAT_DEMAND_SOURCE,
      },
      requestId: makeRequestId(),
      requestMetrics: {
        requestClass: 'interactive_stream',
        supplyLane: FIREWORKS_ADAPTER_ID,
      },
      requestedModel: KHALA_MODEL_ID,
      servedModel: outcome.result.servedModel,
      streamed: false,
      usage: outcome.result.usage,
    })

    const extraction = extractBusinessIntakeSpec(outcome.result.content)
    return noStoreJsonResponse({
      done: extraction.done,
      generatedAt: nowIso(),
      ok: true,
      reply: extraction.reply,
      spec: extraction.spec,
      staleness: intakeChatStaleness,
    })
  })
