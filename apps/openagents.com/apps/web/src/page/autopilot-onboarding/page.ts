// Autopilot onboarding — the page view (issue #6129, UI follow-up #6123).
//
// Assembles the `/autopilot` onboarding HUD that mounts as the overlay of the
// SHARED persistent 3D scene at the `autopilot` pose (see
// `page/loggedOut/page/persistentScene.ts`). This module renders ONLY the
// command-canvas overlay — the conversation thread (with progressive Markdown +
// token streaming), the surfaced typed components (via the #6128 renderer), a
// compact sidebar intake register, and the pinned command composer. The
// persistent canvas, scrim, and camera pose are owned by `persistentScene.ts`
// and are NOT respawned here (one keyed canvas, design doc §1).
//
// This is a command canvas, not a chatbot (DESIGN.md): no centered chat bubble +
// sidebar + rounded feature-card cliche. Surfaces are panes/registers/strips over
// the living scene; energy is Khala blue; the credit signal is amber. All bodies
// build on `@openagentsinc/ui` AI Elements + the kit tokens — no hand-rolled
// styling, no duplicated tokens.
//
// UI follow-up (this change) fixes five real-user problems on the live page:
//   1. Markdown rendering — assistant prose renders through the centralized
//      `response` AI element (bold/headings/lists/code/links), streaming-tolerant.
//   2. Scroll — the thread is a FIXED-HEIGHT internal scroll region with the
//      composer pinned below; native scroll anchoring (a bottom sentinel) keeps
//      the newest message in view without fighting a user who has scrolled up,
//      and an explicit scroll-to-end command fires when the user sends a turn.
//   3. Intake progress — moved OUT of the main column into a slim sidebar
//      register (a lit checklist, not a giant box); on narrow viewports it
//      collapses to a thin progress strip above the thread.
//   4. Streaming — the assistant reply lands token-by-token into a live bubble
//      with a typing cursor; markdown renders progressively as deltas arrive.
//   5. Clutter — ONE intro treatment (the HUD header), no duplicated hero/first
//      message, and tightened speaker labels.

