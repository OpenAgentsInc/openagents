// Public live Tassadar run page (#5118, epic #5112).
//
// `/run` renders the live Tassadar executor run as a data-bound 3D view. The
// page is a thin shell around the self-fetching `oa-tassadar-run` scene
// element. That element owns the actual
// fetch → adapter → `trainingRunView` pipeline (loading / idle-empty / error
// states), so the page stays declarative. Dark-only DESIGN.
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { tassadarRunView } from '../scene/tassadarRunElement'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'

const pageShellClass = 'relative h-dvh overflow-hidden bg-[#000] text-[#f1efe8]'

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
): Html => {
  const h = html<Message>()
  void authState

  return h.div(
    [
      Ui.className<Message>(pageShellClass),
      h.DataAttribute('route', 'tassadar'),
    ],
    [
      h.main(
        [
          h.AriaLabel('Live Tassadar run'),
          Ui.className<Message>('absolute inset-0 overflow-hidden bg-black'),
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
