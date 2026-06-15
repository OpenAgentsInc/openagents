import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { pylonCountdownView } from '../../../scene/pylonCountdownElement'
import { pylonView } from '../../../scene/pylonElement'
import { pylonStatsView } from '../../../scene/pylonStatsElement'
import * as Ui from '../../../ui'
import type { Message } from '../message'

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
      pylonView<Message>([
        Ui.className<Message>('absolute inset-0 block h-full w-full'),
      ]),
      // #5050: live network stats behind the countdown (z-0). When the countdown
      // is removed at launch, this overlay + the activity-lit pylon stay as the
      // homepage.
      pylonStatsView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-0'),
      ]),
      pylonCountdownView<Message>([
        Ui.className<Message>('pointer-events-none absolute inset-0 z-10'),
      ]),
    ],
  )
}
