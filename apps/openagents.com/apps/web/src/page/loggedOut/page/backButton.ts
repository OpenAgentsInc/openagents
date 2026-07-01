import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { statsRouter } from '../../../route'
import * as Ui from '../../../ui'
import { ClickedExitKhala } from '../message'
import type { Message } from '../message'
import type { PublicKhalaTokensServedModel } from '../model'
import { formatKhalaTokensServed } from './home'

// Shared "← OpenAgents" back-home control for the persistent-scene overlays
// (/tassadar, /khala). It pins to the top-left of the dimmed scene and dispatches
// the existing `ClickedExitKhala` navigate-home message (→ NavigateToLanding).
// Both surfaces render the SAME markup/styling so the back affordance is
// visually identical across the scene's poses. Protoss house style: dark glass,
// khala-blue, reduced-motion safe (see root DESIGN.md + styles.css khala-*).

export const backButtonWrapClass = 'fixed left-4 top-4 z-20 sm:left-6 sm:top-6'

export const backButtonClass =
  'khala-focus group pointer-events-auto inline-flex items-center gap-2 rounded-full ' +
  'border border-[#3a7bff]/45 bg-[#070b12]/80 px-4 py-2 font-mono text-xs font-semibold ' +
  'uppercase tracking-[0.2em] text-[#bcd4ff] backdrop-blur-md transition-all duration-300 ' +
  'ease-out hover:border-[#4fd0ff]/80 hover:text-white hover:khala-glow ' +
  'motion-reduce:transition-none'

export const backArrowClass =
  'text-[#4fd0ff] transition-transform duration-300 ease-out group-hover:-translate-x-0.5 ' +
  'motion-reduce:transition-none'

// The homepage pill carries the SAME glass-pill look as the back button but
// must survive a long label ("Tokens Served:") plus a live thousands-
// separated total in the narrow top-left slot. On phones the slot competes with
// the hero, so the pill tightens its padding/letter-spacing, abbreviates the
// label to "Tokens:" (the full label is kept in the DOM, just visually hidden, so
// the accessible name and the text-content assertions stay intact), and caps its
// width to the viewport so a very large total can never push it off-screen or
// over the hero. At `sm`+ it relaxes to the full back-button geometry and the
// full "Tokens Served:" label. `max-w` + `truncate` are the belt-and-
// suspenders guard; the abbreviation is what actually keeps it tidy on mobile.
export const khalaTokensServedPillClass =
  'khala-focus group pointer-events-auto inline-flex max-w-[calc(100vw-2rem)] cursor-pointer ' +
  'items-center gap-1.5 truncate rounded-full border border-[#3a7bff]/45 ' +
  'bg-[#070b12]/80 px-3 py-1.5 font-mono text-[0.7rem] font-semibold uppercase ' +
  'tracking-[0.12em] text-[#bcd4ff] backdrop-blur-md transition-all duration-300 ' +
  'ease-out hover:border-[#4fd0ff]/80 hover:text-white hover:khala-glow ' +
  'motion-reduce:transition-none ' +
  'sm:max-w-none sm:gap-2 sm:px-4 sm:py-2 sm:text-xs sm:tracking-[0.2em]'

// `surface` distinguishes the data attribute per pose for tests/captures while
// keeping identical markup, styling, position, and the same navigate-home wire.
export const backButton = (surface: 'tassadar' | 'khala'): Html => {
  const h = html<Message>()
  return h.div(
    [Ui.className<Message>(backButtonWrapClass)],
    [
      h.button(
        [
          h.Type('button'),
          h.OnClick(ClickedExitKhala()),
          h.AriaLabel('Back to OpenAgents home'),
          h.DataAttribute(`${surface}-back`, 'home'),
          Ui.className<Message>(backButtonClass),
        ],
        [
          h.span([Ui.className<Message>(backArrowClass)], ['←']),
          h.span([], ['OpenAgents']),
        ],
      ),
    ],
  )
}

// The homepage twin of the back button: it occupies the SAME top-left slot the
// back affordance uses on the child poses (/khala, /tassadar), so the landing
// hero shows this live counter pill where the child routes show "← OpenAgents".
// It reuses the SAME wrap/pill styling (dark glass, khala-blue, Commit Mono,
// fixed top-left, reduced-motion safe) and reads the SAME live tokens-served
// model that powers the hero counter — no parallel data source. It links to the
// full public stats page. The digits carry `tabular-nums` so they don't jiggle
// as the live total ticks up.
export const khalaTokensServedPill = (
  model: PublicKhalaTokensServedModel,
): Html => {
  const h = html<Message>()
  return h.div(
    [Ui.className<Message>(backButtonWrapClass)],
    [
      h.a(
        [
          h.Href(statsRouter()),
          h.AriaLabel('Tokens served — open stats'),
          h.DataAttribute('landing-khala-tokens-pill', 'home'),
          Ui.className<Message>(khalaTokensServedPillClass),
        ],
        [
          // Compact label on phones; the full label takes over at `sm`+. The full
          // label stays in the DOM (visually hidden on mobile) so the accessible
          // name and the rendered text content remain "Tokens Served:".
          h.span([Ui.className<Message>('sm:hidden')], ['Tokens:']),
          h.span([Ui.className<Message>('hidden sm:inline')], [
            'Tokens Served:',
          ]),
          h.span(
            [Ui.className<Message>('tabular-nums text-white')],
            [formatKhalaTokensServed(model)],
          ),
        ],
      ),
    ],
  )
}
