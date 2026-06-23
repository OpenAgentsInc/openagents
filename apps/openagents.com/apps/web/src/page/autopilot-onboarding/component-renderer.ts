// Autopilot onboarding — the CLOSED component renderer (client side).
//
// Renders the validated v1 catalog frames (see `./component-catalog.ts` and the
// audit doc §4.3.1) as Foldkit views built from the centralized
// `@openagentsinc/ui` AI Elements + primitives. No hand-rolled styling and no
// duplicated tokens: cards reuse the shared message/task/confirmation elements
// and the kit class constants.
//
// THE GUARANTEE: a closed registry keyed by `component` name, looked up as
// `registry[component] ?? fallback`. A frame that is not in the closed catalog
// (or whose props failed Effect Schema validation) renders the SAFE fallback —
// the `human_handoff` view — never arbitrary model-authored markup. This holds
// even if the gateway is bypassed, because `validateComponentFrame` only ever
// hands the renderer a typed `RenderableFrame`.
import { Match as M } from 'effect'
import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  AiElements,
  eyebrowClass,
  metaClass,
  surfaceClass,
  titleClass,
} from '@openagentsinc/ui'

import {
  type ComponentFrame,
  type ComponentName,
  type ConsentGateProps,
  type CreditKickoffProps,
  type DashboardPreviewProps,
  type HumanHandoffProps,
  type IntakeProgressProps,
  type QuickWinCardProps,
  type RenderableFrame,
} from './component-catalog'

const cardClass = `grid gap-2 ${surfaceClass} p-3`

// Optional action attributes (e.g. an `OnClick`) the page may supply per
// component id so interactive cards can dispatch. The renderer never invents
// messages itself (Elm discipline); page assembly (#6129) wires these.
export type ComponentActionAttrs<Message> = Partial<
  Record<ComponentName, ReadonlyArray<Attribute<Message>>>
>

const formatCents = (amountCents: number): string =>
  `$${(amountCents / 100).toLocaleString('en-US', {
    minimumFractionDigits: 0,
    maximumFractionDigits: 2,
  })}`

// `credit_kickoff` — "Kick off with $500 in credits". A confirmation gate with
// the credit amount and a primary action; the action click is wired by the page.
const creditKickoffView = <Message>(
  props: CreditKickoffProps,
  actionAttrs: ReadonlyArray<Attribute<Message>>,
): Html =>
  AiElements.confirmation<Message>({
    props: {
      title: props.label,
      state: 'requested',
      detail: `${formatCents(props.amountCents)} in credits to start the work.`,
      approveLabel: `Add ${formatCents(props.amountCents)} in credits`,
    },
    actions: [
      AiElements.confirmationAction<Message>({
        label: `Add ${formatCents(props.amountCents)} in credits`,
        variant: 'primary',
        attrs: actionAttrs,
      }),
    ],
  })

// `intake_progress` — the onboarding interview progress, as a task list with one
// item per step; the current step is `active`, prior steps `done`, later ones
// `queued`.
const intakeProgressView = <Message>(props: IntakeProgressProps): Html =>
  AiElements.task<Message>({
    props: {
      title: 'Onboarding progress',
      open: true,
      items: props.steps.map((step, index) => ({
        label: step.label,
        status:
          index < props.current
            ? ('done' as const)
            : index === props.current
              ? ('active' as const)
              : ('queued' as const),
      })),
    },
  })

// `quick_win_card` — the scoped first deliverable with an ETA.
const quickWinCardView = <Message>(props: QuickWinCardProps): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class(cardClass)],
    [
      h.span([h.Class(eyebrowClass)], ['Quick win']),
      h.span([h.Class(titleClass)], [props.title]),
      h.p([h.Class(metaClass)], [props.scope]),
      h.p(
        [h.Class(metaClass)],
        [`Estimated ${props.etaDays} day${props.etaDays === 1 ? '' : 's'}`],
      ),
    ],
  )
}

// `dashboard_preview` — "here's your dashboard with your info in it": the seeded
// workspace ref and the public-safe facts already loaded.
const dashboardPreviewView = <Message>(props: DashboardPreviewProps): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class(cardClass)],
    [
      h.span([h.Class(eyebrowClass)], ['Your dashboard']),
      h.span([h.Class(titleClass)], [props.workspaceRef]),
      props.seededFacts.length === 0
        ? h.p([h.Class(metaClass)], ['No facts seeded yet'])
        : h.ul(
            [h.Class('grid gap-1')],
            props.seededFacts.map(fact =>
              h.li([h.Class(metaClass)], [fact]),
            ),
          ),
    ],
  )
}

