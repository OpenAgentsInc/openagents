import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from './main'
import * as Business from './page/business'
import * as BusinessKpi from './page/businessKpi'
import { BusinessRoute, urlToAppRoute } from './route'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
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

describe('business route', () => {
  test('parses the public /business path', () => {
    expect(urlToAppRoute(appUrl('/business'))).toEqual(BusinessRoute())
  })

  test('parses the customer KPI dashboard path', () => {
    const route = urlToAppRoute(
      appUrl('/business/kpi/engagement.public.vertical_pipeline_1'),
    )

    expect(route).toMatchObject({
      _tag: 'BusinessKpi',
      engagementRef: 'engagement.public.vertical_pipeline_1',
    })
  })

  test('keeps unauthenticated users on the business landing page', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/business'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Business' },
    })
    // Public route: no auth bootstrap command is dispatched.
    expect(commands).toHaveLength(0)
  })

  test('keeps unauthenticated users on the customer KPI dashboard page', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/business/kpi/engagement.public.vertical_pipeline_1'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: {
        _tag: 'BusinessKpi',
        engagementRef: 'engagement.public.vertical_pipeline_1',
      },
    })
    expect(commands).toHaveLength(0)
  })

  test('renders the signup form, workspace invite copy, pricing copy, and Slack opt-in', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    // Required signup fields, including a first-class phone field.
    expect(rendered).toContain('action="/api/public/business-signup"')
    expect(rendered).toContain('data-ui-family="business/intake-forms"')
    expect(rendered).toContain('data-ui-family="forms/input-groups"')
    expect(rendered).toContain('name="businessName"')
    expect(rendered).toContain('name="contactEmail"')
    expect(rendered).toContain('type="email"')
    expect(rendered).toContain('name="website"')
    expect(rendered).toContain('name="phone"')
    expect(rendered).toContain('type="tel"')
    expect(rendered).toContain('name="helpWith"')

    // Opt-in shared Slack channel (UI only).
    expect(rendered).toContain('name="requestSlackChannel"')
    expect(rendered).toContain('Request a shared Slack channel')

    // Prefilled workspace invite copy.
    expect(rendered).toContain('Project invite')
    expect(rendered).toContain(
      'Your invite opens a named project with seeded notes, starter workflows, and an intro receipt.',
    )

    // Pricing framing stays scoped and operator-assisted.
    expect(rendered).toContain(
      'Packages start with a fixed scope and receipt plan before funding.',
    )
    expect(rendered).toContain(
      'Delivery is operator-assisted today; checkout and self-serve hosting are not implied by the rate card.',
    )
  })

  test('renders the Khala intake console with honest empty state and no-JS fallback', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain('Agents that work.')
    expect(rendered).toContain('data-business-intake-chat=""')
    expect(rendered).toContain('KHALA · INTAKE')
    expect(rendered).toContain('data-intake-chat-transcript=""')
    expect(rendered).toContain('data-intake-chat-input=""')
    expect(rendered).toContain('data-intake-chat-send=""')
    expect(rendered).toContain('data-intake-chat-empty=""')
    // Server calls are user-initiated only; the empty state is static copy.
    expect(rendered).toContain('drafts your intake spec')
    // No-JS visitors are pointed at the form, which stays the submit authority.
    expect(rendered).toContain(
      'JavaScript is off — use the form below instead.',
    )
    expect(rendered).toContain(
      'bounded interview · no credentials · receipt-first',
    )
  })

  test('renders the offering menu with honest availability badges', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain('data-ui-family="business/offering-menus"')
    expect(rendered).toContain('data-ui-family="business/offering-cards"')
    expect(rendered).toContain('data-ui-family="business/availability-badges"')
    expect(rendered).toContain('What we can do')
    // Offering buckets from the intake spec.
    expect(rendered).toContain('Coding & agent work')
    expect(rendered).toContain('Inference / AI on tap')
    expect(rendered).toContain('Autopilot business automation')
    expect(rendered).toContain('Payments rails')

    // Honest available-now/operator-assisted framing is present (no overselling).
    expect(rendered).toContain('Available now')
    expect(rendered).toContain('Operator-assisted')
    expect(rendered).toContain('Live now:')
    expect(rendered).toContain('Current caveat:')
    expect(rendered).toContain(
      'The full paid card/Bitcoin-to-credit-to-inference loop is not collectable end-to-end in production yet.',
    )
    expect(rendered).toContain(
      'Packaging this as a priced intake-to-receipt business product is operator-assisted today.',
    )
  })

  test('renders the quick-win to Autopilot ladder', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain('Quick win -> put your business on Autopilot')
    expect(rendered).toContain('data-ui-family="business/quick-win-ladders"')
    expect(rendered).toContain('data-business-ladder-step="Day 1"')
    expect(rendered).toContain('Day 1 - Quick win')
    expect(rendered).toContain('Week 1 - Repeatable lane')
    expect(rendered).toContain('Ongoing - On Autopilot')
  })

  test('renders the public rate card with fixed scopes and receipt plans', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain('data-ui-family="business/rate-cards"')
    expect(rendered).toContain('data-ui-family="business/rate-card-packages"')
    expect(rendered).toContain('Rate card')
    expect(rendered).toContain('Quick Win')
    expect(rendered).toContain('$1,000-$5,000 fixed')
    expect(rendered).toContain('Fleet Sprint')
    expect(rendered).toContain('$5,000-$15,000 / week')
    expect(rendered).toContain('On Autopilot Retainer')
    expect(rendered).toContain('$2,000-$10,000 / month')
    expect(rendered).toContain('QA Swarm')
    expect(rendered).toContain(
      '$1,000-$5,000 audit; $5,000-$15,000 sprint; $2,000-$10,000 / month',
    )
    expect(rendered).toContain('Receipt plan')
    expect(rendered).toContain(
      'The rate card is a quote starter, not a self-serve checkout',
    )
    expect(rendered).toContain('qa_swarm.service_packages.v1')
  })

  test('renders the dark-only operational landing shell', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))
    const shell = renderHtml(Business.businessLandingShell())

    // Dark-only per DESIGN.md — the page never offers a light variant or a
    // theme selector; the operational black surface is the brand.
    expect(rendered).toContain('data-public-landing-shell=""')
    expect(rendered).toContain('data-business-landing-shell=""')
    expect(rendered).toContain('data-public-landing-theme="dark"')
    expect(rendered).toContain('data-public-landing-theme-preference="dark"')
    expect(rendered).not.toContain('data-public-landing-theme-select=""')
    expect(rendered).not.toContain('data-public-landing-theme="light"')

    expect(shell).toContain('data-public-landing-theme="dark"')
    expect(shell).toContain('data-ui-family="business/landing-heroes"')
    expect(shell).toContain('data-ui-family="business/project-invites"')
  })

  test('renders the hidden referral-code field for referral attribution', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain('name="referralCode"')
    expect(rendered).toContain('id="business-referral-code"')
    expect(rendered).toContain('name="sourceRef"')
    expect(rendered).toContain('id="business-source-ref"')
    // The capture script reads ?ref= into the hidden field.
    expect(rendered).toContain("p.get('ref')")
    expect(rendered).toContain("p.get('sourceRef')")
  })

  test('renders the KPI scorekeeper with baseline, live metrics, evidence, and privacy boundaries', () => {
    const route = urlToAppRoute(
      appUrl('/business/kpi/engagement.public.vertical_pipeline_1'),
    )
    if (route._tag !== 'BusinessKpi') {
      throw new Error('expected BusinessKpi route')
    }

    const rendered = renderHtml(
      BusinessKpi.view(route, { _tag: 'LoggedOut' }),
    )

    expect(rendered).toContain(
      'data-business-kpi-dashboard="engagement.public.vertical_pipeline_1"',
    )
    expect(rendered).toContain('Scorekeeper')
    expect(rendered).toContain('Baseline snapshot and live engagement metrics')
    expect(rendered).toContain('data-business-kpi-metric="lead_volume"')
    expect(rendered).toContain('data-business-kpi-metric="conversion"')
    expect(rendered).toContain('data-business-kpi-metric="aov"')
    expect(rendered).toContain('data-business-kpi-metric="revenue"')
    expect(rendered).toContain('data-business-kpi-metric="consult_attach"')
    expect(rendered).toContain('Baseline')
    expect(rendered).toContain('Current')
    expect(rendered).toContain('Delta')
    expect(rendered).toContain('/api/public/business/funnel-dashboard')
    expect(rendered).toContain('table:business_funnel_events')
    expect(rendered).toContain('issue:8105')
    expect(rendered).toContain('roadmap:BF-7.1')
    expect(rendered).toContain('Excluded: client name, contact email, phone')
    expect(rendered).toContain(
      'settlement and payout claims remain out of scope',
    )
    expect(rendered).not.toContain('customer@example.com')
    expect(rendered).not.toContain('555-')
  })
})
