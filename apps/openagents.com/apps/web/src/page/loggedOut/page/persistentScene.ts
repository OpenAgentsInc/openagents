import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { landingSquaresView } from '../../../scene/landingSquaresElement'
import * as Ui from '../../../ui'
import { ClickedEnterKhala } from '../message'
import type { Message } from '../message'
import { view as khalaView } from './khala'

export const PERSISTENT_SCENE_KEY = 'persistent-landing-scene'
export const PERSISTENT_SCENE_SHELL_KEY = 'persistent-scene-shell'
export const PERSISTENT_SCENE_OVERLAY_PREFIX = 'persistent-scene-overlay:'

export type PersistentSceneRoute = 'Landing' | 'Khala'

const persistentCanvasLayer = (
  h: ReturnType<typeof html<Message>>,
  route: PersistentSceneRoute,
): Html =>
  h.keyed('div')(
    PERSISTENT_SCENE_KEY,
    [
      h.DataAttribute('persistent-scene', 'landing-squares'),
      Ui.className<Message>('absolute inset-0 z-0'),
    ],
    [
      landingSquaresView<Message>([
        Ui.className<Message>('block'),
        // Active route -> camera pose. The canvas node is keyed the same across
        // /landing <-> /khala so it persists; only this attribute changes, and
        // the element eases the camera to the new pose (continuous flight).
        h.DataAttribute('pose', route === 'Landing' ? 'landing' : 'khala'),
      ]),
    ],
  )

const landingOverlay = (h: ReturnType<typeof html<Message>>): Html =>
  h.div(
    [
      h.DataAttribute('landing-wordmark', 'openagents'),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-10 flex flex-col items-center justify-center gap-10',
      ),
    ],
    [
      h.div(
        [
          Ui.className<Message>(
            'select-none text-center font-semibold text-white text-5xl sm:text-7xl lg:text-8xl',
          ),
        ],
        ['OpenAgents'],
      ),
      h.button(
        [
          h.Type('button'),
          h.OnClick(ClickedEnterKhala()),
          h.DataAttribute('landing-cta', 'khala'),
          Ui.className<Message>(
            'cursor-pointer ' +
            'pointer-events-auto inline-flex items-center justify-center rounded-full border border-white/20 bg-white/5 px-7 py-3 text-sm font-semibold uppercase tracking-[0.18em] text-white backdrop-blur transition hover:border-white/40 hover:bg-white/10',
          ),
        ],
        ['Enter Khala'],
      ),
    ],
  )

const khalaOverlay = (h: ReturnType<typeof html<Message>>): Html =>
  h.div(
    [
      h.DataAttribute('persistent-scene-overlay', 'khala'),
      Ui.className<Message>('absolute inset-0 z-10 overflow-y-auto'),
    ],
    [khalaView()],
  )

const overlayForRoute = (
  h: ReturnType<typeof html<Message>>,
  route: PersistentSceneRoute,
): Html => (route === 'Landing' ? landingOverlay(h) : khalaOverlay(h))

export const view = (route: PersistentSceneRoute): Html => {
  const h = html<Message>()

  return h.keyed('div')(
    PERSISTENT_SCENE_SHELL_KEY,
    [
      h.DataAttribute('route', route === 'Landing' ? 'landing' : 'khala'),
      h.DataAttribute('persistent-scene-shell', 'landing'),
      Ui.className<Message>(
        'relative h-screen h-dvh min-h-screen min-h-dvh w-full overflow-hidden bg-black',
      ),
    ],
    [
      persistentCanvasLayer(h, route),
      // 75%-black scrim ABOVE the scene, BEHIND the text — on both routes — so
      // the 3D pylon scene stays visible (not hidden by an opaque page bg) while
      // the overlaid copy stays readable. Keyed the same across routes so it
      // persists with the canvas.
      h.keyed('div')(
        'persistent-scene-scrim',
        [
          Ui.className<Message>(
            'pointer-events-none absolute inset-0 z-[5] bg-black/75',
          ),
        ],
        [],
      ),
      h.keyed('div')(
        `${PERSISTENT_SCENE_OVERLAY_PREFIX}${route}`,
        [Ui.className<Message>('absolute inset-0 z-10')],
        [overlayForRoute(h, route)],
      ),
    ],
  )
}