import type { Attribute, Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { AiElements, eyebrowClass, statusDotClass } from '@openagentsinc/ui'

import * as Ui from '../../ui'
import {
  type ComponentActionAttrs,
  renderComponentFrame,
} from './component-renderer'
import {
  type FlowModel,
  type IntakeRegisterStep,
  capturedSectionCount,
  deriveComponentFrames,
  deriveIntakeRegister,
  OUTPUT_SPEC_SECTIONS,
} from './flow'

// Public data attributes so tests / captures / the scroll command can locate the
// HUD surfaces.
export const HUD_ROOT_ATTR = 'autopilot-onboarding-hud'
export const HUD_TRANSCRIPT_ATTR = 'autopilot-onboarding-transcript'
export const HUD_COMPONENTS_ATTR = 'autopilot-onboarding-components'
export const HUD_COMPOSER_ATTR = 'autopilot-onboarding-composer'
export const HUD_COMPONENT_ITEM_ATTR = 'autopilot-onboarding-component'
export const HUD_REGISTER_ATTR = 'autopilot-onboarding-register'
export const HUD_THREAD_END_ATTR = 'autopilot-onboarding-thread-end'
// The selector the scroll-to-end command targets (problem #2).
export const HUD_THREAD_END_SELECTOR = `[data-${HUD_THREAD_END_ATTR}="true"]`
// The legal vertical overlay surfaces (VSL slot + verified stat strip). Present
// only on `/autopilot/legal`; absent on the generic `/autopilot` flow.
export const HUD_LEGAL_OVERLAY_ATTR = 'autopilot-onboarding-legal-overlay'
export const HUD_LEGAL_VSL_ATTR = 'autopilot-onboarding-legal-vsl'
export const HUD_LEGAL_STAT_STRIP_ATTR = 'autopilot-onboarding-legal-stats'

// The message hooks the page needs. Threading these in (rather than importing a
// concrete page `Message`) keeps this view decoupled and unit-testable: the
// loggedOut wiring supplies the real constructors; a test supplies stubs.
export type OnboardingViewActions<Message> = Readonly<{
  updatedComposer: (value: string) => Message
  submittedTurn: () => Message
  clickedCreditKickoff: () => Message
  clickedStartOver: () => Message
}>

const HUD_HEADING = 'Autopilot'

// The "start over" affordance: data attribute so tests/captures can locate it.
export const HUD_START_OVER_ATTR = 'autopilot-onboarding-start-over'

const introForVertical = (vertical: string | null): string =>
  vertical === 'legal'
    ? 'Describe your legal work. Autopilot scopes it, shows you a quick win, and keeps a review gate before anything is sent — no client-identifying detail leaves without your consent.'
    : 'Describe what you want done. Autopilot scopes the work, shows you a quick win, and keeps a human-review gate before anything ships.'

// LEGAL VERTICAL OVERLAY ---------------------------------------------------

// A single VERIFIED stat for the legal stat strip. Every figure here traces to a
// primary source the reader can open and check; the citation travels with the
// figure (issue #6130 hard constraint). Figures are drawn ONLY from the
// re-authenticated legal data sheet's "verified-to-primary-source" set
// (docs/blitz/lawpilot/source/DATA_SHEET.md + VERIFICATION_LEDGER.md). Removed or
// unverified figures are intentionally excluded; nothing here is a scarcity claim
// or an outcome projection.
export type LegalVerifiedStat = Readonly<{
  value: string
  label: string
  source: string
  sourceUrl: string
}>

export const LEGAL_VERIFIED_STATS: ReadonlyArray<LegalVerifiedStat> = [
  {
    value: '69%',
    label:
      'of legal professionals now use generative AI for work — more than double a year earlier.',
    source: '8am 2026 Legal Industry Report (n=1,395)',
    sourceUrl: 'https://www.8am.com/reports/legal-industry-report-2026/',
  },
  {
    value: '9%',
    label:
      'of firms have an actively enforced written AI policy; 43% have none and no plans to create one.',
    source: '8am 2026 Legal Industry Report',
    sourceUrl: 'https://www.8am.com/reports/legal-industry-report-2026/',
  },
  {
    value: 'ABA Op. 512',
    label:
      'requires understanding an AI tool and obtaining informed client consent before inputting client information.',
    source: 'ABA Formal Opinion 512 (July 29, 2024)',
    sourceUrl:
      'https://www.americanbar.org/content/dam/aba/administrative/professional_responsibility/ethics-opinions/aba-formal-opinion-512.pdf',
  },
]

// Legal-flavored starter prompts kept generic (no client/brand/person material):
// bounded, template-driven, verifiable first moments per the legal MVP shape.
const LEGAL_STARTER_PROMPTS: ReadonlyArray<string> = [
  'Prepare a draft NDA prep packet for a routine vendor conversation.',
  'Find a fitting formation/intake template and list the missing facts.',
  'Build a lawyer-review checklist for a routine document.',
]

// The legal-only overlay: the VSL slot (a placeholder embed region — no real
// video asset is required for v1), a legal-specific framing line, a bounded set
// of starter prompts, and the verified stat strip. Rendered ONLY for the `legal`
// vertical; the generic `/autopilot` flow never sees it.
const legalOverlaySection = <Message>(model: FlowModel): Html => {
  const h = html<Message>()

  if (model.vertical !== 'legal') {
    return h.empty
  }

  return h.div(
    [
      h.DataAttribute(HUD_LEGAL_OVERLAY_ATTR, ''),
      Ui.className<Message>(
        'grid gap-4 border border-[#3a7bff]/20 bg-black/40 p-4',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('grid gap-1.5')],
        [
          h.span([Ui.className<Message>(eyebrowClass)], ['For legal teams']),
          h.p(
            [
              Ui.className<Message>(
                'm-0 max-w-[60ch] text-[0.8125rem] leading-[1.5] text-white/65',
              ),
            ],
            [
              'Stay in expert review mode. You share only the source material you choose; Autopilot prepares a bounded, template-driven, source-linked work surface — a draft prep packet, intake questions, and a lawyer-review checklist — with an attorney-review gate before anything is sent. Not an AI lawyer, not case-law research.',
            ],
          ),
        ],
      ),
      // VSL slot — a placeholder embed region. No real video asset is required
      // for v1; the region reserves the space and is labelled so the asset can
      // drop in later without a layout change.
      h.div(
        [
          h.DataAttribute(HUD_LEGAL_VSL_ATTR, ''),
          h.AriaLabel('Legal overview video'),
          Ui.className<Message>(
            'grid aspect-video place-items-center border border-[#222] bg-[#010102] text-center',
          ),
        ],
        [
          h.span(
            [Ui.className<Message>('text-[0.75rem] text-white/45')],
            ['Overview video — coming soon'],
          ),
        ],
      ),
      h.div(
        [Ui.className<Message>('grid gap-2')],
        [
          h.span(
            [Ui.className<Message>(eyebrowClass)],
            ['Bounded first moves'],
          ),
          h.ul(
            [Ui.className<Message>('m-0 grid list-none gap-1.5 p-0')],
            LEGAL_STARTER_PROMPTS.map(prompt =>
              h.li(
                [
                  Ui.className<Message>(
                    'text-[0.8125rem] leading-[1.4] text-white/60',
                  ),
                ],
                [prompt],
              ),
            ),
          ),
        ],
      ),
      legalStatStripView<Message>(),
    ],
  )
}

