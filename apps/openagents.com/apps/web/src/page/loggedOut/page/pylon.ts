import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { downloadRouter } from '../../../route'
import { pylonBezierNetworkView } from '../../../scene/pylonBezierNetworkElement'
import { pylonView } from '../../../scene/pylonElement'
import { pylonLaunchGateView } from '../../../scene/pylonLaunchGateElement'
import { pylonStatsView } from '../../../scene/pylonStatsElement'
import * as Ui from '../../../ui'
import type { Message } from '../message'

const PYLON_INSTALL_COMMAND = 'npx @openagentsinc/pylon'

// A server-rendered Pylon install block overlaid on the pylon scene. The scene
// layers are pointer-events-none, so this card sits above them with
// pointer-events-auto and stays selectable without disrupting the viz. The copy
// frames the expectation that you hand this to your coding agent to run.
const pylonInstallCta = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'pointer-events-none absolute inset-x-0 top-4 z-20 flex justify-center px-3',
      ),
    ],
    [
      h.div(
        [
          h.DataAttribute('cta', 'install-pylon'),
          Ui.className<Message>(
            'pointer-events-auto inline-flex max-w-md flex-col items-center gap-1.5 border border-[#d6f6ff] bg-[rgba(1,1,2,0.86)] px-4 py-3 text-center font-mono text-[#f1efe8] shadow-[0_0_28px_rgba(41,121,255,0.22)]',
          ),
        ],
        [
          h.span(
            [
              Ui.className<Message>(
                'text-[0.75rem] font-bold uppercase leading-none tracking-[0.08em]',
              ),
            ],
            ['Run a Pylon node'],
          ),
          h.span(
            [
              Ui.className<Message>(
                'text-[0.6rem] uppercase leading-none tracking-[0.08em] text-white/55',
              ),
            ],
            ['Paste this to your coding agent'],
          ),
          h.pre(
            [
              h.DataAttribute('cta', 'install-pylon-command'),
              Ui.className<Message>(
                'mt-0.5 w-full select-all overflow-x-auto border border-white/15 bg-[rgba(12,15,19,0.94)] px-3 py-2 text-left text-[0.8rem] leading-none text-[#d6f6ff]',
              ),
            ],
            [h.code([], [PYLON_INSTALL_COMMAND])],
          ),
          // AO-5 (#5446): a single discoverable link to the Mac app download
          // page. The homepage stays Pylon-CLI-first; this is a link only, not
          // a marketing-copy rewrite.
          h.a(
            [
              h.Href(downloadRouter()),
              h.DataAttribute('cta', 'download-autopilot-link'),
              Ui.className<Message>(
                'text-[0.6rem] uppercase leading-none tracking-[0.08em] text-white/55 underline underline-offset-2 hover:text-white',
              ),
            ],
            ['Or download the Mac app'],
          ),
        ],
      ),
    ],
  )
}

export const view = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'pylon'),
      Ui.className<Message>(
        'relative h-screen h-dvh min-h-screen min-h-dvh w-full overflow-hidden bg-[#0c0f13]',
      ),
    ],
    [
      pylonInstallCta(),
      pylonView<Message>([
        Ui.className<Message>('absolute inset-0 block h-full w-full'),
      ]),
      // #5050: the bezier network graph — online pylons on a ring with bezier
      // curves flowing into the central pylon, lit by live activity (z-5, over
      // the pylon, under the stats/countdown).
      pylonBezierNetworkView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-[5]'),
      ]),
      // #5050: live network stats. When the countdown is removed at launch, the
      // bezier graph + stats + activity-lit pylon stay as the homepage.
      pylonStatsView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-[6]'),
      ]),
      pylonLaunchGateView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-10'),
      ]),
    ],
  )
}
