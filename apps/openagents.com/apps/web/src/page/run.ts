// Public live Tassadar run page (#5118, epic #5112).
//
// `/run` renders the live Tassadar executor run as a data-bound 3D view. The
// page is a thin shell: the public header, a header line naming the run and
// flagging it as live data (with a link to the public endpoint), and the
// self-fetching `oa-tassadar-run` scene element. That element owns the actual
// fetch → adapter → `trainingRunView` pipeline (loading / idle-empty / error
// states), so the page stays declarative. Dark-only DESIGN.

import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  TASSADAR_RUN_SUMMARY_ENDPOINT,
  tassadarRunView,
} from '../scene/tassadarRunElement'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'
import * as PublicHeader from './publicHeader'

const pageShellClass =
  'flex h-dvh flex-col overflow-hidden bg-[#000] text-[#f1efe8]'

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>(pageShellClass)],
    [
      PublicHeader.view(authState),
      h.header(
        [
          Ui.className<Message>(
            'flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 border-b border-[#1a1a1a] px-4 py-3',
          ),
        ],
        [
          h.div(
            [],
            [
              h.span(
                [
                  Ui.className<Message>(
                    'mr-2 font-mono text-[0.7rem] uppercase tracking-[0.08em] text-white/35',
                  ),
                ],
                ['Live data'],
              ),
              h.span(
                [Ui.className<Message>('text-base text-white/85 sm:text-sm')],
                ['Tassadar executor run — public projection, breathing in real time'],
              ),
            ],
          ),
          h.a(
            [
              h.Href(TASSADAR_RUN_SUMMARY_ENDPOINT),
              Ui.className<Message>(
                'font-mono text-[0.7rem] text-white/40 underline-offset-2 transition hover:text-white/70 hover:underline',
              ),
            ],
            [TASSADAR_RUN_SUMMARY_ENDPOINT],
          ),
        ],
      ),
      h.main(
        [
          h.AriaLabel('Live Tassadar run'),
          Ui.className<Message>('relative min-h-0 flex-1 overflow-hidden bg-black'),
        ],
        [
          tassadarRunView<Message>([
            Ui.className<Message>(
              'absolute inset-0 block h-full min-h-full w-full',
            ),
          ]),
        ],
      ),
    ],
  )
}