// The verified stat strip. A single strip of primary-sourced figures, each with
// its citation as an openable link. No scarcity, no projections, no unproven
// numbers — only figures the legal data sheet marks verified-to-primary-source.
const legalStatStripView = <Message>(): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute(HUD_LEGAL_STAT_STRIP_ATTR, ''),
      Ui.className<Message>(
        'grid gap-3 border-t border-[#222] pt-3 sm:grid-cols-3',
      ),
    ],
    LEGAL_VERIFIED_STATS.map(stat =>
      h.div(
        [Ui.className<Message>('grid gap-1')],
        [
          h.span(
            [
              Ui.className<Message>(
                'font-medium text-[#f1efe8] text-lg tabular-nums',
              ),
            ],
            [stat.value],
          ),
          h.span(
            [
              Ui.className<Message>(
                'text-[0.75rem] leading-[1.4] text-white/55',
              ),
            ],
            [stat.label],
          ),
          h.a(
            [
              h.Href(stat.sourceUrl),
              h.Target('_blank'),
              h.Rel('noopener noreferrer'),
              Ui.className<Message>(
                'text-[0.6875rem] text-[#7aa2ff] underline decoration-[#3a7bff]/40 underline-offset-2 hover:text-[#a8c2ff]',
              ),
            ],
            [stat.source],
          ),
        ],
      ),
    ),
  )
}

// TRANSCRIPT --------------------------------------------------------------

// One transcript turn. The assistant body renders as Markdown through the
// centralized `response` element (problem #1); the user body is plain text.
const transcriptTurnView = <Message>(turn: {
  role: 'user' | 'assistant'
  content: string
}): Html =>
  turn.role === 'assistant'
    ? AiElements.message<Message>({
        props: { role: 'assistant', author: HUD_HEADING },
        markdown: turn.content,
      })
    : AiElements.message<Message>({
        props: { role: 'user', body: turn.content, author: 'You' },
      })

// The in-flight streaming assistant bubble (problem #4): renders the partial
// reply as progressive Markdown with a live typing cursor. Only present while a
// reply is streaming.
const streamingTurnView = <Message>(partial: string): Html =>
  AiElements.message<Message>({
    props: { role: 'assistant', author: HUD_HEADING },
    markdown: partial,
    streaming: true,
  })

// The conversation thread: the running transcript plus the in-flight streaming
// bubble, and a bottom sentinel the scroll-to-end command and native scroll
// anchoring both target. The intro is NOT duplicated here (problem #5) — it lives
// once in the HUD header. An empty thread shows a calm starter line so the
// surface reads complete on first paint (no class-gated blank).
const transcriptView = <Message>(model: FlowModel): Html => {
  const h = html<Message>()

  const isEmpty =
    model.transcript.length === 0 && model.streamingReply === null

  return h.div(
    [
      h.DataAttribute(HUD_TRANSCRIPT_ATTR, ''),
      Ui.className<Message>('grid gap-3'),
    ],
    [
      isEmpty
        ? h.p(
            [
              Ui.className<Message>(
                'm-0 text-[0.8125rem] leading-[1.5] text-white/45',
              ),
            ],
            ['Send a message to start. Autopilot will scope the work with you.'],
          )
        : h.empty,
      ...model.transcript.map(turn => transcriptTurnView<Message>(turn)),
      model.streamingReply === null
        ? h.empty
        : streamingTurnView<Message>(model.streamingReply),
      // Bottom sentinel: the scroll-to-end target (problem #2). `overflow-anchor`
      // lets the browser keep this in view as content grows ONLY when the user is
      // already at the bottom — native "don't fight the user" behavior.
      h.div(
        [
          h.DataAttribute(HUD_THREAD_END_ATTR, 'true'),
          Ui.className<Message>('h-px w-full [overflow-anchor:auto]'),
        ],
        [],
      ),
    ],
  )
}

