// Autopilot onboarding — the CLOSED component catalog (client side).
//
// This is the v1 closed catalog of onboarding component frames the Khala
// gateway streams over `event: oa.component` SSE frames (see the audit doc
// `docs/blitz/2026-06-23-autopilot-onboarding-intake-and-khala-audit.md`
// §4.3.1). The gateway validates these props server-side before a frame leaves
// Khala; this module re-validates them with Effect Schema at the client
// boundary (defense in depth) and is the source of truth for which components
// the Foldkit renderer will ever render.
//
// The decisive guarantee lives here and in `component-renderer.ts`: the catalog
// is a *closed* tagged union keyed by `component`, and the renderer does
// `registry[component] ?? fallback`. An unrecognized component renders a SAFE
// fallback (or the `human_handoff` view), never arbitrary model-authored
// markup — even if the gateway is bypassed.
//
// Issue #6127 owns the matching SERVER catalog independently; both mirror
// §4.3.1 exactly so the wire format lines up. Integration (#6129) reconciles
// them later.
import { Result, Schema as S } from 'effect'

import { parseJsonRecord } from '../../json-boundary'

// The wire envelope version. json-render lacks a version field; §4.3.1 adds
// one as the single thing not to copy.
export const COMPONENT_FRAME_VERSION = 1

// CLOSED catalog (v1). Adding a 7th component is a deliberate, reviewed bump,
// never an ad-hoc model invention. Keep this list in lockstep with the
// per-component prop schemas below and the renderer registry.
export const ComponentName = S.Literals([
  'credit_kickoff',
  'intake_progress',
  'quick_win_card',
  'dashboard_preview',
  'human_handoff',
  'consent_gate',
])
export type ComponentName = typeof ComponentName.Type

export const componentNames: ReadonlyArray<ComponentName> = ComponentName.literals

// PROP SCHEMAS — exact shapes from §4.3.1. Each is an Effect Schema struct.

// `credit_kickoff {amountCents, label}`
export const CreditKickoffProps = S.Struct({
  amountCents: S.Int.check(S.isGreaterThan(0)),
  label: S.NonEmptyString,
})
export type CreditKickoffProps = typeof CreditKickoffProps.Type

// `intake_progress {steps[], current}`
export const IntakeProgressStep = S.Struct({
  id: S.NonEmptyString,
  label: S.NonEmptyString,
})
export type IntakeProgressStep = typeof IntakeProgressStep.Type

export const IntakeProgressProps = S.Struct({
  steps: S.NonEmptyArray(IntakeProgressStep),
  current: S.Int.check(S.isGreaterThanOrEqualTo(0)),
})
export type IntakeProgressProps = typeof IntakeProgressProps.Type

// `quick_win_card {title, scope, etaDays}`
export const QuickWinCardProps = S.Struct({
  title: S.NonEmptyString,
  scope: S.NonEmptyString,
  etaDays: S.Int.check(S.isGreaterThan(0)),
})
export type QuickWinCardProps = typeof QuickWinCardProps.Type

// `dashboard_preview {workspaceRef, seededFacts[]}`
export const DashboardPreviewProps = S.Struct({
  workspaceRef: S.NonEmptyString,
  seededFacts: S.Array(S.NonEmptyString),
})
export type DashboardPreviewProps = typeof DashboardPreviewProps.Type

// `human_handoff {reason, contact}`
export const HumanHandoffProps = S.Struct({
  reason: S.NonEmptyString,
  contact: S.NonEmptyString,
})
export type HumanHandoffProps = typeof HumanHandoffProps.Type

// `consent_gate {scope, dataPractices, required}`
export const ConsentGateProps = S.Struct({
  scope: S.NonEmptyString,
  dataPractices: S.NonEmptyArray(S.NonEmptyString),
  required: S.Boolean,
})
export type ConsentGateProps = typeof ConsentGateProps.Type

// The validated frame as a tagged union over `component`. `props` is validated
// against the matching prop schema as a whole object (self-contained card; no
// per-field patch reassembly — §4.3.1 rejects json-render's element graph and
// JSON-Patch streaming).
const FrameStruct = <Name extends ComponentName, Props extends S.Struct.Fields>(
  component: Name,
  props: S.Struct<Props>,
) =>
  S.Struct({
    v: S.Literal(COMPONENT_FRAME_VERSION),
    component: S.Literal(component),
    props,
    id: S.NonEmptyString,
  })

export const CreditKickoffFrame = FrameStruct('credit_kickoff', CreditKickoffProps)
export const IntakeProgressFrame = FrameStruct('intake_progress', IntakeProgressProps)
export const QuickWinCardFrame = FrameStruct('quick_win_card', QuickWinCardProps)
export const DashboardPreviewFrame = FrameStruct(
  'dashboard_preview',
  DashboardPreviewProps,
)
export const HumanHandoffFrame = FrameStruct('human_handoff', HumanHandoffProps)
export const ConsentGateFrame = FrameStruct('consent_gate', ConsentGateProps)

