import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  AutopilotPage,
  BlogIndexPage,
  BlogPostPage,
  BusinessPage,
  DocPageView,
  DocsIndexPage,
  KhalaCodeDownloadPage,
} from './-funnel-components'
import { findBlogPost, findDocPage } from './-funnel-data'

describe('Start funnel routes', () => {
  test('server-renders the business funnel copy and intake form', () => {
    const html = renderToStaticMarkup(<BusinessPage />)

    expect(html).toContain('data-route="business"')
    expect(html).toContain('Agents that work.')
    expect(html).toContain('Talk to Khala')
    expect(html).toContain('KHALA · INTAKE')
    expect(html).toContain('data-business-intake-chat=""')
    expect(html).toContain('name="businessName"')
    expect(html).toContain('name="requestSlackChannel"')
    expect(html).toContain('Request a shared Slack channel')
    expect(html).toContain('Coding &amp; agent work')
    expect(html).toContain(
      'The full paid card/Bitcoin-to-credit-to-inference loop is not collectable end-to-end in production yet.',
    )
    expect(html).toContain('Quick win -&gt; put your business on Autopilot')
    expect(html).toContain('Rate card')
  })

  test('server-renders the docs index and developer API page copy', () => {
    const indexHtml = renderToStaticMarkup(<DocsIndexPage />)
    const apiPage = findDocPage('api')

    expect(apiPage).toBeDefined()
    expect(indexHtml).toContain('OpenAgents docs')
    expect(indexHtml).toContain('Khala Code + OpenAgents Overview')
    expect(indexHtml).not.toContain('Autopilot Basics')

    const apiHtml = renderToStaticMarkup(<DocPageView page={apiPage!} />)
    expect(apiHtml).toContain('Developer API')
    expect(apiHtml).toContain(
      'Instruction files are onboarding material, not authority.',
    )
    expect(apiHtml).toContain('Live Scoped Actions')
    expect(apiHtml).toContain('Omni SDK Seed')
    expect(apiHtml).toContain('/.well-known/openagents.json')
  })

  test('server-renders the blog index and Khala Code post copy', () => {
    const indexHtml = renderToStaticMarkup(<BlogIndexPage />)
    const post = findBlogPost('introducing-khala-code')

    expect(post).toBeDefined()
    expect(indexHtml).toContain('OpenAgents Blog')
    expect(indexHtml).toContain('Introducing Khala Code')

    const postHtml = renderToStaticMarkup(<BlogPostPage post={post!} />)
    expect(postHtml).toContain('The coding front door')
    expect(postHtml).toContain(
      'Khala Code is the OpenAgents front door for coding work.',
    )
    expect(postHtml).toContain('Free, paid, and the honest promise state')
  })

  test('server-renders the Khala Code download page inside the promise gate', () => {
    const html = renderToStaticMarkup(<KhalaCodeDownloadPage />)

    expect(html).toContain('data-route="khala-code-download"')
    expect(html).toContain('npm install -g @openai/codex')
    expect(html).toContain('codex login')
    expect(html).toContain('npm install -g @openagentsinc/khala')
    expect(html).toContain('public artifact pending')
    expect(html).toContain('/api/public/khala-code/download-counts')
    expect(html).toContain('empty counts array')
    expect(html).toContain('khala_code.desktop_codex_wrapper.v1')
  })

  test('server-renders the Autopilot legal vertical copy', () => {
    const html = renderToStaticMarkup(<AutopilotPage legal />)

    expect(html).toContain('data-route="autopilot-legal"')
    expect(html).toContain('For legal teams')
    expect(html).toContain('Not an AI lawyer, not case-law research.')
    expect(html).toContain('data-autopilot-onboarding-legal-overlay=""')
    expect(html).toContain('ABA Op. 512')
  })
})
