import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { authBootstrapFromSession } from './domain/session'
import { Flags, init } from './main'
import * as AutopilotOnboarding from './page/autopilot-onboarding'
import {
  AutopilotRoute,
  AutopilotVerticalRoute,
  AutopilotWorkRoute,
  urlToAppRoute,
} from './route'

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
  test('parses /autopilot and /autopilot/<vertical>, leaving /autopilot/work for the cockpit', () => {
    expect(urlToAppRoute(appUrl('/autopilot'))).toEqual(AutopilotRoute())
    expect(urlToAppRoute(appUrl('/autopilot/legal'))).toEqual(
      AutopilotVerticalRoute({ vertical: 'legal' }),
    )
    // The cockpit sub-route still wins over the optional vertical segment.
    expect(urlToAppRoute(appUrl('/autopilot/work'))).toEqual(
      AutopilotWorkRoute(),
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

  test('renders the placeholder heading and intro, adapting the intro to the vertical', () => {
    const bare = renderHtml(
      AutopilotOnboarding.view({ _tag: 'LoggedOut' }, Option.none()),
    )

    expect(bare).toContain('Put an AI workforce to work')
    expect(bare).toContain('Tell Autopilot what you want done.')
    expect(bare).toContain('data-autopilot-onboarding-shell')

    const legal = renderHtml(
      AutopilotOnboarding.view({ _tag: 'LoggedOut' }, Option.some('legal')),
    )

    expect(legal).toContain('Put an AI workforce to work')
    expect(legal).toContain('for your legal work')
  })
})
