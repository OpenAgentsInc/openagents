import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { AUTOPILOT_DESKTOP_MACOS_ARM64_DMG_URL } from '../../../constant'
import { pylonBezierNetworkView } from '../../../scene/pylonBezierNetworkElement'
import { pylonView } from '../../../scene/pylonElement'
import { pylonLaunchGateView } from '../../../scene/pylonLaunchGateElement'
import { pylonStatsView } from '../../../scene/pylonStatsElement'
import * as Ui from '../../../ui'
import type { Message } from '../message'

// #5059: a real server-rendered Download Autopilot link overlaid on the pylon
// scene. The scene layers are pointer-events-none, so this anchor sits above
// them with pointer-events-auto and stays clickable without disrupting the viz.
const downloadAutopilotCta = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'pointer-events-none absolute inset-x-0 top-4 z-20 flex justify-center px-3',
      ),
    ],
    [
      h.a(
        [
          h.Href(AUTOPILOT_DESKTOP_MACOS_ARM64_DMG_URL),
          h.DataAttribute('cta', 'download-autopilot'),
          Ui.className<Message>(
            'pointer-events-auto inline-flex flex-col items-center gap-0.5 border border-[#d6f6ff] bg-[rgba(1,1,2,0.86)] px-4 py-2 text-center font-mono text-[#f1efe8] shadow-[0_0_28px_rgba(41,121,255,0.22)] hover:border-white hover:bg-[rgba(12,15,19,0.94)]',
          ),
        ],
        [
          h.span(
            [
              Ui.className<Message>(
                'text-[0.75rem] font-bold uppercase leading-none tracking-[0.08em]',
              ),
            ],
            ['Download Autopilot'],
          ),
          h.span(
            [
              Ui.className<Message>(
                'text-[0.6rem] uppercase leading-none tracking-[0.08em] text-white/55',
              ),
            ],
            ['macOS arm64 DMG'],
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
      downloadAutopilotCta(),
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
