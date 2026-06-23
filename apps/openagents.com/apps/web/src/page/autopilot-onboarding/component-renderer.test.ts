import { describe, expect, test } from 'vitest'

import {
  COMPONENT_FRAME_VERSION,
  validateComponentFrame,
} from './component-catalog'
import { renderComponentFrame } from './component-renderer'

// A snabbdom VNode is `{ sel, data: { attrs }, children, text }`. These walkers
// collect every `data-ui-base` AI-Elements tag and all rendered text so we can
// assert a frame produced trusted AI-Elements markup and the expected content.
type VNodeLike = {
  sel?: string
  text?: string
  data?: { attrs?: Record<string, unknown> }
  children?: ReadonlyArray<VNodeLike | string>
}

const walk = (node: unknown, visit: (n: VNodeLike) => void): void => {
  if (node === null || node === undefined || typeof node === 'string') {
    return
  }

  const n = node as VNodeLike
  visit(n)
  for (const child of n.children ?? []) {
    walk(child, visit)
  }
}

const uiBaseTags = (node: unknown): ReadonlyArray<string> => {
  const tags: Array<string> = []
  walk(node, n => {
    const base = n.data?.attrs?.['data-ui-base']
    if (typeof base === 'string') {
      tags.push(base)
    }
  })
  return tags
}

const allText = (node: unknown): string => {
  const parts: Array<string> = []
  walk(node, n => {
    if (typeof n.text === 'string') {
      parts.push(n.text)
    }
  })
  return parts.join(' ')
}

const catalogFrame = (component: string, props: unknown) =>
  validateComponentFrame({
    v: COMPONENT_FRAME_VERSION,
    component,
    props,
    id: 'cmp_01',
  })

describe('renderComponentFrame — per-component render', () => {
  test('credit_kickoff renders an AI-Elements confirmation with the amount', () => {
    const node = renderComponentFrame(
      catalogFrame('credit_kickoff', {
        amountCents: 50000,
        label: 'Kick off with $500 in credits',
      }),
    )

    expect(node).not.toBeNull()
    expect(uiBaseTags(node)).toContain('ai-elements:confirmation/Confirmation')
    expect(allText(node)).toContain('$500')
  })

  test('intake_progress renders an AI-Elements task with each step', () => {
    const node = renderComponentFrame(
      catalogFrame('intake_progress', {
        steps: [
          { id: 's1', label: 'Your business' },
          { id: 's2', label: 'Painful work' },
        ],
        current: 1,
      }),
    )

    expect(uiBaseTags(node)).toContain('ai-elements:task/Task')
    const text = allText(node)
    expect(text).toContain('Your business')
    expect(text).toContain('Painful work')
  })

  test('quick_win_card renders the title, scope and ETA', () => {
    const node = renderComponentFrame(
      catalogFrame('quick_win_card', {
        title: 'Automate intake',
        scope: 'Replace the 60-minute intake',
        etaDays: 5,
      }),
    )

    const text = allText(node)
    expect(text).toContain('Automate intake')
    expect(text).toContain('Replace the 60-minute intake')
    expect(text).toContain('5 days')
  })

  test('dashboard_preview renders the workspace ref and seeded facts', () => {
    const node = renderComponentFrame(
      catalogFrame('dashboard_preview', {
        workspaceRef: 'ws_demo_01',
        seededFacts: ['15 LLCs per year', '~1 hour each'],
      }),
    )

    const text = allText(node)
    expect(text).toContain('ws_demo_01')
    expect(text).toContain('15 LLCs per year')
    expect(text).toContain('~1 hour each')
  })

  test('human_handoff renders the reason and contact', () => {
    const node = renderComponentFrame(
      catalogFrame('human_handoff', {
        reason: 'This needs a person to review.',
        contact: 'team@openagents.com',
      }),
    )

    const text = allText(node)
    expect(text).toContain('This needs a person to review.')
    expect(text).toContain('team@openagents.com')
  })

  test('consent_gate renders the data practices and a confirmation', () => {
    const node = renderComponentFrame(
      catalogFrame('consent_gate', {
        scope: 'Process your client documents',
        dataPractices: ['Redacted before inference', 'Never used for training'],
        required: true,
      }),
    )

    expect(uiBaseTags(node)).toContain('ai-elements:confirmation/Confirmation')
    const text = allText(node)
    expect(text).toContain('Process your client documents')
    expect(text).toContain('Redacted before inference')
  })
})

describe('renderComponentFrame — registry fallback (never arbitrary markup)', () => {
  test('an unknown component renders the safe fallback, not a crash', () => {
    const node = renderComponentFrame(
      validateComponentFrame({
        v: COMPONENT_FRAME_VERSION,
        component: 'inject_arbitrary_markup',
        props: { html: '<script>alert(1)</script>' },
        id: 'cmp_evil',
      }),
    )

    expect(node).not.toBeNull()
    const text = allText(node)
    // The fallback is the human_handoff view; the model-authored component name
    // and props are NEVER rendered.
    expect(text).not.toContain('inject_arbitrary_markup')
    expect(text).not.toContain('<script>')
    expect(text).not.toContain('alert(1)')
    expect(text.toLowerCase()).toContain('teammate')
  })

  test('a catalog component with invalid props also renders the safe fallback', () => {
    const node = renderComponentFrame(
      validateComponentFrame({
        v: COMPONENT_FRAME_VERSION,
        component: 'credit_kickoff',
        props: { amountCents: -1, label: '' },
        id: 'cmp_bad',
      }),
    )

    expect(node).not.toBeNull()
    expect(allText(node).toLowerCase()).toContain('teammate')
  })
})
