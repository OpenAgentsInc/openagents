import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { Flags, init } from './main'
import { LoggedOut } from './model'
import { GymRoute } from './route'
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

describe('/gym route', () => {
  test('parses /gym to the Gym route for unauthenticated visitors', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/gym'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Gym' },
    })
    // #6261: entering /gym seeds the run cards + cursor from the public sync
    // snapshot and cold-reads the live public run-progress projection, so the
    // follow-along renders live runs instead of a permanent empty state.
    expect(commands.map(command => command.name)).toEqual([
      'LoadGymRunProgressSnapshot',
      'LoadPublicGymRunProgress',
    ])
  })

  // Regression for #6258: `/gym` rendered the homepage-lookalike maintenance
  // body for logged-out visitors because the top-level `publicRouteBody`
  // dispatcher had no `Gym` branch, so the registry's `submodel` disposition
  // silently fell through to `maintenanceBody`. This drives the FULL `view`
  // (publicRouteBody + the maintenance fallthrough), so it fails on the old
  // code and passes once `/gym` is wired through the loggedOut Submodel.
  test('renders the gym page (not the maintenance/home body) for logged-out visitors', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(GymRoute())),
      // Gym-page markers: the page heading and the run-progress accessible
      // mirror added by the live-run-progress surface.
      Scene.expect(Scene.role('heading', { name: 'OpenAgents Gym' })).toExist(),
      Scene.expect(
        Scene.selector('[data-gym-page]'),
      ).toExist(),
      Scene.expect(
        Scene.selector('[data-gym-run-progress-accessible-mirror]'),
      ).toExist(),
      // The maintenance/home body would render this copy; the gym page must not.
      Scene.expect(Scene.text('is a cloud coding agent.')).not.toExist(),
      Scene.expect(Scene.text('Live Pylons')).not.toExist(),
    )
  })
})
