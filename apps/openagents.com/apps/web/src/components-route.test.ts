import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from './domain/session'
import { Flags, init } from './main'
import { LoggedIn, LoggedOut } from './model'
import { ComponentsFamilyRoute, ComponentsRoute } from './route'
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

  test('parses /components/<family> to the family route', () => {
    const [model] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/components/data-display'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'ComponentsFamily', family: 'data-display' },
    })
  })

  test('renders the selected family on /components/<family>', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(ComponentsFamilyRoute({ family: 'data-display' })),
      ),
      Scene.expect(
        Scene.role('heading', { name: 'Component library' }),
      ).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Data display' })).toExist(),
      // Leads with live, rendered components (the export-name caption of a
      // real previewBox instance), with the contract metadata below.
      Scene.expect(Scene.text('tableList')).toExist(),
      Scene.expect(Scene.text('Contract')).toExist(),
    )
  })

  test('renders the training grammar gallery on /components/training', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ComponentsFamilyRoute({ family: 'training' }))),
      Scene.expect(
        Scene.role('heading', { name: 'Training grammar' }),
      ).toExist(),
      Scene.expect(Scene.text('Run field')).toExist(),
      Scene.expect(Scene.text('Contributor node')).toExist(),
      Scene.expect(Scene.text('Replay pair')).toExist(),
      Scene.expect(Scene.text('Verification gate')).toExist(),
      Scene.expect(Scene.text('Receipt burst')).toExist(),
      Scene.expect(Scene.text('Proof drawer')).toExist(),
      Scene.expect(
        Scene.text('oa-training-run / @openagentsinc/three-effect'),
      ).toExist(),
      Scene.expect(Scene.text('oa-training-grammar-replay-pair')).toExist(),
    )
  })

  test('renders live component previews for representative families', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ComponentsRoute())),
      // Each of these is the export-name label of a real previewBox rendering
      // a live instance from @openagentsinc/ui, proving the gallery shows
      // rendered components and not just metadata prose.
      Scene.expect(Scene.text('inputGroup')).toExist(),
      Scene.expect(Scene.text('tableList')).toExist(),
      Scene.expect(Scene.text('marketingHero')).toExist(),
      Scene.expect(Scene.text('businessIntakeForm')).toExist(),
      Scene.expect(Scene.text('workroomTimeline')).toExist(),
      Scene.expect(Scene.text('v4Composer')).toExist(),
      Scene.expect(Scene.text('applicationHomeScreen')).toExist(),
      Scene.expect(Scene.text('AiElements.promptInput')).toExist(),
    )
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
      Scene.expect(Scene.role('heading', { name: 'Public theme' })).toExist(),
      Scene.expect(
        Scene.role('heading', { name: 'Business landing' }),
      ).toExist(),
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

  test('renders the business landing component family', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ComponentsFamilyRoute({ family: 'business' }))),
      Scene.expect(
        Scene.role('heading', { name: 'Business landing' }),
      ).toExist(),
      Scene.expect(Scene.text('businessOfferingMenu mode=light')).toExist(),
      Scene.expect(Scene.text('businessIntakeForm')).toExist(),
      Scene.expect(Scene.text('data-ui-family business/* markers')).toExist(),
    )
  })

  test('renders the public landing theme component family', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(ComponentsFamilyRoute({ family: 'public-theme' })),
      ),
      Scene.expect(Scene.role('heading', { name: 'Public theme' })).toExist(),
      Scene.expect(Scene.text('publicLandingThemeSelector')).toExist(),
      Scene.expect(
        Scene.text('publicLandingThemeShell mode=light'),
      ).toExist(),
      Scene.expect(
        Scene.text('publicLandingThemeShell mode=dark'),
      ).toExist(),
      Scene.expect(Scene.text('Shell-scoped theme')).toExist(),
      Scene.expect(Scene.text('data-public-landing-shell')).toExist(),
    )
  })
})
