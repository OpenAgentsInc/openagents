import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { landingSquaresView } from '../../../scene/landingSquaresElement'
import * as Ui from '../../../ui'
import type { Message } from '../message'

// Standalone `/landing` surface: a full black screen whose only content is a
// subtle, ambient three-effect-style WebGL canvas of a few drifting white
// squares. No nav, no chrome — just the calm background pattern filling the
// viewport.

export const view = (): Html => {
  const h = html<Message>()

  return h.div(
    [
      h.DataAttribute('route', 'landing'),
      Ui.className<Message>(
        'relative h-screen h-dvh min-h-screen min-h-dvh w-full overflow-hidden bg-black',
      ),
    ],
    [landingSquaresView<Message>([Ui.className<Message>('block')])],
  )
}
