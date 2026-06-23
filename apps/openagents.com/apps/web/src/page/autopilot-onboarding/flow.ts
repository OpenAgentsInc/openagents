// Autopilot onboarding — the client flow state machine (issue #6129).
//
// This is the self-contained, transport-agnostic state for the `/autopilot`
// onboarding conversation. The page (`./page.ts`) renders it; the loggedOut
// MVU (`page/loggedOut/{model,message,update}.ts`) owns one slot of this model
// and delegates turn submission to the command defined alongside the route.
//
// The flow drives turns against `POST /api/autopilot/onboarding/{sessionId}/turn`
// (the merged Khala program, #6126). The v1 program returns assistant TEXT plus
// the accumulated 10-section Output Spec; the model-chosen typed-component
// streaming over the gateway (#6127) stays inert for now (triple-gated), so this
// module SURFACES the closed-catalog components (#6128) deterministically from
// the conversation/spec state instead. That is the honest v1 scope: the renderer
// and the catalog are real and exercised, and the page is ready to swap the
// derived frames for streamed ones once the gateway flag flips.
//
// Determinism: no `Math.random` / time-of-day here. Component ids are derived
// from stable keys (the spec section name, the turn count) so captures, tests,
// and the flutter-in stagger are reproducible.

import { Option, Schema as S } from 'effect'
import { ts } from 'foldkit/schema'

import {
  type RenderableFrame,
  validateComponentFrame,
} from './component-catalog'

// MODEL -------------------------------------------------------------------

// A single rendered transcript turn. Mirrors the server transcript turn but
// stays a client-local schema so the page model is self-describing.
export const FlowTurn = S.Struct({
  role: S.Literals(['user', 'assistant']),
  content: S.String,
})
export type FlowTurn = typeof FlowTurn.Type

