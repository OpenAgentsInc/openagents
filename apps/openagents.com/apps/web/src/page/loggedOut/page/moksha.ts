import { mokshaView } from '@openagentsinc/three-effect/foldkit'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import * as Ui from '../../../ui'
import type { Message } from '../message'

export const view = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'moksha'),
      Ui.className<Message>(
        'h-screen h-dvh min-h-screen min-h-dvh w-full overflow-hidden bg-[#0c0f13]',
      ),
    ],
    [
      mokshaView<Message>([
        Ui.className<Message>('block h-full min-h-full w-full'),
      ]),
    ],
  )
}
