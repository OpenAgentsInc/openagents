import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from './domain/session'
import { Flags, init } from './main'
import { LoggedIn, LoggedOut } from './model'
import { ComponentsRoute } from './route'
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

const authWithTeam = {
  ...authBootstrapFromSession({
    email: 'chris@openagents.com',
    name: 'Christopher David',
    userId: 'github:14167547',
  }),
  teams: [
    {
      id: 'team_openagents_core',
      name: 'OpenAgents Core Team',
      slug: 'openagents-core-team',
      role: 'owner',
      members: [],
    },
  ],
}

describe('components gallery route', () => {
  test('parses /components for unauthenticated visitors', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/components'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Components' },
    })
    expect(commands).toHaveLength(0)
  })

  test('parses /components for authenticated visitors', () => {
    const [model] = init(
      Flags.make({ maybeAuth: Option.some(authWithTeam) }),
      appUrl('/components'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Components' },
    })
  })

  test('renders the design-system workbench with every family', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ComponentsRoute())),
      Scene.expect(
        Scene.role('heading', { name: 'Component library' }),
      ).toExist(),
      Scene.expect(Scene.text('Internal - design-system workbench')).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Primitives' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Shared' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Forms' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Layout' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Navigation' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Data display' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Feedback' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Workroom' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Public' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Page examples' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'V4' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'AI Elements' })).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Live samples' })).toExist(),
    )
  })

  test('renders inside the authenticated workroom shell', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedIn.init(ComponentsRoute(), authWithTeam)),
      Scene.expect(
        Scene.role('heading', { name: 'Component library' }),
      ).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Primitives' })).toExist(),
    )
  })
})
