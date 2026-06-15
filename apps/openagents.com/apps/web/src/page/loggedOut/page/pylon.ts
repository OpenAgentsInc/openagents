import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { pylonCountdownView } from '../../../scene/pylonCountdownElement'
import { pylonView } from '../../../scene/pylonElement'
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
      pylonCountdownView<Message>([
        Ui.className<Message>(
          'pointer-events-none absolute inset-0 z-10 block',
        ),
      ]),
    ],
  )
}
