// Khala typed component channel over SSE (EPIC #6123, issue #6127).
//
// WHY THIS EXISTS
// ---------------
// The Khala onboarding flow (§4.3 / §4.3.1 of the 2026-06-23 autopilot-onboarding
// audit) needs the model to stream renderable UI cards alongside prose — a Stripe
// "kick off with $500 in credits" card, an intake-progress card, a dashboard
// preview, and so on. The Khala `/v1/chat/completions` gateway today emits ONLY
// OpenAI `chat.completion.chunk` `{content: string}` deltas; there is no
// structured/component channel. This module is that channel.
//
// THE WIRE CONTRACT (§4.3.1, designed from the json-render study)
// ---------------------------------------------------------------
// Khala keeps emitting normal OpenAI `chat.completion.chunk` `{content}` deltas
// for prose. When a card is surfaced, it emits a DISTINCT, ATOMIC, VERSIONED SSE
// frame on a custom event type:
//
//   event: oa.component
//   data: {"v":1,"component":"credit_kickoff","props":{...},"id":"cmp_01"}
//
// - ATOMIC + VERSIONED: one complete card per frame (no JSON-patch reassembly, no
//   partial-JSON parse). The explicit `"v":1` is the one thing json-render lacks
//   and we add.
// - OPENAI-SSE-FRIENDLY: a standard OpenAI client switches on `event:` types it
//   knows and IGNORES `oa.component`, so the stream still parses as normal text.
//   The Foldkit client switches on `oa.component` to render the typed card.
//
// THE MODEL INPUT MECHANISM (model -> gateway)
// --------------------------------------------
// Khala routes across multiple backends (Gemini Flash, Fireworks) with uneven
// tool-calling fidelity, and raw OpenAI `tool_calls` arguments stream as PARTIAL
// JSON strings — exactly the half-parsed-props problem json-render avoids. So the
// model surfaces a card via a constrained fenced block IN ITS TEXT:
//
//   ```oa-component
//   {"component":"credit_kickoff","props":{"amountCents":50000,"label":"..."}}
//   ```
//
// The gateway PARSES the fenced block out of the model's text (the
// `createMixedStreamParser` concept: split prose vs fenced component JSON),
// VALIDATES props against the CLOSED catalog with Effect Schema, and re-emits the
// clean `event: oa.component` frame. THAT validated, typed frame is the "typed
// tool response" the client consumes. A native `tool_call` input is also accepted
// where a backend reliably produces one, but the gateway never RELIES on uniform
// tool-calling.
//
// THE INVARIANTS THIS ENFORCES (see INVARIANTS.md "Khala Typed Component Channel")
// --------------------------------------------------------------------------------
//   1. CLOSED CATALOG. The model may only select from + fill the 6-component v1
//      catalog. An unknown component name is rejected, never emitted. Adding a
//      7th is a deliberate, reviewed catalog bump.
//   2. GATEWAY-SIDE SCHEMA VALIDATION + BOUNDED REPAIR. Props are validated with
//      Effect Schema BEFORE a frame leaves Khala. On invalid output ONE bounded
//      repair turn runs; if it still fails the component is DROPPED (never ship
//      malformed UI).
//   3. NO PROVIDER-IDENTITY LEAKAGE. Component props are scanned with the SAME
//      `khala-identity` redaction backstop. A card whose props leak a provider
//      identity is dropped, so the structured channel cannot become a side door
//      around the prose identity guard.
//   4. ADDITIVE / OPT-IN. The whole channel is gated behind a flag/opt-in; with
//      it off the gateway is byte-for-byte today's text-only stream. Standard
//      OpenAI clients always parse the stream as normal text regardless.

import { Schema as S } from 'effect'

import { parseJsonUnknown } from '../json-boundary'
import { guardKhalaCompletion, verifyKhalaSignatures } from './khala-identity'

// ---------------------------------------------------------------------------
// The wire-format version. Explicit + checked so a future v2 catalog can ship
// without breaking a v1 client (json-render lacked this; §4.3.1).
// ---------------------------------------------------------------------------
export const OA_COMPONENT_WIRE_VERSION = 1 as const