// `human_handoff` — escalate to a person. This is also the SAFE FALLBACK view
// for any unknown / invalid frame.
const humanHandoffView = <Message>(props: HumanHandoffProps): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class(cardClass)],
    [
      h.span([h.Class(eyebrowClass)], ['Handoff to a person']),
      h.p([h.Class(metaClass)], [props.reason]),
      h.p([h.Class(titleClass)], [props.contact]),
    ],
  )
}

// `consent_gate` — explicit consent before sensitive data / inference, with the
// data practices listed. A confirmation gate; the action click is wired by the
// page.
const consentGateView = <Message>(
  props: ConsentGateProps,
  actionAttrs: ReadonlyArray<Attribute<Message>>,
): Html => {
  const h = html<Message>()

  return h.div(
    [h.Class('grid gap-2')],
    [
      h.div(
        [h.Class(cardClass)],
        [
          h.span([h.Class(eyebrowClass)], ['Data practices']),
          h.ul(
            [h.Class('grid gap-1')],
            props.dataPractices.map(practice =>
              h.li([h.Class(metaClass)], [practice]),
            ),
          ),
        ],
      ),
      AiElements.confirmation<Message>({
        props: {
          title: props.scope,
          state: 'requested',
          detail: props.required
            ? 'Your consent is required to continue.'
            : 'You can decline and continue with limited scope.',
          approveLabel: 'I consent',
        },
        actions: [
          AiElements.confirmationAction<Message>({
            label: 'I consent',
            variant: 'primary',
            attrs: actionAttrs,
          }),
        ],
      }),
    ],
  )
}

// THE CLOSED REGISTRY: catalog component name -> view. Because the input is a
// validated `ComponentFrame` tagged union, this match is exhaustive over the
// closed set. There is no string-keyed dynamic dispatch into arbitrary markup.
const renderCatalogFrame = <Message>(
  frame: ComponentFrame,
  actions: ComponentActionAttrs<Message>,
): Html =>
  M.value(frame).pipe(
    M.when({ component: 'credit_kickoff' }, ({ props }) =>
      creditKickoffView<Message>(props, actions.credit_kickoff ?? []),
    ),
    M.when({ component: 'intake_progress' }, ({ props }) =>
      intakeProgressView<Message>(props),
    ),
    M.when({ component: 'quick_win_card' }, ({ props }) =>
      quickWinCardView<Message>(props),
    ),
    M.when({ component: 'dashboard_preview' }, ({ props }) =>
      dashboardPreviewView<Message>(props),
    ),
    M.when({ component: 'human_handoff' }, ({ props }) =>
      humanHandoffView<Message>(props),
    ),
    M.when({ component: 'consent_gate' }, ({ props }) =>
      consentGateView<Message>(props, actions.consent_gate ?? []),
    ),
    M.exhaustive,
  )

// The SAFE FALLBACK for any frame outside the closed catalog (unknown component
// name, or props that failed schema validation). Renders the `human_handoff`
// view rather than anything model-authored, so the user is never shown broken
// or untrusted UI. The model-supplied component name and reason are
// deliberately NOT rendered here — they are operator/debug evidence on the
// `UnknownComponentFrame`, never user-facing markup.
const fallbackView = <Message>(): Html =>
  humanHandoffView<Message>({
    reason: 'We hit something we could not safely render automatically.',
    contact: 'A teammate will follow up with you directly.',
  })

// Render a single validated frame. `registry[component] ?? fallback` is realized
// here: a `CatalogComponentFrame` hits the closed registry; an
// `UnknownComponentFrame` hits the safe fallback.
export const renderComponentFrame = <Message>(
  frame: RenderableFrame,
  actions: ComponentActionAttrs<Message> = {},
): Html =>
  frame._tag === 'CatalogComponentFrame'
    ? renderCatalogFrame<Message>(frame.frame, actions)
    : fallbackView<Message>()

// Render an ordered list of frames (e.g. the components surfaced so far in an
// onboarding session). Prose deltas are rendered separately by the page; this
// only renders component frames so the two interleave by document order.
export const renderComponentFrames = <Message>(
  frames: ReadonlyArray<RenderableFrame>,
  actions: ComponentActionAttrs<Message> = {},
): ReadonlyArray<Html> =>
  frames.map(frame => renderComponentFrame<Message>(frame, actions))
