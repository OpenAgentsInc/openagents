import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { LoggedOut } from './model'
import { NewLandingRoute, urlToAppRoute } from './route'
import { update } from './update'
import { view } from './view'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

describe('/new landing route', () => {
  test('parses to the Launch UI landing route', () => {
    expect(urlToAppRoute(appUrl('/new'))._tag).toBe('NewLanding')
  })

  test('renders the Launch UI replica shell', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(NewLandingRoute())),
      Scene.expect(Scene.selector('[data-route="new-landing"]')).toExist(),
      Scene.expect(
        Scene.selector('[data-launch-ui-replica="blue-minimal"]'),
      ).toExist(),
      Scene.expect(
        Scene.text('Give your big idea the design it deserves'),
      ).toExist(),
      Scene.expect(Scene.text('Launch UI v2 is out!')).toExist(),
    )
  })
})
