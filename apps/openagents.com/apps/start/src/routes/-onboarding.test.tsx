import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { OnboardingPage } from './-onboarding-page'

describe('Start /onboarding route', () => {
  test('server-renders the GitHub-login landing a real anonymous visitor sees', () => {
    const html = renderToStaticMarkup(<OnboardingPage />)

    expect(html).toContain('data-route="onboarding"')
    expect(html).toContain('OpenAgents Autopilot')
    expect(html).toContain('Stop Babysitting Your AI')
    expect(html).toContain(
      'Launch coding agents. Close your laptop. Stay in the loop from anywhere.',
    )
    expect(html).toContain('Start work. Walk away.')
    expect(html).toContain('Your agents keep going.')
  })

  test('links both the header and hero CTA to the GitHub login route', () => {
    const html = renderToStaticMarkup(<OnboardingPage />)
    const loginLinkCount = (html.match(/href="\/login\/github"/g) ?? []).length

    expect(loginLinkCount).toBe(2)
    expect(html).toContain('Log in with GitHub')
  })

  test('does not fabricate the funding-demo step, which is unreachable from this route', () => {
    const html = renderToStaticMarkup(<OnboardingPage />)

    expect(html).not.toContain('Fund your account')
    expect(html).not.toContain('Funding amount')
  })
})