// The custom SSE event name. Standard OpenAI clients ignore unknown `event:`
// types; the Foldkit client switches on this one.
export const OA_COMPONENT_SSE_EVENT = 'oa.component' as const

// The fenced-block language tag the model uses to surface a card in its text.
export const OA_COMPONENT_FENCE_TAG = 'oa-component' as const

// ---------------------------------------------------------------------------
// The CLOSED catalog (v1). Six components, each an Effect-Schema'd props object.
// EXACT shapes from §4.3.1. Adding a 7th is a reviewed catalog bump, never an
// ad-hoc model invention.
// ---------------------------------------------------------------------------

// `credit_kickoff {amountCents, label}` — the "kick off with $500 in credits"
// card. `amountCents` is a positive integer number of cents; `label` is the
// rendered CTA copy.
export const CreditKickoffProps = S.Struct({
  amountCents: S.Int.check(S.isGreaterThan(0)),
  label: S.NonEmptyString,
})

// `intake_progress {steps[], current}` — the multi-step intake progress card.
// `steps` is the ordered list of step labels; `current` is the 0-based index of
// the active step (bounded to the steps array at validation time below).
export const IntakeProgressProps = S.Struct({
  steps: S.NonEmptyArray(S.NonEmptyString),
  current: S.Int.check(S.isGreaterThanOrEqualTo(0)),
})

// `quick_win_card {title, scope, etaDays}` — the scoped quick-win offer card.
export const QuickWinCardProps = S.Struct({
  title: S.NonEmptyString,
  scope: S.NonEmptyString,
  etaDays: S.Int.check(S.isGreaterThan(0)),
})

// `dashboard_preview {workspaceRef, seededFacts[]}` — the personalized dashboard
// preview. `workspaceRef` is an opaque ref to the prefilled workspace;
// `seededFacts` are the public-safe facts already seeded into it.
export const DashboardPreviewProps = S.Struct({
  workspaceRef: S.NonEmptyString,
  seededFacts: S.Array(S.NonEmptyString),
})

// `human_handoff {reason, contact}` — escalate to a human. Also the render-time
// safe fallback in the client's closed registry (§4.3.1).
export const HumanHandoffProps = S.Struct({
  reason: S.NonEmptyString,
  contact: S.NonEmptyString,
})

// `consent_gate {scope, dataPractices, required}` — the informed-consent gate
// (ABA Op. 512 / PHI-redaction posture). `required` makes the gate blocking.
export const ConsentGateProps = S.Struct({
  scope: S.NonEmptyString,
  dataPractices: S.NonEmptyString,
  required: S.Boolean,
})

// The catalog: component name -> props schema. The KEYS are the closed enum; a
// component name not in this map is rejected. Keep this the single source of
// truth — the enum, the validators, and the prompt all derive from it.
export const KHALA_COMPONENT_CATALOG = {
  consent_gate: ConsentGateProps,
  credit_kickoff: CreditKickoffProps,
  dashboard_preview: DashboardPreviewProps,
  human_handoff: HumanHandoffProps,
  intake_progress: IntakeProgressProps,
  quick_win_card: QuickWinCardProps,
} as const

export type KhalaComponentName = keyof typeof KHALA_COMPONENT_CATALOG

// The closed component-name enum, derived from the catalog keys so the schema and
// the runtime map can never drift.
export const KHALA_COMPONENT_NAMES = Object.keys(
  KHALA_COMPONENT_CATALOG,
) as ReadonlyArray<KhalaComponentName>

export const KhalaComponentName = S.Literals(
  KHALA_COMPONENT_NAMES as ReadonlyArray<KhalaComponentName>,
)

export const isKnownKhalaComponent = (
  name: string,
): name is KhalaComponentName =>
  Object.prototype.hasOwnProperty.call(KHALA_COMPONENT_CATALOG, name)

// ---------------------------------------------------------------------------
// The validated wire frame. This is the typed payload the gateway emits and the
// client consumes — the "typed tool response".
// ---------------------------------------------------------------------------
export type KhalaComponentFrame = Readonly<{
  v: typeof OA_COMPONENT_WIRE_VERSION
  component: KhalaComponentName
  props: Readonly<Record<string, unknown>>
  id: string
}>