// The closed component frame: a tagged union of the six catalog frames. Schema
// decode rejects any `component` not in the catalog and any frame whose props
// fail the matching schema.
export const ComponentFrame = S.Union([
  CreditKickoffFrame,
  IntakeProgressFrame,
  QuickWinCardFrame,
  DashboardPreviewFrame,
  HumanHandoffFrame,
  ConsentGateFrame,
])
export type ComponentFrame = typeof ComponentFrame.Type

// A frame that decoded but is NOT in the closed catalog (or failed prop
// validation). Carried so the renderer can render the SAFE fallback view and
// the page can surface a typed reason. Raw model material is never rendered.
export type UnknownComponentFrame = Readonly<{
  _tag: 'UnknownComponentFrame'
  // `component` may be any string the model emitted; it is shown as a label
  // only, never used to select markup.
  component: string
  id?: string
  reason: string
}>

export const UnknownComponentFrameValue = (input: {
  component: string
  id?: string
  reason: string
}): UnknownComponentFrame => ({
  _tag: 'UnknownComponentFrame',
  component: input.component,
  ...(input.id === undefined ? {} : { id: input.id }),
  reason: input.reason,
})

// A validated catalog frame, tagged so the renderer can switch over the
// renderable union without colliding with the unknown-fallback marker.
export type CatalogComponentFrame = Readonly<{
  _tag: 'CatalogComponentFrame'
  frame: ComponentFrame
}>

export const CatalogComponentFrameValue = (
  frame: ComponentFrame,
): CatalogComponentFrame => ({ _tag: 'CatalogComponentFrame', frame })

// The renderer input: either a validated catalog frame or the typed-unknown
// fallback marker. The renderer switches on this; it can never receive raw,
// unvalidated model output.
export type RenderableFrame = CatalogComponentFrame | UnknownComponentFrame

const decodeFrame = S.decodeUnknownResult(ComponentFrame)

const looksLikeFrameRecord = (
  value: unknown,
): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const componentLabel = (value: unknown): string => {
  if (looksLikeFrameRecord(value) && typeof value.component === 'string') {
    return value.component
  }

  return 'unknown'
}

const idLabel = (value: unknown): string | undefined => {
  if (looksLikeFrameRecord(value) && typeof value.id === 'string') {
    return value.id
  }

  return undefined
}

// Re-validate a single frame payload with Effect Schema (defense in depth,
// mirroring the gateway). Returns either a typed catalog frame or the
// typed-unknown fallback marker — never throws, so a misbehaving model can
// never crash the renderer.
export const validateComponentFrame = (value: unknown): RenderableFrame =>
  Result.match(decodeFrame(value), {
    onFailure: () => {
      const id = idLabel(value)
      const component = componentLabel(value)
      const reason = componentNames.includes(component as ComponentName)
        ? 'component_props_failed_schema_validation'
        : 'component_not_in_closed_catalog'

      return UnknownComponentFrameValue({
        component,
        ...(id === undefined ? {} : { id }),
        reason,
      })
    },
    onSuccess: frame => CatalogComponentFrameValue(frame),
  })

export const isUnknownComponentFrame = (
  frame: RenderableFrame,
): frame is UnknownComponentFrame => frame._tag === 'UnknownComponentFrame'

// SSE PARSE — reuse the existing client SSE-parse pattern from
// `page/loggedIn/customer-order/transitions.ts`: split on newlines, read
// `event:`/`data:` lines, JSON-parse the `data:` payload. We only surface
// `event: oa.component` frames here; prose `content` deltas are handled by the
// chat/onboarding stream separately. Standard OpenAI clients ignore the
// unknown `oa.component` event type, which is the additive-frame contract from
// §4.3.1.
export const COMPONENT_SSE_EVENT = 'oa.component'

type SseEvent = Readonly<{ event: string; data: string }>

// Split a raw SSE text body into discrete events. SSE events are separated by a
// blank line; each event accumulates `event:` and `data:` field lines.
const splitSseEvents = (text: string): ReadonlyArray<SseEvent> => {
  const blocks = text.split(/\r?\n\r?\n/)

  return blocks.flatMap(block => {
    const lines = block.split(/\r?\n/)
    const eventLine = lines.find(line => line.startsWith('event:'))
    const dataLines = lines
      .filter(line => line.startsWith('data:'))
      .map(line => line.slice('data:'.length).replace(/^ /, ''))

    if (dataLines.length === 0) {
      return []
    }

    return [
      {
        event:
          eventLine === undefined
            ? 'message'
            : eventLine.slice('event:'.length).replace(/^ /, '').trim(),
        data: dataLines.join('\n'),
      },
    ]
  })
}

// Parse a raw SSE body, validate every `oa.component` frame through the closed
// catalog, and return the renderable frames in order. Non-component events
// (prose `content` deltas, OpenAI chunks) are ignored here. Invalid component
// frames become typed-unknown fallback markers rather than being dropped, so
// the page can render the safe fallback and surface a reason.
export const parseComponentFrames = (
  text: string,
): ReadonlyArray<RenderableFrame> =>
  splitSseEvents(text)
    .filter(event => event.event === COMPONENT_SSE_EVENT)
    .map(event => validateComponentFrame(parseJsonRecord(event.data)))