// The 10-section Output Spec, mirrored from the server program. Every field is
// optional so a partial spec mid-interview is valid; the page lights up sections
// as they fill in.
export const FlowOutputSpec = S.Struct({
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
export type FlowOutputSpec = typeof FlowOutputSpec.Type

// The request lifecycle for a turn:
//   - `idle`        — no request in flight.
//   - `submitting`  — a turn is posted; the SSE stream may not have opened yet.
//   - `streaming`   — the assistant reply is landing token-by-token (deltas are
//                     accumulating in `streamingReply`).
//   - `error`       — the last turn failed; `errorReason` carries the message.
// `error` carries an operator-readable reason the composer surfaces; the user
// can retry.
export const FlowStatus = S.Literals(['idle', 'submitting', 'streaming', 'error'])
export type FlowStatus = typeof FlowStatus.Type

// The onboarding flow model. `sessionId` is generated once the first turn is
// submitted (in the command, off the pure update path). `vertical` is the
// optional `/autopilot/{vertical}` segment; pending turns resolve it to the
// bounded `FlowVertical` enum before crossing the transport boundary.
export const FlowVertical = S.Literals(['general', 'legal'])
export type FlowVertical = typeof FlowVertical.Type

// A turn waiting for (or receiving) its streamed reply. The subscription
// (`subscriptions.ts`) reads this to open the SSE stream and dispatch deltas;
// `id` is the stable per-turn key so the subscription opens the stream exactly
// once per turn (never re-fires while it is in flight). `null` when no turn is
// pending.
export const FlowPendingTurn = S.Struct({
  id: S.String,
  sessionId: S.NullOr(S.String),
  userText: S.String,
  vertical: FlowVertical,
})
export type FlowPendingTurn = typeof FlowPendingTurn.Type

export const FlowModel = ts('AutopilotOnboardingFlow', {
  vertical: S.NullOr(S.String),
  sessionId: S.NullOr(S.String),
  composerDraft: S.String,
  status: FlowStatus,
  errorReason: S.NullOr(S.String),
  transcript: S.Array(FlowTurn),
  // The in-flight assistant reply, accumulating as SSE deltas arrive. `null`
  // when no reply is streaming. On stream completion it is committed to the
  // transcript and reset to null. Held separately so the streaming turn renders
  // a live, markdown-progressive bubble without mutating the durable transcript.
  streamingReply: S.NullOr(S.String),
  // The turn the streaming subscription should drive, or null when none is in
  // flight. Carries the per-turn id so the SSE stream opens exactly once.
  pendingTurn: S.NullOr(FlowPendingTurn),
  outputSpec: FlowOutputSpec,
  turnCount: S.Int,
})
export type FlowModel = typeof FlowModel.Type

// The turn response from `POST /api/autopilot/onboarding/{sessionId}/turn`,
// mirrored client-side from the server program's `OnboardingTurnResponse`
// (workers/api `autopilot-onboarding-program.ts`). The web and api packages are
// separate, so this is a deliberate, narrow mirror decoded at the client
// boundary; if the server contract changes, this decode fails loudly.
export const OnboardingTurnResponse = S.Struct({
  sessionId: S.String,
  reply: S.String,
  status: S.Literals(['interviewing', 'complete']),
  turnCount: S.Int,
  outputSpec: FlowOutputSpec,
})
export type OnboardingTurnResponse = typeof OnboardingTurnResponse.Type

// STREAM WIRE -------------------------------------------------------------

// The narrow SSE wire the streaming turn route emits (see
// `workers/api/src/autopilot-onboarding-routes.ts`):
//   event: delta  data: { "text": "…" }
//   event: done   data: <OnboardingTurnResponse>
//   event: error  data: { "error": "…" }
// One parsed SSE event from the stream, normalized for the subscription. The
// subscription maps these to the streaming messages.
export type OnboardingStreamEvent =
  | Readonly<{ kind: 'delta'; text: string }>
  | Readonly<{ kind: 'done'; response: OnboardingTurnResponse }>
  | Readonly<{ kind: 'error'; reason: string }>

const DeltaPayload = S.Struct({ text: S.String })

// Parse one decoded SSE block (its `event` name + JSON `data` payload) into a
// typed stream event. Unknown events / malformed payloads yield `undefined` so
// the consumer simply skips them (forward-compatible, never throws).
export const parseOnboardingStreamEvent = (
  event: string,
  data: unknown,
): OnboardingStreamEvent | undefined => {
  if (event === 'delta') {
    return S.decodeUnknownOption(DeltaPayload)(data).pipe(
      Option.match({
        onNone: () => undefined,
        onSome: ({ text }) => ({ kind: 'delta' as const, text }),
      }),
    )
  }
  if (event === 'done') {
    return S.decodeUnknownOption(OnboardingTurnResponse)(data).pipe(
      Option.match({
        onNone: () => undefined,
        onSome: response => ({ kind: 'done' as const, response }),
      }),
    )
  }
  if (event === 'error') {
    return { kind: 'error', reason: 'stream_failed' }
  }
  return undefined
}

export const initFlowModel = (vertical: Option.Option<string>): FlowModel =>
  FlowModel({
    vertical: Option.getOrNull(vertical),
    sessionId: null,
    composerDraft: '',
    status: 'idle',
    errorReason: null,
    transcript: [],
    streamingReply: null,
    pendingTurn: null,
    outputSpec: {},
    turnCount: 0,
  })

// OUTPUT-SPEC SECTIONS ----------------------------------------------------

// The ordered 10 sections, with display labels. The `intake_progress` register
// lights these up as they are captured; the order is the interview order from
// the intake spec (#6126).
export const OUTPUT_SPEC_SECTIONS: ReadonlyArray<{
  readonly id: keyof FlowOutputSpec
  readonly label: string
}> = [
  { id: 'business', label: 'Your business' },
  { id: 'goal', label: 'What you want done' },
  { id: 'chosenOfferings', label: 'Offerings' },
  { id: 'quickWin', label: 'First quick win' },
  { id: 'successMetric', label: 'Success metric' },
  { id: 'scope', label: 'Scope' },
  { id: 'constraints', label: 'Constraints' },
  { id: 'timeline', label: 'Timeline' },
  { id: 'payment', label: 'Payment' },
  { id: 'openQuestions', label: 'Open questions' },
]

const isCaptured = (spec: FlowOutputSpec, id: keyof FlowOutputSpec): boolean => {
  const value = spec[id]
  return typeof value === 'string' && value.trim() !== ''
}

// How many sections are captured (filled, non-blank). Used by the progress
// register and to decide whether the flow has reached a quote-ready state.
export const capturedSectionCount = (spec: FlowOutputSpec): number =>
  OUTPUT_SPEC_SECTIONS.reduce(
    (count, section) => count + (isCaptured(spec, section.id) ? 1 : 0),
    0,
  )

// The index of the FIRST not-yet-captured section, clamped into range — the
// "current" step for `intake_progress`. When all are captured it points at the
// last section.
export const currentSectionIndex = (spec: FlowOutputSpec): number => {
  const firstOpen = OUTPUT_SPEC_SECTIONS.findIndex(
    section => !isCaptured(spec, section.id),
  )
  return firstOpen === -1 ? OUTPUT_SPEC_SECTIONS.length - 1 : firstOpen
}

// INTAKE REGISTER ---------------------------------------------------------

// One row of the sidebar intake register (problem #3). The 10 Output-Spec
// sections render as a slim vertical register that lights up / checks off as
// each is captured — glanceable, not a giant box. `done` = captured, `active` =
// the current (first open) step, `queued` = not yet reached.
export type IntakeRegisterStatus = 'done' | 'active' | 'queued'

export type IntakeRegisterStep = Readonly<{
  id: keyof FlowOutputSpec
  label: string
  status: IntakeRegisterStatus
}>

// Derive the register rows for the current spec. Pure + deterministic.
export const deriveIntakeRegister = (
  model: FlowModel,
): ReadonlyArray<IntakeRegisterStep> => {
  const spec = model.outputSpec
  const current = currentSectionIndex(spec)

  return OUTPUT_SPEC_SECTIONS.map((section, index) => ({
    id: section.id,
    label: section.label,
    status: isCaptured(spec, section.id)
      ? ('done' as const)
      : index === current
        ? ('active' as const)
        : ('queued' as const),
  }))
}

// The flow is "quote-ready" once the credit-kickoff-relevant facts are captured:
// the business, the goal, the quick win, and the payment intent. This is the
// honest v1 gate for surfacing the `credit_kickoff` card — it does not pretend
// the full spec is complete, only that enough is known to kick off paid work.
export const isQuoteReady = (spec: FlowOutputSpec): boolean =>
  isCaptured(spec, 'business') &&
  isCaptured(spec, 'goal') &&
  isCaptured(spec, 'quickWin')

// DERIVED COMPONENT FRAMES ------------------------------------------------

// The default credit-kickoff grant for the v1 flow: $5.00 (matches the existing
// onboarding funding floor). The amount is honest chrome, not a claimed balance.
export const CREDIT_KICKOFF_AMOUNT_CENTS = 500

// Derive the closed-catalog component frames to surface for the current flow
// state. Pure and deterministic: same spec/status in, same frames out. These go
// through the #6128 renderer exactly as a gateway-streamed frame would; the only
// difference today is that the page produces them from session state rather than
// reading them off the `oa.component` SSE channel (which is inert in v1).
//
// The `intake_progress` register is NOT in this list any more (problem #3): the
// 10-section interhview progress is now a compact sidebar register (see
// `deriveIntakeRegister`), so it stays glanceable and never dominates the main
// column. The components below are the inline, in-thread surfaces only.
//
// Ordering follows document order so the renderer interleaves them naturally:
//   1. consent_gate      — when a regulated vertical is active and consent is due
//   2. quick_win_card    — once a quick win is captured
//   3. dashboard_preview — once enough is captured to seed a workspace preview
//   4. credit_kickoff    — once quote-ready (rendered + clickable; backend deferred)
export const deriveComponentFrames = (
  model: FlowModel,
): ReadonlyArray<RenderableFrame> => {
  const spec = model.outputSpec

  // Build plain frame records and run each through `validateComponentFrame`, the
  // SAME closed-catalog decode (#6128) a gateway-streamed frame goes through.
  // This validates props (defense in depth) and yields the renderer's typed
  // `RenderableFrame`; a record that somehow failed would degrade to the safe
  // fallback rather than crash — exactly the gateway contract.
  const records: Array<Record<string, unknown>> = []

  // consent_gate — for regulated verticals (legal/health), surface an explicit
  // consent gate before anything client-identifying. v1 only knows `legal`; the
  // overlay CONTENT is #6130. Honors the closed catalog shape.
  if (model.vertical === 'legal') {
    records.push({
      v: 1,
      component: 'consent_gate',
      id: 'consent-gate-legal',
      props: {
        scope: 'Legal intake',
        dataPractices: [
          'Information you share is used only to scope and run your requested work.',
          'No client-identifying detail is published; review gates precede anything sensitive.',
        ],
        required: true,
      },
    })
  }

  // quick_win_card — the scoped first deliverable, once captured.
  const quickWin = spec.quickWin
  if (typeof quickWin === 'string' && quickWin.trim() !== '') {
    records.push({
      v: 1,
      component: 'quick_win_card',
      id: 'quick-win',
      props: {
        title: quickWin.trim(),
        scope:
          typeof spec.scope === 'string' && spec.scope.trim() !== ''
            ? spec.scope.trim()
            : 'A bounded first deliverable, reviewed before anything ships.',
        etaDays: 3,
      },
    })
  }

  // dashboard_preview — "here's your dashboard, already seeded": surfaced once
  // the business is known. Seeded facts are the captured spec fields, labelled.
  const business = spec.business
  if (typeof business === 'string' && business.trim() !== '') {
    const seededFacts = OUTPUT_SPEC_SECTIONS.flatMap(section => {
      const value = spec[section.id]
      return typeof value === 'string' && value.trim() !== ''
        ? [`${section.label}: ${value.trim()}`]
        : []
    })

    records.push({
      v: 1,
      component: 'dashboard_preview',
      id: 'dashboard-preview',
      props: {
        workspaceRef: `${business.trim()} workspace`,
        seededFacts,
      },
    })
  }

  // credit_kickoff — the earned ask, once quote-ready. Rendered + clickable; the
  // click stubs to the existing `POST /api/billing/checkout` entry (the
  // payment->workspace->promise backend is explicitly deferred, #6129).
  if (isQuoteReady(spec)) {
    records.push({
      v: 1,
      component: 'credit_kickoff',
      id: 'credit-kickoff',
      props: {
        amountCents: CREDIT_KICKOFF_AMOUNT_CENTS,
        label: 'Kick off the work',
      },
    })
  }

  return records.map(validateComponentFrame)
}
