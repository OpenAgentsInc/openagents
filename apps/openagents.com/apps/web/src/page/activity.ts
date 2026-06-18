import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { publicActivityTimelineView } from '../scene/publicActivityTimelineElement'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'

const pageShellClass = 'min-h-screen bg-[#08090a] text-[#f4f2ea]'

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  void authState

  return h.div(
    [Ui.className<Message>(pageShellClass), h.DataAttribute('route', 'activity')],
    [
      h.main(
        [
          h.AriaLabel('OpenAgents public activity'),
          Ui.className<Message>('min-h-screen'),
        ],
        [publicActivityTimelineView<Message>([h.DataAttribute('route', 'activity')])],
      ),
    ],
  )
}
