import { describe, expect, test } from 'vitest'

import {
  COMPONENT_FRAME_VERSION,
  COMPONENT_SSE_EVENT,
  componentNames,
  isUnknownComponentFrame,
  parseComponentFrames,
  validateComponentFrame,
  type RenderableFrame,
} from './component-catalog'

const at = (
  frames: ReadonlyArray<RenderableFrame>,
  index: number,
): RenderableFrame => {
  const frame = frames[index]
  if (frame === undefined) {
    throw new Error(`expected a frame at index ${index}`)
  }
  return frame
}

const frame = (component: string, props: unknown, id = 'cmp_01') => ({
  v: COMPONENT_FRAME_VERSION,
  component,
  props,
  id,
})

const sseFrame = (component: string, props: unknown, id = 'cmp_01') =>
  `event: ${COMPONENT_SSE_EVENT}\ndata: ${JSON.stringify(frame(component, props, id))}\n\n`

describe('component catalog — closed v1 set', () => {
  test('exposes exactly the six v1 component names', () => {
    expect(componentNames).toEqual([
      'credit_kickoff',
      'intake_progress',
      'quick_win_card',
      'dashboard_preview',
      'human_handoff',
      'consent_gate',
    ])
  })
})

describe('validateComponentFrame — per-component valid props', () => {
  test('credit_kickoff', () => {
    const result = validateComponentFrame(
      frame('credit_kickoff', {
        amountCents: 50000,
        label: 'Kick off with $500 in credits',
      }),
    )

    expect(result._tag).toBe('CatalogComponentFrame')
    if (result._tag === 'CatalogComponentFrame') {
      expect(result.frame.component).toBe('credit_kickoff')
    }
  })

  test('intake_progress', () => {
    const result = validateComponentFrame(
      frame('intake_progress', {
        steps: [
          { id: 's1', label: 'Your business' },
          { id: 's2', label: 'Painful work' },
        ],
        current: 1,
      }),
    )

    expect(result._tag).toBe('CatalogComponentFrame')
  })

  test('quick_win_card', () => {
    const result = validateComponentFrame(
      frame('quick_win_card', {
        title: 'Automate intake',
        scope: 'Replace the 60-minute intake with a guided form',
        etaDays: 5,
      }),
    )

    expect(result._tag).toBe('CatalogComponentFrame')
  })

  test('dashboard_preview', () => {
    const result = validateComponentFrame(
      frame('dashboard_preview', {
        workspaceRef: 'ws_demo_01',
        seededFacts: ['15 LLCs per year', '~1 hour each'],
      }),
    )

    expect(result._tag).toBe('CatalogComponentFrame')
  })

  test('human_handoff', () => {
    const result = validateComponentFrame(
      frame('human_handoff', {
        reason: 'This needs a person to review.',
        contact: 'team@openagents.com',
      }),
    )

    expect(result._tag).toBe('CatalogComponentFrame')
  })

  test('consent_gate', () => {
    const result = validateComponentFrame(
      frame('consent_gate', {
        scope: 'Process your client documents',
        dataPractices: ['Redacted before inference', 'Never used for training'],
        required: true,
      }),
    )

    expect(result._tag).toBe('CatalogComponentFrame')
  })
})

describe('validateComponentFrame — registry fallback (the real guarantee)', () => {
  test('unknown component name falls back to typed-unknown, never crashes', () => {
    const result = validateComponentFrame(
      frame('inject_arbitrary_markup', { html: '<script>alert(1)</script>' }),
    )

    expect(isUnknownComponentFrame(result)).toBe(true)
    if (isUnknownComponentFrame(result)) {
      expect(result.component).toBe('inject_arbitrary_markup')
      expect(result.reason).toBe('component_not_in_closed_catalog')
    }
  })

  test('catalog component with invalid props falls back to typed-unknown', () => {
    const result = validateComponentFrame(
      // amountCents must be a positive integer
      frame('credit_kickoff', { amountCents: -5, label: 'bad' }),
    )

    expect(isUnknownComponentFrame(result)).toBe(true)
    if (isUnknownComponentFrame(result)) {
      expect(result.component).toBe('credit_kickoff')
      expect(result.reason).toBe('component_props_failed_schema_validation')
    }
  })

  test('catalog component missing required props falls back', () => {
    const result = validateComponentFrame(
      frame('quick_win_card', { title: 'Only a title' }),
    )

    expect(isUnknownComponentFrame(result)).toBe(true)
  })

  test('wrong version falls back rather than rendering', () => {
    const result = validateComponentFrame({
      v: 999,
      component: 'credit_kickoff',
      props: { amountCents: 50000, label: 'x' },
      id: 'cmp_01',
    })

    expect(isUnknownComponentFrame(result)).toBe(true)
  })

  test('non-object / garbage input does not crash', () => {
    expect(isUnknownComponentFrame(validateComponentFrame(null))).toBe(true)
    expect(isUnknownComponentFrame(validateComponentFrame('nope'))).toBe(true)
    expect(isUnknownComponentFrame(validateComponentFrame(42))).toBe(true)
    expect(isUnknownComponentFrame(validateComponentFrame([1, 2, 3]))).toBe(true)
  })
})

describe('parseComponentFrames — SSE consumption', () => {
  test('parses a single oa.component frame', () => {
    const frames = parseComponentFrames(
      sseFrame('credit_kickoff', {
        amountCents: 50000,
        label: 'Kick off with $500 in credits',
      }),
    )

    expect(frames).toHaveLength(1)
    expect(at(frames, 0)._tag).toBe('CatalogComponentFrame')
  })

  test('ignores non-component events (prose / OpenAI chunks) and interleaves correctly', () => {
    const stream = [
      'event: message\ndata: {"choices":[{"delta":{"content":"Hello, "}}]}\n\n',
      sseFrame('intake_progress', {
        steps: [{ id: 's1', label: 'Your business' }],
        current: 0,
      }),
      'event: message\ndata: {"choices":[{"delta":{"content":"let me show you."}}]}\n\n',
      sseFrame('credit_kickoff', { amountCents: 50000, label: 'Kick off' }, 'cmp_02'),
    ].join('')

    const frames = parseComponentFrames(stream)

    // Only the two oa.component frames are surfaced, in document order.
    expect(frames).toHaveLength(2)
    const first = at(frames, 0)
    const second = at(frames, 1)
    expect(first._tag).toBe('CatalogComponentFrame')
    expect(second._tag).toBe('CatalogComponentFrame')
    if (
      first._tag === 'CatalogComponentFrame' &&
      second._tag === 'CatalogComponentFrame'
    ) {
      expect(first.frame.component).toBe('intake_progress')
      expect(second.frame.component).toBe('credit_kickoff')
    }
  })

  test('an invalid oa.component frame becomes a typed-unknown fallback, not dropped', () => {
    const frames = parseComponentFrames(
      sseFrame('made_up_component', { anything: true }),
    )

    expect(frames).toHaveLength(1)
    expect(isUnknownComponentFrame(at(frames, 0))).toBe(true)
  })

  test('handles an empty / prose-only stream', () => {
    expect(parseComponentFrames('')).toHaveLength(0)
    expect(
      parseComponentFrames(
        'event: message\ndata: {"choices":[{"delta":{"content":"hi"}}]}\n\n',
      ),
    ).toHaveLength(0)
  })
})
