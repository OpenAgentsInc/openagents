// Autopilot onboarding — the page view (issue #6129).
//
// Assembles the `/autopilot` onboarding HUD that mounts as the overlay of the
// SHARED persistent 3D scene at the `autopilot` pose (see
// `page/loggedOut/page/persistentScene.ts`). This module renders ONLY the
// command-canvas overlay — the conversation transcript, the streamed/surfaced
// typed components (via the #6128 renderer), and the command composer. The
// persistent canvas, scrim, and camera pose are owned by `persistentScene.ts`
// and are NOT respawned here (one keyed canvas, design doc §1).
//
// This is a command canvas, not a chatbot (DESIGN.md): no centered chat bubble +
// sidebar + rounded feature-card cliche. Surfaces are panes/registers/strips over
// the living scene; energy is Khala blue; the credit signal is amber. All bodies
// build on `@openagentsinc/ui` AI Elements + the kit tokens — no hand-rolled
// styling, no duplicated tokens.
//
// Spatial behavior landed in v1: components flutter in deterministically
// (`oa-flutter-in`, staggered by document order, with a `prefers-reduced-motion`
// fallback). The full 3D-anchored `htmlOverlayPrimitives` choreography (cards
// tracking scene anchors as the camera drifts) is a documented follow-up — see
// the report and the design doc §12; v1 ships the clean, working, on-brand flow.

import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { AiElements, eyebrowClass } from '@openagentsinc/ui'

import * as Ui from '../../ui'
import {
  type ComponentActionAttrs,
  renderComponentFrame,
} from './component-renderer'
import {
  type FlowModel,
  deriveComponentFrames,
} from './flow'

// Public data attributes so tests / captures can locate the HUD surfaces.
export const HUD_ROOT_ATTR = 'autopilot-onboarding-hud'
export const HUD_TRANSCRIPT_ATTR = 'autopilot-onboarding-transcript'
export const HUD_COMPONENTS_ATTR = 'autopilot-onboarding-components'
export const HUD_COMPOSER_ATTR = 'autopilot-onboarding-composer'
export const HUD_COMPONENT_ITEM_ATTR = 'autopilot-onboarding-component'

// The message hooks the page needs. Threading these in (rather than importing a
// concrete page `Message`) keeps this view decoupled and unit-testable: the
// loggedOut wiring supplies the real constructors; a test supplies stubs.
export type OnboardingViewActions<Message> = Readonly<{
  updatedComposer: (value: string) => Message
  submittedTurn: () => Message
  clickedCreditKickoff: () => Message
}>

const HUD_HEADING = 'Autopilot'

const introForVertical = (vertical: string | null): string =>
  vertical === 'legal'
    ? 'Describe your legal work. Autopilot scopes it, shows you a quick win, and keeps a review gate before anything is sent — no client-identifying detail leaves without your consent.'
    : 'Describe what you want done. Autopilot scopes the work, shows you a quick win, and keeps a human-review gate before anything ships.'

// One transcript turn rendered as an AI Elements `message`. The agent surface is
// the assistant role; the visitor is the user role.
const transcriptTurnView = <Message>(turn: {
  role: 'user' | 'assistant'
  content: string
}): Html =>
  AiElements.message<Message>({
    props: {
      role: turn.role,
      body: turn.content,
      author: turn.role === 'assistant' ? 'Autopilot' : 'You',
    },
  })

// The conversation register: the agent's opening line plus the running
// transcript. Renders the opening even before the first turn so a headless
// render shows a complete, readable surface (no class-gated blank).
const transcriptView = <Message>(model: FlowModel): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute(HUD_TRANSCRIPT_ATTR, ''),
      Ui.className<Message>('grid gap-3'),
    ],
    [
      AiElements.message<Message>({
        props: {
          role: 'assistant',
          body: introForVertical(model.vertical),
          author: 'Autopilot',
        },
      }),
      ...model.transcript.map(turn => transcriptTurnView<Message>(turn)),
    ],
  )
}