// ---------------------------------------------------------------------------
// The mixed-stream parser (the `createMixedStreamParser` concept, §4.3.1): split
// the model's text into PROSE (re-emitted as normal `{content}` deltas) and
// FENCED component JSON (parsed, validated, re-emitted as `oa.component` frames).
// ---------------------------------------------------------------------------

// One raw candidate extracted from the model text: either a prose segment or the
// raw JSON body of a fenced `oa-component` block (still UNVALIDATED).
export type ParsedSegment =
  | Readonly<{ kind: 'prose'; text: string }>
  | Readonly<{ kind: 'component'; rawJson: string }>

// Match a fenced ```oa-component ... ``` block. Tolerant of surrounding
// whitespace and an optional trailing newline before the closing fence. Global +
// multiline so multiple cards in one completion are all extracted in order.
const FENCE_RE = new RegExp(
  '```' + OA_COMPONENT_FENCE_TAG + '\\s*\\n([\\s\\S]*?)\\n?```',
  'g',
)

// Split a completed model-text string into ordered prose + component segments.
// The prose between/around fenced blocks is preserved verbatim (minus the fenced
// blocks themselves), so the client renders exactly the prose the model wrote and
// the cards appear where the model placed them.
export const splitMixedStream = (
  completion: string,
): ReadonlyArray<ParsedSegment> => {
  const segments: Array<ParsedSegment> = []
  let lastIndex = 0
  FENCE_RE.lastIndex = 0
  let match: RegExpExecArray | null
  while ((match = FENCE_RE.exec(completion)) !== null) {
    const prose = completion.slice(lastIndex, match.index)
    if (prose !== '') {
      segments.push({ kind: 'prose', text: prose })
    }
    segments.push({ kind: 'component', rawJson: match[1] ?? '' })
    lastIndex = match.index + match[0].length
  }
  const tail = completion.slice(lastIndex)
  if (tail !== '') {
    segments.push({ kind: 'prose', text: tail })
  }
  // No fences at all => the whole completion is prose (the common case; default
  // behavior is unchanged for any completion that never emits a card).
  if (segments.length === 0 && completion !== '') {
    segments.push({ kind: 'prose', text: completion })
  }
  return segments
}

// The prose with all fenced component blocks REMOVED — what the prose channel
// should carry when the component channel is on. Whitespace left by a removed
// block is collapsed so the prose reads cleanly.
export const stripComponentFences = (completion: string): string =>
  splitMixedStream(completion)
    .filter((s): s is Extract<ParsedSegment, { kind: 'prose' }> => s.kind === 'prose')
    .map(s => s.text)
    .join('')
    .replace(/\n{3,}/g, '\n\n')
    .trim()

// ---------------------------------------------------------------------------
// Validation: a raw candidate -> a typed frame, or a typed rejection reason.
// ---------------------------------------------------------------------------

export type ComponentValidationResult =
  | Readonly<{ ok: true; frame: KhalaComponentFrame }>
  | Readonly<{
      ok: false
      // Stable, neutral reason ref (no raw provider/model material).
      reason:
        | 'invalid_json'
        | 'missing_component'
        | 'unknown_component'
        | 'invalid_props'
        | 'provider_identity_leak'
      // A short, public-safe detail for the repair turn (schema issue summary or
      // the rejected component name). NEVER contains provider identity material.
      detail: string
    }>

// Scan a value for a forbidden provider-identity leak using the SAME khala
// identity signatures that guard the prose channel. Returns true if ANY string
// anywhere in the props (deeply) trips the first-person provider-identity guard.
// This is the structured-channel mirror of the prose identity backstop — the
// component channel must not become a side door around it.
const propsLeakProviderIdentity = (value: unknown): boolean => {
  const strings: Array<string> = []
  const visit = (v: unknown): void => {
    if (typeof v === 'string') {
      strings.push(v)
    } else if (Array.isArray(v)) {
      for (const item of v) visit(item)
    } else if (v !== null && typeof v === 'object') {
      for (const item of Object.values(v)) visit(item)
    }
  }
  visit(value)
  if (strings.length === 0) return false
  const joined = strings.join('\n')
  return verifyKhalaSignatures(joined).some(verdict => !verdict.satisfied)
}

