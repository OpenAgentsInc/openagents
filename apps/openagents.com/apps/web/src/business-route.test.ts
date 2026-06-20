import { Option } from 'effect'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import { Flags, init } from './main'
import * as Business from './page/business'
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

  test('renders the signup form, workspace invite copy, pricing copy, and Slack opt-in', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    // Required signup fields, including a first-class phone field.
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

    // Exact pricing framing required by the issue.
    expect(rendered).toContain(
      'Usage is billed as clear token-based credits — buy credits and spend them as you go. No monthly AI subscription, and your credits never expire.',
    )
  })

  test('renders the offering menu with honest availability badges', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain('What we can do')
    // Offering buckets from the intake spec.
    expect(rendered).toContain('Coding & agent work')
    expect(rendered).toContain('Inference / AI on tap')
    expect(rendered).toContain('Autopilot business automation')
    expect(rendered).toContain('Payments rails')

    // Honest now/soon/roadmap framing is present (no overselling).
    expect(rendered).toContain('Available now')
    expect(rendered).toContain('Available soon')
  })

  test('renders the quick-win to Autopilot ladder', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain(
      'Quick win → put your business on Autopilot',
    )
    expect(rendered).toContain('Day 1 — Quick win')
    expect(rendered).toContain('Week 1 — Repeatable lane')
    expect(rendered).toContain('Ongoing — On Autopilot')
  })

  test('renders the hidden referral-code field for referral attribution', () => {
    const rendered = renderHtml(Business.view({ _tag: 'LoggedOut' }))

    expect(rendered).toContain('name="referralCode"')
    expect(rendered).toContain('id="business-referral-code"')
    // The capture script reads ?ref= into the hidden field.
    expect(rendered).toContain("p.get('ref')")
  })
})