// The surfaced typed components, each wrapped in a flutter-in surface whose
// stagger index is its document order. `credit_kickoff` is wired clickable to
// the page's kickoff action; every other component is presentational in v1.
const componentsView = <Message>(
  model: FlowModel,
  actions: OnboardingViewActions<Message>,
): Html => {
  const h = html<Message>()
  const frames = deriveComponentFrames(model)

  const componentActions: ComponentActionAttrs<Message> = {
    credit_kickoff: [h.OnClick(actions.clickedCreditKickoff())],
  }

  return h.div(
    [
      h.DataAttribute(HUD_COMPONENTS_ATTR, ''),
      Ui.className<Message>('grid gap-3'),
    ],
    frames.map((frame, index) =>
      h.div(
        [
          h.DataAttribute(HUD_COMPONENT_ITEM_ATTR, String(index)),
          Ui.className<Message>('oa-flutter-in'),
          // Deterministic stagger from document order (design doc §11). Not
          // Math.random / time-of-day, so captures and replays are stable; the
          // CSS reduced-motion fallback drops the animation entirely.
          h.Style({ animationDelay: `${index * 70}ms` }),
        ],
        [renderComponentFrame<Message>(frame, componentActions)],
      ),
    ),
  )
}

// The command composer: transport-agnostic text input (voice deferred — the
// composer states leave room for `listening`/`speaking` later). Maps the prompt
// status from the flow status so the submit control tracks the request.
const composerView = <Message>(
  model: FlowModel,
  actions: OnboardingViewActions<Message>,
): Html => {
  const h = html<Message>()

  const status =
    model.status === 'submitting'
      ? ('submitted' as const)
      : model.status === 'error'
        ? ('error' as const)
        : ('ready' as const)

  const submitAttrs: ReadonlyArray<Attribute<Message>> =
    model.status === 'submitting' || model.composerDraft.trim() === ''
      ? [h.Disabled(true)]
      : []

  return h.div(
    [
      h.DataAttribute(HUD_COMPOSER_ATTR, ''),
      Ui.className<Message>('grid gap-2'),
    ],
    [
      model.status === 'error' && model.errorReason !== null
        ? h.p(
            [
              h.Role('alert'),
              Ui.className<Message>('m-0 text-[0.75rem] text-[#ff6f00]'),
            ],
            [model.errorReason],
          )
        : h.empty,
      AiElements.promptInput<Message>({
        props: {
          name: 'autopilot-onboarding-message',
          placeholder: 'Tell Autopilot what you want done…',
          value: model.composerDraft,
          status,
          submitLabel: 'Send',
        },
        formAttrs: [h.OnSubmit(actions.submittedTurn())],
        textareaAttrs: [
          h.Value(model.composerDraft),
          h.OnInput(value => actions.updatedComposer(value)),
          h.AriaLabel('Tell Autopilot what you want done'),
        ],
        submitAttrs,
      }),
    ],
  )
}

// The HUD overlay: an operational command pane floating over the dimmed scene.
// One sanctioned glass surface (design doc §9) — a breath of the scene bleeding
// through the panel — because it ties the DOM HUD to the 3D backdrop. The pane
// is a single bordered surface, never nested cards.
export const overlayView = <Message>(
  model: FlowModel,
  actions: OnboardingViewActions<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute(HUD_ROOT_ATTR, ''),
      h.AriaLabel('Autopilot onboarding'),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-10 flex items-stretch justify-center overflow-y-auto px-4 py-6 sm:py-10',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-auto m-auto grid w-[min(100%,46rem)] content-start gap-5 border border-[#3a7bff]/25 bg-black/55 p-4 backdrop-blur-md khala-glow sm:p-6',
          ),
        ],
        [
          h.div(
            [Ui.className<Message>('grid gap-1.5')],
            [
              h.span(
                [Ui.className<Message>(eyebrowClass)],
                [HUD_HEADING],
              ),
              // The HUD heading + intro are prose, not strip rows, so they use
              // explicit on-brand classes (mono off-white, balanced) rather than
              // the single-line-truncating `titleClass`/`metaClass` strip tokens.
              h.h1(
                [
                  Ui.className<Message>(
                    'm-0 text-balance font-medium text-[#f1efe8] text-2xl sm:text-3xl',
                  ),
                ],
                ['Put an AI workforce to work'],
              ),
              h.p(
                [
                  Ui.className<Message>(
                    'm-0 max-w-[60ch] text-[0.8125rem] leading-[1.5] text-white/60',
                  ),
                ],
                [introForVertical(model.vertical)],
              ),
            ],
          ),
          transcriptView<Message>(model),
          componentsView<Message>(model, actions),
          composerView<Message>(model, actions),
        ],
      ),
    ],
  )
}