// INTAKE REGISTER (SIDEBAR) -----------------------------------------------

const registerRowTone = (status: IntakeRegisterStep['status']) =>
  status === 'done'
    ? ('positive' as const)
    : status === 'active'
      ? ('info' as const)
      : ('neutral' as const)

const registerRowView = <Message>(step: IntakeRegisterStep): Html => {
  const h = html<Message>()

  return h.li(
    [
      Ui.className<Message>(
        `flex items-center gap-2 text-[0.75rem] leading-[1.3] ${
          step.status === 'done'
            ? 'text-white/55'
            : step.status === 'active'
              ? 'text-[#f1efe8]'
              : 'text-white/35'
        }`,
      ),
    ],
    [
      h.span([Ui.className<Message>(statusDotClass(registerRowTone(step.status)))], []),
      h.span([Ui.className<Message>('min-w-0 truncate')], [step.label]),
    ],
  )
}

// The compact sidebar intake register (problem #3): the 10 Output-Spec sections
// as a slim vertical register that lights up / checks off as each is captured —
// glanceable, never dominating the column. A small captured/total count anchors
// it. On narrow viewports the parent grid stacks it above the thread as a thin
// strip.
const intakeRegisterView = <Message>(model: FlowModel): Html => {
  const h = html<Message>()
  const steps = deriveIntakeRegister(model)
  const captured = capturedSectionCount(model.outputSpec)

  return h.aside(
    [
      h.DataAttribute(HUD_REGISTER_ATTR, ''),
      h.AriaLabel('Intake progress'),
      Ui.className<Message>(
        'grid content-start gap-2 border border-[#222] bg-black/40 p-3',
      ),
    ],
    [
      h.div(
        [Ui.className<Message>('flex items-center justify-between gap-2')],
        [
          h.span([Ui.className<Message>(eyebrowClass)], ['Intake']),
          h.span(
            [
              Ui.className<Message>(
                'text-[0.6875rem] tabular-nums text-white/45',
              ),
            ],
            [`${captured}/${OUTPUT_SPEC_SECTIONS.length}`],
          ),
        ],
      ),
      h.ul(
        [Ui.className<Message>('m-0 grid list-none gap-1.5 p-0')],
        steps.map(step => registerRowView<Message>(step)),
      ),
    ],
  )
}

// INLINE COMPONENTS -------------------------------------------------------

