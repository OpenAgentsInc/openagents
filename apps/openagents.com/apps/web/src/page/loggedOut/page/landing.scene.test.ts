import { Scene } from 'foldkit'
import { describe, test } from 'vitest'

import { LoggedOut } from '../../../model'
import { HomeRoute } from '../../../route'
import { update } from '../../../update'
import { view } from '../../../view'

// The standalone homepage surface: a near-black page whose backdrop is the
// blue-glowing squares canvas (`oa-landing-squares`) with "OpenAgents" set
// large and centred above it. We assert the wordmark and the canvas-host
// element render; the bloom/glow itself is verified by the committed headless
// screenshot proof (docs/landing-page-screenshot.png).

describe('landing scene', () => {
  test('renders the centred OpenAgents wordmark', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.text('OpenAgents')).toExist(),
      Scene.expect(
        Scene.selector('[data-landing-wordmark="openagents"]'),
      ).toExist(),
    )
  })

  test('mounts the blue-glow squares canvas element', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(HomeRoute())),
      Scene.expect(Scene.selector('oa-landing-squares')).toExist(),
      Scene.expect(Scene.selector('[data-route="landing"]')).toExist(),
    )
  })
})
