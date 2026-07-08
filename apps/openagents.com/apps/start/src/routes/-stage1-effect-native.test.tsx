import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  Stage1EffectNativePage,
  initialStage1LandingState,
  stage1LandingView,
  stage1StateFromPublicSnapshots,
} from './-stage1-effect-native-page'

describe('EN-1 stage1 Effect Native route', () => {
  test('server render is only a thin mount shim, not landing-content React', () => {
    const html = renderToStaticMarkup(<Stage1EffectNativePage />)

    expect(html).toContain('data-route="stage1-effect-native"')
    expect(html).toContain('data-stage1-effect-native-root=""')
    expect(html).not.toContain('Software, built by agents.')
  })

  test('authored landing content is a typed Effect Native tree', () => {
    const tree = stage1LandingView(initialStage1LandingState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({
      tag: 'Stack',
      key: 'stage1-root',
    })
    expect(serialized).toContain('"catalogVersion":"effect-native/v0"')
    expect(serialized).toContain('Software, built by agents.')
    expect(serialized).toContain('Stage1Navigated')
    expect(serialized).not.toContain('className')
  })

  test('public projection snapshots hydrate stats and plan cards', () => {
    const state = stage1StateFromPublicSnapshots(
      { tokensServed: 1234567 },
      {
        pylonsOnlineNow: 4,
        pylonsAssignmentReadyNow: 3,
        trainingModelProgressContributors: 2,
      },
      {
        summary: 'Public catalog summary',
        plans: [
          {
            planId: 'free',
            kind: 'free',
            label: 'Free',
            tagline: 'Start with the free public lane.',
            priceLabel: '$0',
            isDefault: true,
            captureExcluded: false,
            terms: ['No card required', 'Public receipts'],
          },
        ],
      },
    )

    expect(state.stats.map((stat) => stat.value)).toEqual([
      '1,234,567',
      '4',
      '3',
      '2',
    ])
    expect(state.pricingState).toBe('ready')
    expect(state.planSummary).toBe('Public catalog summary')
    expect(state.plans[0]?.cta).toBe('Get started')
  })

  test('source boundary uses Effect Native packages instead of launch-ui content components', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-stage1-effect-native-page.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).not.toContain('@/components/launch-ui')
    expect(source).not.toContain('lucide-react')
  })
})
