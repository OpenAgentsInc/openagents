import { pendingDecision } from '@openagentsinc/autopilot-control-protocol'
import {
  decisionRequestFixture,
  sessionListFixture,
} from '@openagentsinc/autopilot-control-protocol/fixtures'
import { DecisionCard, SessionList } from '@openagentsinc/autopilot-ui'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../ui'

export const view = (): Html => {
  const h = html<never>()
  const decision = pendingDecision(decisionRequestFixture)

  return h.section(
    [
      Ui.className<never>(
        'mx-auto grid w-[min(100%,1120px)] gap-5 px-4 py-6 text-[#f1efe8]',
      ),
    ],
    [
      h.header(
        [
          Ui.className<never>(
            'grid gap-2 border-b border-white/10 pb-4',
          ),
        ],
        [
          h.p(
            [
              Ui.className<never>(
                'm-0 font-mono text-xs uppercase tracking-wide text-white/45',
              ),
            ],
            ['Clients preview'],
          ),
          h.h1(
            [
              Ui.className<never>(
                'm-0 text-2xl font-semibold tracking-normal text-white sm:text-3xl',
              ),
            ],
            ['Autopilot control surface'],
          ),
        ],
      ),
      h.div(
        [
          Ui.className<never>(
            'grid gap-5 lg:grid-cols-[minmax(0,1.15fr)_minmax(20rem,0.85fr)]',
          ),
        ],
        [
          h.section(
            [Ui.className<never>('grid content-start gap-3')],
            [
              h.h2(
                [Ui.className<never>('m-0 text-sm font-semibold text-white')],
                ['Sessions'],
              ),
              SessionList({ sessions: sessionListFixture }),
            ],
          ),
          h.section(
            [Ui.className<never>('grid content-start gap-3')],
            [
              h.h2(
                [Ui.className<never>('m-0 text-sm font-semibold text-white')],
                ['Decision'],
              ),
              DecisionCard({ decision }),
            ],
          ),
        ],
      ),
    ],
  )
}