// Validate one raw fenced-block JSON body against the closed catalog. Order:
//   1. parse JSON (invalid_json)
//   2. require a `component` name (missing_component)
//   3. component must be in the closed catalog (unknown_component)
//   4. props must satisfy the component's Effect Schema (invalid_props)
//   5. props must not leak a provider identity (provider_identity_leak)
// A passing validation yields a typed `KhalaComponentFrame`.
export const validateComponentCandidate = (
  rawJson: string,
  input: Readonly<{ id: string }>,
): ComponentValidationResult => {
  let parsed: unknown
  try {
    // Parse the fenced-block body at the named JSON boundary helper (zero-debt
    // architecture: raw JSON.parse stays inside `json-boundary.ts`).
    parsed = parseJsonUnknown(rawJson)
  } catch {
    return { detail: 'component json did not parse', ok: false, reason: 'invalid_json' }
  }
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    return { detail: 'component must be a json object', ok: false, reason: 'invalid_json' }
  }
  const record = parsed as Record<string, unknown>
  const componentName = record['component']
  if (typeof componentName !== 'string' || componentName === '') {
    return {
      detail: 'component name missing',
      ok: false,
      reason: 'missing_component',
    }
  }
  // CLOSED-ENUM ENFORCEMENT: reject any name not in the v1 catalog.
  if (!isKnownKhalaComponent(componentName)) {
    return {
      detail: `unknown component "${componentName}"; allowed: ${KHALA_COMPONENT_NAMES.join(', ')}`,
      ok: false,
      reason: 'unknown_component',
    }
  }
  const propsSchema = KHALA_COMPONENT_CATALOG[componentName]
  const rawProps = record['props']
  let props: Record<string, unknown>
  try {
    props = S.decodeUnknownSync(propsSchema)(rawProps) as Record<string, unknown>
  } catch (error) {
    return {
      detail: summarizeSchemaError(error),
      ok: false,
      reason: 'invalid_props',
    }
  }
  // CROSS-FIELD bound: intake_progress.current must index into steps. Effect
  // Schema validates the field shapes; this is the one relational constraint the
  // per-field schema cannot express, so it is checked here (still pre-emit).
  if (componentName === 'intake_progress') {
    const steps = props['steps']
    const current = props['current']
    if (
      Array.isArray(steps) &&
      typeof current === 'number' &&
      current >= steps.length
    ) {
      return {
        detail: `intake_progress.current (${current}) out of range for ${steps.length} steps`,
        ok: false,
        reason: 'invalid_props',
      }
    }
  }
  // PROVIDER-IDENTITY NON-LEAKAGE: the structured channel honors the same
  // redaction backstop as the prose channel. A leak drops the card.
  if (propsLeakProviderIdentity(props)) {
    return {
      detail: 'component props contained a forbidden provider identity',
      ok: false,
      reason: 'provider_identity_leak',
    }
  }
  return {
    frame: {
      component: componentName,
      id: input.id,
      props,
      v: OA_COMPONENT_WIRE_VERSION,
    },
    ok: true,
  }
}

// A short, public-safe summary of an Effect Schema decode error. Bounded so a
// repair turn or a log never carries an unbounded provider payload.
const summarizeSchemaError = (error: unknown): string => {
  const message = error instanceof Error ? error.message : String(error)
  // Collapse whitespace and bound length; this text feeds ONE repair turn and a
  // public-safe rejection ref, never raw provider material.
  return message.replace(/\s+/g, ' ').slice(0, 240)
}

// ---------------------------------------------------------------------------
// Serialization: a typed frame -> the `oa.component` SSE wire frame.
// ---------------------------------------------------------------------------