// The surfaced typed components, each wrapped in a flutter-in surface whose
// stagger index is its document order. `credit_kickoff` is wired clickable to
// the page's kickoff action; every other component is presentational in v1.
const componentsView = <Message>(
  model: FlowModel,
  actions: OnboardingViewActions<Message>,
): Html => {
  const h = html<Message>()
  const frames = deriveComponentFrames(model)

  if (frames.length === 0) {
    return h.empty
  }

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

// COMPOSER ----------------------------------------------------------------

// The command composer: transport-agnostic text input (voice deferred — the
// composer states leave room for `listening`/`speaking` later). Maps the prompt
// status from the flow status so the submit control tracks the request.
const composerView = <Message>(
  model: FlowModel,
  actions: OnboardingViewActions<Message>,
): Html => {
  const h = html<Message>()

  const inFlight =
    model.status === 'submitting' || model.status === 'streaming'

  const status =
    model.status === 'streaming'
      ? ('streaming' as const)
      : model.status === 'submitting'
        ? ('submitted' as const)
        : model.status === 'error'
          ? ('error' as const)
          : ('ready' as const)

  const submitAttrs: ReadonlyArray<Attribute<Message>> =
    inFlight || model.composerDraft.trim() === '' ? [h.Disabled(true)] : []

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

// START OVER --------------------------------------------------------------

// A quiet "Start over" control. Present only once a conversation exists (a
// session, transcript, or in-flight stream), so the first paint stays clean. It
// drops the persisted/restored session and begins fresh.
const startOverControl = <Message>(
  model: FlowModel,
  actions: OnboardingViewActions<Message>,
): Html => {
  const h = html<Message>()

  const hasConversation =
    model.sessionId !== null ||
    model.transcript.length > 0 ||
    model.streamingReply !== null

  if (!hasConversation) {
    return h.empty
  }

  return h.button(
    [
      h.Type('button'),
      h.DataAttribute(HUD_START_OVER_ATTR, ''),
      h.OnClick(actions.clickedStartOver()),
      Ui.className<Message>(
        'shrink-0 border border-[#222] px-2.5 py-1 text-[0.6875rem] text-white/55 transition-colors hover:border-[#3a7bff]/40 hover:text-white/80',
      ),
    ],
    ['Start over'],
  )
}

// OVERLAY -----------------------------------------------------------------

// The HUD overlay: an operational command pane floating over the dimmed scene.
// One sanctioned glass surface (design doc §9) — a breath of the scene bleeding
// through the panel — because it ties the DOM HUD to the 3D backdrop. The pane is
// a single bordered surface (never nested cards) laid out as a two-column grid on
// wide viewports: a slim intake register rail + the main thread column. The
// composer is pinned BELOW the scroll region so it never scrolls away; only the
// thread + inline components scroll internally (problem #2).
export const overlayView = <Message>(
  model: FlowModel,
  actions: OnboardingViewActions<Message>,
): Html => {
  const h = html<Message>()

  // The scrollable region: the thread + inline components. Fixed max height with
  // internal overflow so the page itself does not scroll; native scroll anchoring
  // (the bottom sentinel) keeps the newest content in view.
  const scrollRegion = h.div(
    [
      Ui.className<Message>(
        'oa-thread-scroll grid min-h-0 content-start gap-3 overflow-y-auto pr-1',
      ),
    ],
    [
      transcriptView<Message>(model),
      componentsView<Message>(model, actions),
    ],
  )

  return h.div(
    [
      h.DataAttribute(HUD_ROOT_ATTR, ''),
      h.AriaLabel('Autopilot onboarding'),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-10 flex items-stretch justify-center px-4 py-6 sm:py-10',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'pointer-events-auto m-auto grid max-h-full w-[min(100%,60rem)] content-start gap-5 border border-[#3a7bff]/25 bg-black/55 p-4 backdrop-blur-md khala-glow sm:p-6',
          ),
        ],
        [
          // ONE intro treatment (problem #5): the HUD header. The first-message
          // duplicate is gone; the thread starts empty with a calm starter line.
          // A quiet "Start over" control appears once a conversation exists so a
          // visitor can drop the restored/persisted session and begin fresh.
          h.div(
            [
              Ui.className<Message>(
                'flex items-start justify-between gap-3',
              ),
            ],
            [
              h.div(
                [Ui.className<Message>('grid gap-1.5')],
                [
                  h.span([Ui.className<Message>(eyebrowClass)], [HUD_HEADING]),
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
              startOverControl<Message>(model, actions),
            ],
          ),
          legalOverlaySection<Message>(model),
          // Two-column work area: the intake register rail + the thread column.
          // `min-h-0` on the grid + the scroll region is what lets the inner
          // overflow actually scroll inside the capped pane. On narrow viewports
          // the rail stacks above the thread (a thin progress strip).
          h.div(
            [
              Ui.className<Message>(
                'grid min-h-0 gap-4 sm:grid-cols-[minmax(0,1fr)_13rem] sm:gap-5',
              ),
            ],
            [
              // Thread column first in source order so it leads the document /
              // a11y order; the rail is pulled to the right on wide via grid
              // column placement.
              h.div(
                [
                  Ui.className<Message>(
                    'grid min-h-0 content-start gap-4 sm:col-start-1 sm:row-start-1',
                  ),
                ],
                [scrollRegion, composerView<Message>(model, actions)],
              ),
              h.div(
                [Ui.className<Message>('sm:col-start-2 sm:row-start-1')],
                [intakeRegisterView<Message>(model)],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}
