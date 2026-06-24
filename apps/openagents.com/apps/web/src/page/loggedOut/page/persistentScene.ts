import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { landingSquaresView } from '../../../scene/landingSquaresElement'
import * as Ui from '../../../ui'
import { ClickedEnterTassadar } from '../message'
import type { Message } from '../message'
import { view as tassadarView } from './tassadar'

export const PERSISTENT_SCENE_KEY = 'persistent-landing-scene'
export const PERSISTENT_SCENE_SHELL_KEY = 'persistent-scene-shell'
export const PERSISTENT_SCENE_OVERLAY_PREFIX = 'persistent-scene-overlay:'

export type PersistentSceneRoute =
  | 'Landing'
  | 'Khala'
  | 'Tassadar'
  | 'Autopilot'

// Active route -> camera pose. The canvas node is keyed the same across
// /landing <-> /khala <-> /tassadar <-> /autopilot so the ONE scene instance
// persists; only this attribute changes, and the element eases the camera to
// the new pose (continuous flight). /autopilot reuses the same keyed canvas
// with its own onboarding `autopilot` vantage — no second scene is created.
export const poseForRoute = (route: PersistentSceneRoute): string =>
  route === 'Khala'
    ? 'khala'
    : route === 'Tassadar'
      ? 'tassadar'
      : route === 'Autopilot'
        ? 'autopilot'
        : 'landing'

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
        h.DataAttribute('pose', poseForRoute(route)),
      ]),
    ],
  )

// Glowing-blue Protoss CTA on dark glass. Reuses the shared khala-* glow
// utilities (see styles.css) so the buttons share the scene's energy: an
// ease-out glow on hover, no bounce, motion-reduced fallback. The two buttons
// sit side by side on wide viewports and stack on narrow ones.
const landingButtonClass =
  'khala-focus group pointer-events-auto inline-flex items-center justify-center gap-2 ' +
  'rounded-full border border-[#3a7bff]/55 bg-[#0b1322]/80 px-7 py-3 font-mono text-sm ' +
  'font-semibold uppercase tracking-[0.18em] text-[#dce8ff] backdrop-blur-md khala-glow ' +
  'transition-all duration-300 ease-out hover:border-[#4fd0ff]/85 hover:text-white ' +
  'hover:khala-glow-strong cursor-pointer motion-reduce:transition-none'

const landingOverlay = (h: ReturnType<typeof html<Message>>): Html =>
  h.div(
    [
      h.DataAttribute('landing-wordmark', 'openagents'),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-10 flex flex-col items-center justify-center gap-10 px-6',
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
      h.div(
        [
          Ui.className<Message>(
            'flex flex-col items-center gap-4 sm:flex-row sm:gap-5',
          ),
        ],
        [
          // "What is Khala?" CTA hidden until Khala is fully live. To restore:
          // re-add `ClickedEnterKhala` to the '../message' import and uncomment:
          //   h.button(
          //     [
          //       h.Type('button'),
          //       h.OnClick(ClickedEnterKhala()),
          //       h.DataAttribute('landing-cta', 'khala'),
          //       Ui.className<Message>(landingButtonClass),
          //     ],
          //     ['What is Khala?'],
          //   ),
          h.button(
            [
              h.Type('button'),
              h.OnClick(ClickedEnterTassadar()),
              h.DataAttribute('landing-cta', 'tassadar'),
              Ui.className<Message>(landingButtonClass),
            ],
            ['Join the Tassadar training run'],
          ),
        ],
      ),
    ],
  )

// The real /khala chat HUD (the chat box + info popup) is rendered by the
// loggedOut view and threaded in here as `khalaOverlay`, so it mounts as the
// overlay of the SAME keyed canvas (no second scene). When none is supplied
// (e.g. a unit test of the scene shell in isolation), a minimal energy label
// stands in, proving the canvas hosts the `Khala` pose.
const khalaPlaceholderOverlay = (h: ReturnType<typeof html<Message>>): Html =>
  h.div(
    [
      h.DataAttribute('persistent-scene-overlay', 'khala'),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-10 flex flex-col items-center justify-center gap-6 px-6',
      ),
    ],
    [
      h.div(
        [
          h.DataAttribute('khala-glow', 'energy'),
          Ui.className<Message>(
            'khala-glow select-none text-center font-semibold text-white text-4xl sm:text-6xl',
          ),
        ],
        ['Khala'],
      ),
    ],
  )

const tassadarOverlay = (
  h: ReturnType<typeof html<Message>>,
  copiedAgentInstructions: boolean,
): Html =>
  h.div(
    [
      h.DataAttribute('persistent-scene-overlay', 'tassadar'),
      Ui.className<Message>('absolute inset-0 z-10 overflow-y-auto'),
    ],
    [tassadarView(copiedAgentInstructions)],
  )

// Onboarding overlay for the shared persistent scene at the `autopilot` pose.
// The real /autopilot onboarding HUD (#6129) is rendered by the loggedOut view
// and threaded in here as `autopilotOverlay`, so it mounts as the overlay of the
// SAME keyed canvas (no second scene instance — design doc §1). When no overlay
// is supplied (e.g. a unit test of the scene shell in isolation), a minimal
// energy label stands in, proving the canvas hosts the `Autopilot` pose.
const autopilotPlaceholderOverlay = (
  h: ReturnType<typeof html<Message>>,
): Html =>
  h.div(
    [
      h.DataAttribute('persistent-scene-overlay', 'autopilot'),
      Ui.className<Message>(
        'pointer-events-none absolute inset-0 z-10 flex flex-col items-center justify-center gap-6 px-6',
      ),
    ],
    [
      h.div(
        [
          h.DataAttribute('autopilot-glow', 'energy'),
          Ui.className<Message>(
            'khala-glow select-none text-center font-semibold text-white text-4xl sm:text-6xl',
          ),
        ],
        ['Autopilot'],
      ),
    ],
  )

const overlayForRoute = (
  h: ReturnType<typeof html<Message>>,
  route: PersistentSceneRoute,
  copiedAgentInstructions: boolean,
  autopilotOverlay: Html | undefined,
  khalaOverlay: Html | undefined,
): Html =>
  route === 'Landing'
    ? landingOverlay(h)
    : route === 'Tassadar'
      ? tassadarOverlay(h, copiedAgentInstructions)
      : route === 'Autopilot'
        ? (autopilotOverlay ?? autopilotPlaceholderOverlay(h))
        : (khalaOverlay ?? khalaPlaceholderOverlay(h))

// `autopilotOverlay` is the real /autopilot onboarding HUD (#6129) and
// `khalaOverlay` is the real /khala chat HUD, both supplied by the loggedOut
// view. Each is only consulted on its own route; other routes ignore it.
// Keeping them parameters (rather than importing the pages here) avoids a
// model/message import cycle through the scene module.
export const view = (
  route: PersistentSceneRoute,
  copiedAgentInstructions = false,
  autopilotOverlay?: Html,
  khalaOverlay?: Html,
): Html => {
  const h = html<Message>()

  return h.keyed('div')(
    PERSISTENT_SCENE_SHELL_KEY,
    [
      h.DataAttribute('route', poseForRoute(route)),
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
        [
          overlayForRoute(
            h,
            route,
            copiedAgentInstructions,
            autopilotOverlay,
            khalaOverlay,
          ),
        ],
      ),
    ],
  )
}
