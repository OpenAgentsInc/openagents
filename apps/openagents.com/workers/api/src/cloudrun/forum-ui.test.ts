// #8634 / #8635 scope 5: /forum* monolith serving shell (EN live cutover).
import { describe, expect, test } from 'vitest'

import { forumDocumentTitle, forumPageHtml, handleForumUiRequest } from './forum-ui'

describe('forum ui serving (#8634 EN live cutover)', () => {
  test('does not own non-forum paths or unconverted forum descendants', async () => {
    for (const path of [
      '/',
      '/forums',
      '/promises',
      '/api/forum/tips',
      '/forum/u/actor_1',
      '/forum/f/a/deeper',
      '/forum/unknown/x/y',
    ]) {
      expect(
        await handleForumUiRequest(new Request(`https://openagents.com${path}`)),
      ).toBeUndefined()
    }
  })

  test('serves the EN shell for the four converted forum document routes', async () => {
    const cases: ReadonlyArray<readonly [string, string]> = [
      ['/forum', 'Forum - OpenAgents'],
      ['/forum/f/product-promises', 'product-promises Forum - OpenAgents'],
      [
        '/forum/t/55555555-5555-4555-8555-555555555555',
        '55555555 Topic - OpenAgents',
      ],
      ['/forum/receipts/receipt_1', 'Forum Receipt - OpenAgents'],
    ]
    for (const [path, title] of cases) {
      const response = await handleForumUiRequest(
        new Request(`https://openagents.com${path}`),
      )
      expect(response?.status).toBe(200)
      expect(response?.headers.get('content-type')).toContain('text/html')
      const html = await response!.text()
      expect(html).toContain('id="forum-root"')
      expect(html).toContain('/forum/app.js')
      expect(html).toContain(`<title>${title}</title>`)
      expect(html).toBe(forumPageHtml(title))
    }
  })

  test('query strings and anchors do not change document ownership', async () => {
    const response = await handleForumUiRequest(
      new Request(
        'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555?sortDir=desc',
      ),
    )
    expect(response?.status).toBe(200)
  })

  test('HTML-escapes ref-derived titles', () => {
    expect(forumDocumentTitle('/forum/f/%3Cscript%3E')).toBe(
      '<script> Forum - OpenAgents',
    )
    expect(forumPageHtml('<script> Forum - OpenAgents')).not.toContain(
      '<title><script>',
    )
    expect(forumPageHtml('<script> Forum - OpenAgents')).toContain(
      '&lt;script&gt;',
    )
  })

  test('rejects non-GET methods on owned routes', async () => {
    const response = await handleForumUiRequest(
      new Request('https://openagents.com/forum', { method: 'POST' }),
    )
    expect(response?.status).toBe(405)
    expect(response?.headers.get('allow')).toBe('GET, HEAD')
  })
})
