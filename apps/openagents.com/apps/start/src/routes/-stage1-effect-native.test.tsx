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

describe('WEB-1-EN stage1 Effect Native marketing-catalog landing (#8595)', () => {
  test('server render is only a thin mount shim, not landing-content React', () => {
    const html = renderToStaticMarkup(<Stage1EffectNativePage />)

    expect(html).toContain('data-route="stage1-effect-native"')
    expect(html).toContain('data-stage1-effect-native-root=""')
    expect(html).toContain('data-web1-en-marketing-catalog=""')
    expect(html).not.toContain('Give your big idea the design it deserves')
  })

  test('authored landing content is a typed Effect Native marketing tree', () => {
    const tree = stage1LandingView(initialStage1LandingState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({
      tag: 'Stack',
      key: 'stage1-root',
    })
    expect(serialized).toContain('"catalogVersion":"effect-native/v25"')
    // Marketing catalog tags (v20 wave)
    expect(serialized).toContain('"_tag":"NavBar"')
    expect(serialized).toContain('"_tag":"Hero"')
    expect(serialized).toContain('"_tag":"AnnouncementBadge"')
    expect(serialized).toContain('"_tag":"LogoRow"')
    expect(serialized).toContain('"_tag":"StatsBand"')
    expect(serialized).toContain('"_tag":"Accordion"')
    expect(serialized).toContain('"_tag":"CtaSection"')
    expect(serialized).toContain('"_tag":"Footer"')
    expect(serialized).toContain('"_tag":"Glow"')
    expect(serialized).toContain('"_tag":"MockupFrame"')
    // Launch-ui replica copy (preserved for /new parity)
    expect(serialized).toContain('Give your big idea the design it deserves')
    expect(serialized).toContain('Launch UI v2 is out!')
    expect(serialized).toContain("Everything you need. Nothing you don't.")
    expect(serialized).toContain('Questions and Answers')
    expect(serialized).toContain('Start building')
    // Intents
    expect(serialized).toContain('Stage1Navigated')
    expect(serialized).toContain('Stage1FaqToggled')
    expect(serialized).not.toContain('className')
    // No launch-ui React imports in the tree
    expect(serialized).not.toContain('launch-ui')
  })

  test('hydrated pricing state produces PricingTable columns', () => {
    const hydrated = {
      ...initialStage1LandingState,
      ...stage1StateFromPublicSnapshots(
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
      ),
    }
    const serialized = JSON.stringify(stage1LandingView(hydrated))
    expect(serialized).toContain('"_tag":"PricingTable"')
    expect(serialized).toContain('"_tag":"PricingColumn"')
    expect(serialized).toContain('Get started')
    expect(serialized).toContain('1,234,567')
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

  test('source boundary uses Effect Native marketing packages, not launch-ui sections', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-stage1-effect-native-page.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).toContain('Hero')
    expect(source).toContain('NavBar')
    expect(source).toContain('StatsBand')
    expect(source).toContain('PricingTable')
    expect(source).toContain('Accordion')
    expect(source).toContain('MockupFrame')
    expect(source).not.toContain('@/components/launch-ui')
    expect(source).not.toContain('lucide-react')
  })
})