// Serialize a validated frame as a custom-event SSE frame:
//   event: oa.component\n data: {...}\n\n
// The `event:` line is what a standard OpenAI client ignores (it only reads
// `data:` frames it recognizes), and what the Foldkit client switches on.
export const serializeComponentFrame = (frame: KhalaComponentFrame): string =>
  `event: ${OA_COMPONENT_SSE_EVENT}\ndata: ${JSON.stringify(frame)}\n\n`

// ---------------------------------------------------------------------------
// The repair turn (bounded). On a schema-invalid candidate, ONE re-ask is made
// with a structured instruction naming the rejection; if the repaired candidate
// is still invalid, the component is DROPPED (never ship malformed UI).
// ---------------------------------------------------------------------------

// The reask hook: given a repair instruction, return the model's repaired raw
// component JSON (or undefined if the reask is unavailable / failed). Kept
// transport-agnostic so the route can wire it to a single non-streaming Khala
// call without coupling this module to the dispatch path.
export type ComponentRepairReask = (
  instruction: string,
) => Promise<string | undefined>

// Build the bounded repair instruction for a rejected candidate. Names the
// closed catalog + the exact rejection so the model can fix the ONE card.
export const buildComponentRepairInstruction = (
  rejection: Extract<ComponentValidationResult, { ok: false }>,
): string =>
  [
    'Your previous oa-component block was invalid and was not rendered.',
    `Rejection: ${rejection.reason} — ${rejection.detail}.`,
    `Re-emit exactly ONE corrected component as a single \`\`\`${OA_COMPONENT_FENCE_TAG}\`\`\` fenced JSON object.`,
    `It must be one of: ${KHALA_COMPONENT_NAMES.join(', ')}, with props matching that component's schema.`,
    'Return only the corrected fenced block, no prose.',
  ].join(' ')

// Validate a candidate with ONE bounded repair turn. Pure given the injected
// `reask`. Returns the validated frame, or a typed `dropped` outcome carrying the
// final rejection (so the route can log a public-safe reason). NEVER returns a
// malformed frame.
export const validateWithBoundedRepair = async (
  rawJson: string,
  input: Readonly<{ id: string; reask?: ComponentRepairReask | undefined }>,
): Promise<
  | Readonly<{ outcome: 'valid'; frame: KhalaComponentFrame; repaired: boolean }>
  | Readonly<{
      outcome: 'dropped'
      reason: Extract<ComponentValidationResult, { ok: false }>['reason']
      detail: string
    }>
> => {
  const first = validateComponentCandidate(rawJson, { id: input.id })
  if (first.ok) {
    return { frame: first.frame, outcome: 'valid', repaired: false }
  }
  // ONE bounded repair turn (only when a reask is wired).
  if (input.reask !== undefined) {
    const instruction = buildComponentRepairInstruction(first)
    let repairedRaw: string | undefined
    try {
      repairedRaw = await input.reask(instruction)
    } catch {
      repairedRaw = undefined
    }
    if (repairedRaw !== undefined) {
      // The reask may return either a bare JSON object or a fenced block; extract
      // the fenced body if present, else use the raw text.
      const segments = splitMixedStream(repairedRaw)
      const componentSegment = segments.find(
        (s): s is Extract<ParsedSegment, { kind: 'component' }> =>
          s.kind === 'component',
      )
      const candidate = componentSegment?.rawJson ?? repairedRaw
      const second = validateComponentCandidate(candidate, { id: input.id })
      if (second.ok) {
        return { frame: second.frame, outcome: 'valid', repaired: true }
      }
      // Still invalid after the bounded repair => drop with the repair rejection.
      return { detail: second.detail, outcome: 'dropped', reason: second.reason }
    }
  }
  // No reask available, or reask failed => drop with the first rejection.
  return { detail: first.detail, outcome: 'dropped', reason: first.reason }
}

// ---------------------------------------------------------------------------
// The full gateway transform: a completed model-text completion -> the prose to
// stream + the validated component frames to emit. This is the single entry the
// route calls once the upstream completion is assembled (additive; only invoked
// when the component channel is opted in).
// ---------------------------------------------------------------------------

