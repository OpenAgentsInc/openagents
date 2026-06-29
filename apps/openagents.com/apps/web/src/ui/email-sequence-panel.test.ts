import type { Html } from 'foldkit/html'
import { describe, expect, it } from 'vitest'

import {
  type EmailSequenceEnrollmentModel,
  type EmailSequenceModel,
  emailSequenceEnrollmentStatus,
  emailSequencePanel,
  isSequenceEnrollable,
} from './email-sequence-panel'

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

const baseSequence: EmailSequenceModel = {
  slug: 'welcome',
  name: 'New customer welcome',
  audience: 'customer',
  status: 'active',
  steps: [
    {
      stepKey: 'step-1',
      name: 'Welcome',
      delayLabel: 'Immediately',
      status: 'active',
      lifecycleKind: 'signup_day_0',
    },
    {
      stepKey: 'step-2',
      name: 'Getting started',
      delayLabel: 'After 2 days',
      status: 'active',
    },
  ],
}

describe('isSequenceEnrollable', () => {
  it('is true only for active sequences', () => {
    expect(isSequenceEnrollable(baseSequence)).toBe(true)
    expect(isSequenceEnrollable({ ...baseSequence, status: 'draft' })).toBe(
      false,
    )
    expect(isSequenceEnrollable({ ...baseSequence, status: 'paused' })).toBe(
      false,
    )
    expect(isSequenceEnrollable({ ...baseSequence, status: 'archived' })).toBe(
      false,
    )
  })
})

describe('emailSequencePanel', () => {
  it('renders the sequence header, status, and steps', () => {
    const rendered = renderHtml(emailSequencePanel(baseSequence))

    expect(rendered).toContain('data-email-sequence-panel="welcome"')
    expect(rendered).toContain('data-email-sequence-status="active"')
    expect(rendered).toContain('New customer welcome')
    expect(rendered).toContain('Audience: customer')
    expect(rendered).toContain('data-email-sequence-step="step-1"')
    expect(rendered).toContain('data-email-sequence-step="step-2"')
    expect(rendered).toContain('Welcome')
    expect(rendered).toContain('Immediately · signup_day_0')
    expect(rendered).toContain('After 2 days')
    expect(rendered).toContain('2 steps in this sequence.')
  })

  it('renders an empty-steps message when there are no steps', () => {
    const rendered = renderHtml(
      emailSequencePanel({ ...baseSequence, steps: [] }),
    )

    expect(rendered).toContain('data-email-sequence-no-steps=""')
    expect(rendered).toContain('This sequence has no steps yet.')
    expect(rendered).toContain('0 steps in this sequence.')
  })

  it('does not render an enroll button without enrollAttrs', () => {
    const rendered = renderHtml(emailSequencePanel(baseSequence))

    expect(rendered).not.toContain('Join this sequence')
  })

  it('renders an enabled enroll button for active sequences with enrollAttrs', () => {
    const rendered = renderHtml(
      emailSequencePanel(baseSequence, { enrollAttrs: [] }),
    )

    expect(rendered).toContain('Join this sequence')
    expect(rendered).not.toMatch(/<button[^>]*\sdisabled[\s>]/)
  })

  it('renders a disabled enroll button for non-active sequences', () => {
    const rendered = renderHtml(
      emailSequencePanel(
        { ...baseSequence, status: 'paused' },
        { enrollAttrs: [] },
      ),
    )

    expect(rendered).toContain('Not accepting signups')
    expect(rendered).toMatch(/<button[^>]*\sdisabled[\s>]/)
  })

  it('hides the enroll button when the viewer is already enrolled', () => {
    const rendered = renderHtml(
      emailSequencePanel(baseSequence, {
        enrollAttrs: [],
        enrollment: { state: 'enrolled', scheduledSendCount: 2 },
      }),
    )

    expect(rendered).not.toContain('Join this sequence')
    expect(rendered).toContain('data-email-sequence-enrollment="enrolled"')
  })
})

describe('emailSequenceEnrollmentStatus', () => {
  const status = (model: EmailSequenceEnrollmentModel): string =>
    renderHtml(emailSequenceEnrollmentStatus(model))

  it('renders the enrolled state with a scheduled-send count', () => {
    const rendered = status({ state: 'enrolled', scheduledSendCount: 3 })

    expect(rendered).toContain('data-email-sequence-enrollment="enrolled"')
    expect(rendered).toContain('You are enrolled. 3 emails scheduled.')
  })

  it('renders a singular send count correctly', () => {
    const rendered = status({ state: 'enrolled', scheduledSendCount: 1 })

    expect(rendered).toContain('You are enrolled. 1 email scheduled.')
  })

  it('renders the not-enrolled state', () => {
    const rendered = status({ state: 'none' })

    expect(rendered).toContain('data-email-sequence-enrollment="none"')
    expect(rendered).toContain('You are not enrolled in this sequence.')
  })

  it('renders the skipped state with a preference reason', () => {
    const rendered = status({
      state: 'skipped',
      skipReason: 'drip_preference_disabled',
    })

    expect(rendered).toContain('data-email-sequence-enrollment="skipped"')
    expect(rendered).toContain(
      'You have turned off these emails in your preferences.',
    )
  })

  it('renders the skipped state with a suppression reason', () => {
    const rendered = status({ state: 'skipped', skipReason: 'drip_suppressed' })

    expect(rendered).toContain(
      'These emails are suppressed for your address.',
    )
  })
})
