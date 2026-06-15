import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { liveCopyInstructionsView } from '../../../scene/liveCopyInstructionsElement'
import { pylonBezierNetworkView } from '../../../scene/pylonBezierNetworkElement'
import { pylonView } from '../../../scene/pylonElement'
import { pylonStatsView } from '../../../scene/pylonStatsElement'
import * as Ui from '../../../ui'
import type { Message } from '../message'

export const view = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'live'),
      Ui.className<Message>(
        'relative h-screen h-dvh min-h-screen min-h-dvh w-full overflow-hidden bg-[#0c0f13]',
      ),
    ],
    [
      pylonView<Message>([
        Ui.className<Message>('absolute inset-0 block h-full w-full'),
      ]),
      pylonBezierNetworkView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-[5]'),
      ]),
      pylonStatsView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-[6]'),
      ]),
      liveCopyInstructionsView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-10'),
      ]),
    ],
  )
}
