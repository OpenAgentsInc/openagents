import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { ComponentsPage } from './-components-page'

describe('Start components workbench routes', () => {
  test('server-renders the component family inventory', () => {
    const html = renderToStaticMarkup(<ComponentsPage />)

    expect(html).toContain('data-route="components"')
    expect(html).toContain('Component library')
    expect(html).toContain('Internal - design-system workbench')
    expect(html).toContain('Primitives')
    expect(html).toContain('Shared')
    expect(html).toContain('Forms')
    expect(html).toContain('Layout')
    expect(html).toContain('Navigation')
    expect(html).toContain('Data display')
    expect(html).toContain('Feedback')
    expect(html).toContain('Workroom')
    expect(html).toContain('Public')
    expect(html).toContain('Public theme')
    expect(html).toContain('Business landing')
    expect(html).toContain('Page examples')
    expect(html).toContain('V4')
    expect(html).toContain('AI Elements')
    expect(html).toContain('Live samples')
  })

  test('server-renders representative live preview anchors', () => {
    const html = renderToStaticMarkup(<ComponentsPage />)

    expect(html).toContain('inputGroup')
    expect(html).toContain('tableList')
    expect(html).toContain('marketingHero')
    expect(html).toContain('businessIntakeForm')
    expect(html).toContain('workroomTimeline')
    expect(html).toContain('v4Composer')
    expect(html).toContain('applicationHomeScreen')
    expect(html).toContain('AiElements.promptInput')
  })

  test('server-renders selected component families', () => {
    const dataDisplay = renderToStaticMarkup(
      <ComponentsPage selectedFamily="data-display" />,
    )
    const training = renderToStaticMarkup(<ComponentsPage selectedFamily="training" />)
    const business = renderToStaticMarkup(<ComponentsPage selectedFamily="business" />)
    const publicTheme = renderToStaticMarkup(
      <ComponentsPage selectedFamily="public-theme" />,
    )

    expect(dataDisplay).toContain('Data display')
    expect(dataDisplay).toContain('tableList')
    expect(dataDisplay).toContain('Contract')
    expect(training).toContain('Training grammar')
    expect(training).toContain('Run field')
    expect(training).toContain('Contributor node')
    expect(training).toContain('Replay pair')
    expect(training).toContain('Verification gate')
    expect(training).toContain('Receipt burst')
    expect(training).toContain('Proof drawer')
    expect(training).toContain('oa-training-run / @openagentsinc/three-effect')
    expect(training).toContain('oa-training-grammar-replay-pair')
    expect(business).toContain('Business landing')
    expect(business).toContain('businessOfferingMenu mode=light')
    expect(business).toContain('businessIntakeForm')
    expect(business).toContain('data-ui-family business/* markers')
    expect(publicTheme).toContain('Public theme')
    expect(publicTheme).toContain('publicLandingThemeSelector')
    expect(publicTheme).toContain('publicLandingThemeShell mode=light')
    expect(publicTheme).toContain('publicLandingThemeShell mode=dark')
    expect(publicTheme).toContain('Shell-scoped theme')
    expect(publicTheme).toContain('data-public-landing-shell')
  })
})
