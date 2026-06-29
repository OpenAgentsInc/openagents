import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { landingSquaresView } from '../../../scene/landingSquaresElement'
import * as Ui from '../../../ui'
import type { Message } from '../message'

// Standalone `/landing` surface: a full near-black screen whose backdrop is a
// dense grid of small blue-glowing (HDR bloom) squares, with "OpenAgents" set
// large and white in the centre. No nav, no chrome — the canvas fills the
// viewport and the centred wordmark sits above it, pointer-inert.

export const view = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'landing'),
      Ui.className<Message>(
        'relative h-screen h-dvh min-h-screen min-h-dvh w-full overflow-hidden bg-black',
      ),
    ],
    [
      landingSquaresView<Message>([Ui.className<Message>('block')]),
      h.div(
        [
          h.DataAttribute('landing-wordmark', 'openagents'),
          Ui.className<Message>(
            'pointer-events-none absolute inset-0 z-10 flex items-center justify-center',
          ),
        ],
        [
          h.div(
            [
              Ui.className<Message>(
                'select-none text-center font-semibold tracking-tight text-white text-5xl sm:text-7xl lg:text-8xl',
              ),
            ],
            ['OpenAgents'],
          ),
        ],
      ),
    ],
  )
}
