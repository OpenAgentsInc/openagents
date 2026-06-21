// Public Tassadar route.
//
// The old web Tassadar scene is intentionally retired. The live 3D
// world is Autopilot Desktop Verse; this web route stays only as a compact
// pointer to the public summary data and the Desktop surface. Proof replay
// routes remain live below.
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import {
  tassadarProofReplayView,
  TASSADAR_REPLAY_SLUG_DATA_KEY,
} from '../scene/tassadarProofReplayElement'
import * as Ui from '../ui'
import type { PublicHeaderAuthState } from './publicHeader'

const pageShellClass = 'relative h-dvh overflow-hidden bg-[#000] text-[#f1efe8]'

const retiredTassadarSceneView = <Message>(): Html => {
  const h = html<Message>()

  return h.section(
    [
      h.AriaLabel('Retired Tassadar web scene'),
      h.DataAttribute('tassadar-scene', 'retired'),
      Ui.className<Message>(
        'absolute inset-0 grid place-items-center bg-black p-5 text-[#f1efe8]',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'grid w-full max-w-xl gap-4 border border-white/10 bg-[#010102] p-5 font-mono shadow-2xl shadow-black/60',
          ),
        ],
        [
          h.p(
            [
              Ui.className<Message>(
                'm-0 text-[0.64rem] font-semibold uppercase leading-none tracking-[0.16em] text-white/35',
              ),
            ],
            ['Retired scene'],
          ),
          h.h1(
            [
              Ui.className<Message>(
                'm-0 text-xl font-semibold leading-tight text-white/90 sm:text-2xl',
              ),
            ],
            ['Tassadar lives in the Verse'],
          ),
          h.p(
            [
              Ui.className<Message>(
                'm-0 text-sm leading-6 text-white/58',
              ),
            ],
            [
              'The old web training-run scene is deprecated. Use Autopilot Desktop Verse for the in-world Pylon and Tassadar surface.',
            ],
          ),
          h.div(
            [
              Ui.className<Message>(
                'flex flex-wrap gap-2 pt-1 text-xs font-semibold',
              ),
            ],
            [
              h.a(
                [
                  h.Href('/api/public/tassadar-run-summary'),
                  Ui.className<Message>(
                    'border border-white/15 px-3 py-2 text-white/75 underline-offset-4 hover:border-white/30 hover:text-white hover:underline',
                  ),
                ],
                ['Public summary API'],
              ),
              h.a(
                [
                  h.Href('/tassadar/replay/first-real-settlement'),
                  Ui.className<Message>(
                    'border border-white/15 px-3 py-2 text-white/75 underline-offset-4 hover:border-white/30 hover:text-white hover:underline',
                  ),
                ],
                ['Proof replay'],
              ),
            ],
          ),
        ],
      ),
    ],
  )
}

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
            : retiredTassadarSceneView<Message>(),
        ],
      ),
    ],
  )
}