export type ComponentChannelOutput = Readonly<{
  // The prose with component fences stripped — what the `{content}` channel
  // carries. Identity-guarded (the SAME backstop the prose path already runs).
  prose: string
  // The validated component frames to emit as `oa.component` SSE frames, in the
  // order the model placed them.
  frames: ReadonlyArray<KhalaComponentFrame>
  // Public-safe drops (a card that failed validation + bounded repair). Logged,
  // never emitted.
  dropped: ReadonlyArray<
    Readonly<{ reason: string; detail: string }>
  >
}>

// Transform an assembled completion through the component channel.
//   - splits prose vs fenced components
//   - validates each candidate with ONE bounded repair turn
//   - drops malformed/unknown/leaking candidates
//   - identity-guards the prose
// `idForIndex` mints a stable component id per surfaced card (defaults to
// `cmp_<n>`). `reask` (optional) wires the bounded repair turn.
export const runComponentChannel = async (
  completion: string,
  input: Readonly<{
    reask?: ComponentRepairReask | undefined
    idForIndex?: (index: number) => string
  }>,
): Promise<ComponentChannelOutput> => {
  const segments = splitMixedStream(completion)
  const proseParts: Array<string> = []
  const frames: Array<KhalaComponentFrame> = []
  const dropped: Array<{ reason: string; detail: string }> = []
  const idFor = input.idForIndex ?? ((index: number) => `cmp_${index + 1}`)
  let componentIndex = 0
  for (const segment of segments) {
    if (segment.kind === 'prose') {
      proseParts.push(segment.text)
      continue
    }
    const id = idFor(componentIndex)
    componentIndex += 1
    const result = await validateWithBoundedRepair(segment.rawJson, {
      id,
      reask: input.reask,
    })
    if (result.outcome === 'valid') {
      frames.push(result.frame)
    } else {
      dropped.push({ detail: result.detail, reason: result.reason })
    }
  }
  const rawProse = proseParts.join('').replace(/\n{3,}/g, '\n\n').trim()
  // Identity-guard the prose with the SAME backstop the prose path uses.
  const guard = await guardKhalaCompletion({ completion: rawProse })
  return { dropped, frames, prose: guard.text }
}

// ---------------------------------------------------------------------------
// The catalog system prompt (the `catalog.prompt()` concept). Lists the closed
// component set + their prop schemas so the model emits a card via the fenced
// mechanism that works across ALL Khala backends. Injected as a STABLE prefix
// block ONLY when the component channel is opted in (additive).
// ---------------------------------------------------------------------------

export const KHALA_COMPONENT_CATALOG_PROMPT = [
  'When a renderable onboarding card is appropriate, you may surface ONE card by emitting a fenced code block tagged `oa-component` containing a single JSON object.',
  `The JSON object is {"component": <name>, "props": {...}}. The component name MUST be one of: ${KHALA_COMPONENT_NAMES.join(', ')}. You may NOT invent any other component.`,
  'Component prop shapes:',
  '- credit_kickoff: {"amountCents": <positive integer cents>, "label": <non-empty string>}',
  '- intake_progress: {"steps": <non-empty array of step labels>, "current": <0-based index into steps>}',
  '- quick_win_card: {"title": <string>, "scope": <string>, "etaDays": <positive integer>}',
  '- dashboard_preview: {"workspaceRef": <string>, "seededFacts": <array of strings>}',
  '- human_handoff: {"reason": <string>, "contact": <string>}',
  '- consent_gate: {"scope": <string>, "dataPractices": <string>, "required": <boolean>}',
  'Write normal prose for everything else; only use the fenced oa-component block for a card. Emit at most one card per turn unless the flow clearly needs more, and never put a card mid-sentence.',
  'Never put any model or provider identity, vendor name, or "built on"/"powered by" language inside a card.',
].join(' ')

// Parse the component-channel opt-in flag. Default OFF: the channel is additive
// and inert unless explicitly enabled.
export const isComponentChannelEnabled = (
  value: string | undefined,
): boolean => {
  if (value === undefined) {
    return false
  }
  return ['1', 'true', 'yes', 'on'].includes(value.trim().toLowerCase())
}
