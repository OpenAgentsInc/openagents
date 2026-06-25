import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import { ClickedExitKhala } from '../message'
import type { Message } from '../message'

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
