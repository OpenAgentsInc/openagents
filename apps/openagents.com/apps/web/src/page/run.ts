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
import {
  tassadarProofReplayView,
  TASSADAR_REPLAY_SLUG_DATA_KEY,
} from '../scene/tassadarProofReplayElement'
import {
  TASSADAR_SPACETIME_DATABASE_DATA_KEY,
  TASSADAR_SPACETIME_WORLD_URL_DATA_KEY,
} from '../scene/tassadarSpacetimeWorld'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'

const pageShellClass = 'relative h-dvh overflow-hidden bg-[#000] text-[#f1efe8]'
const TASSADAR_SPACETIME_WORLD_URL = 'https://spacetime.openagents.com'
const TASSADAR_SPACETIME_DATABASE = 'openagents-world'

export const view = <Message>(
  authState: PublicHeaderAuthState<Message>,
  replaySlug?: string,
): Html => {
  const h = html<Message>()
  void authState
  const isReplay = replaySlug !== undefined && replaySlug.trim() !== ''

  return h.div(
    [
      Ui.className<Message>(pageShellClass),
      h.DataAttribute('route', isReplay ? 'tassadar-replay' : 'tassadar'),
    ],
    [
      h.main(
        [
          h.AriaLabel(isReplay ? 'Tassadar proof replay' : 'Live Tassadar run'),
          Ui.className<Message>('absolute inset-0 overflow-hidden bg-black'),
        ],
        [
          isReplay
            ? tassadarProofReplayView<Message>([
                Ui.className<Message>(
                  'absolute inset-0 block h-full min-h-full w-full',
                ),
                h.DataAttribute(TASSADAR_REPLAY_SLUG_DATA_KEY, replaySlug),
              ])
            : tassadarRunView<Message>([
                Ui.className<Message>(
                  'absolute inset-0 block h-full min-h-full w-full',
                ),
                h.DataAttribute(
                  TASSADAR_SPACETIME_WORLD_URL_DATA_KEY,
                  TASSADAR_SPACETIME_WORLD_URL,
                ),
                h.DataAttribute(
                  TASSADAR_SPACETIME_DATABASE_DATA_KEY,
                  TASSADAR_SPACETIME_DATABASE,
                ),
              ]),
        ],
      ),
    ],
  )
}
