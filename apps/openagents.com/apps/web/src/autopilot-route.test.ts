import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from './domain/session'
import { Flags, init } from './main'
import { initFlowModel } from './page/autopilot-onboarding/flow'
import * as AutopilotOnboardingPage from './page/autopilot-onboarding/page'
import {
  AutopilotRoute,
  AutopilotVerticalRoute,
  AutopilotWorkRoute,
  NotFoundRoute,
  urlToAppRoute,
} from './route'

// Stub action constructors for the page view (the test only inspects markup).
const onboardingActions = {
  updatedComposer: (value: string) => ({ _tag: 'stub-composer', value }),
  submittedTurn: () => ({ _tag: 'stub-submit' }),
  clickedCreditKickoff: () => ({ _tag: 'stub-kickoff' }),
}

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

// A logged-in user whose team is the core team can see the cockpit; a
// logged-in user with no such team has no workspace and sees onboarding.
const authWithWorkspace = {
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

const authWithoutWorkspace = authBootstrapFromSession({
  email: 'visitor@example.com',
  name: 'Visitor',
  userId: 'github:visitor',
})

type VNodeLike = Readonly<{
  sel?: string
  text?: string
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    props?: Record<string, unknown>
    class?: Record<string, boolean>
  }
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

describe('autopilot onboarding route', () => {
  test('parses /autopilot and /autopilot/legal, leaving /autopilot/work for the cockpit', () => {
    expect(urlToAppRoute(appUrl('/autopilot'))).toEqual(AutopilotRoute())
    expect(urlToAppRoute(appUrl('/autopilot/legal'))).toEqual(
      AutopilotVerticalRoute({ vertical: 'legal' }),
    )
    // The cockpit sub-route still wins over the optional vertical segment.
    expect(urlToAppRoute(appUrl('/autopilot/work'))).toEqual(
      AutopilotWorkRoute(),
    )
  })

  test('does not claim arbitrary or deeper autopilot verticals as live onboarding pages', () => {
    expect(urlToAppRoute(appUrl('/autopilot/foo'))).toEqual(
      NotFoundRoute({ path: '/autopilot/foo' }),
    )
    expect(urlToAppRoute(appUrl('/autopilot/legal/foo'))).toEqual(
      NotFoundRoute({ path: '/autopilot/legal/foo' }),
    )
  })

  test('serves the onboarding SPA shell to logged-out users (no 302) for both paths', () => {
    const [bareModel] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/autopilot'),
    )

    expect(bareModel).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Autopilot' },
    })

    const [verticalModel] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/autopilot/legal'),
    )

    expect(verticalModel).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'AutopilotVertical', vertical: 'legal' },
    })
  })

  test('serves onboarding to a logged-in user without a workspace', () => {
    const [model] = init(
      Flags.make({ maybeAuth: Option.some(authWithoutWorkspace) }),
      appUrl('/autopilot'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Autopilot' },
    })
  })

  test('resolves a logged-in user with a workspace to the existing cockpit', () => {
    const [model] = init(
      Flags.make({ maybeAuth: Option.some(authWithWorkspace) }),
      appUrl('/autopilot'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedIn',
      route: { _tag: 'Chat' },
    })
  })

  test('renders the onboarding HUD heading, intro, composer, and intake register', () => {
    const bare = renderHtml(
      AutopilotOnboardingPage.overlayView(
        initFlowModel(Option.none()),
        onboardingActions,
      ),
    )

    expect(bare).toContain('Put an AI workforce to work')
    expect(bare).toContain('Describe what you want done.')
    expect(bare).toContain(`data-${AutopilotOnboardingPage.HUD_ROOT_ATTR}`)
    // The command composer is present (text now, voice deferred).
    expect(bare).toContain(`data-${AutopilotOnboardingPage.HUD_COMPOSER_ATTR}`)
    // The intake_progress register surfaces from the first render (a complete,
    // readable surface, not a class-gated blank).
    expect(bare).toContain('Onboarding progress')
    // No credit_kickoff before the flow is quote-ready.
    expect(bare).not.toContain('Kick off the work')
  })

  test('adapts the intro and surfaces a legal consent gate for /autopilot/legal', () => {
    const legal = renderHtml(
      AutopilotOnboardingPage.overlayView(
        initFlowModel(Option.some('legal')),
        onboardingActions,
      ),
    )

    expect(legal).toContain('Put an AI workforce to work')
    expect(legal).toContain('legal work')
    // The legal vertical surfaces a consent gate (content owned by #6130; this
    // only asserts the page accepts and threads the vertical).
    expect(legal).toContain('Data practices')
  })
})
