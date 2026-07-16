import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { SplashPage } from './-splash-page'

describe('Desktop splash', () => {
  test('server-renders the live workroom reconstruction and its accessible controls', () => {
    const html = renderToStaticMarkup(<SplashPage />)

    expect(html).toContain('data-route="splash"')
    expect(html).toContain('OpenAgents Desktop live product preview')
    expect(html).toContain('T3CODE YOINK')
    expect(html).toContain('Use the actual Desktop components on web')
    expect(html).toContain('data-en-react-surface="true"')
    expect(html).toContain('data-chat-composer="true"')
    expect(html).toContain('packages/ui')
    expect(html).toContain('Steer a Codex message')
    expect(html).not.toContain('<img')
  })
})
