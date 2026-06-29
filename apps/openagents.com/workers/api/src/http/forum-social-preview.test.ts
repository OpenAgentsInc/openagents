import { describe, expect, test } from 'vitest'

import {
  type ForumThreadPreviewSource,
  buildForumThreadOgImageSvg,
  buildSocialPreviewExcerpt,
  forumThreadOgImageResponse,
  forumThreadSocialPreviewFromDetail,
  injectForumThreadSocialPreviewMeta,
  renderForumThreadSocialPreviewMeta,
} from './forum-social-preview'

const detailWith = (
  overrides: Partial<{
    bodyText: string | null
    title: string
    topicId: string
  }> = {},
): ForumThreadPreviewSource => ({
  posts: [
    {
      bodyText:
        overrides.bodyText === undefined
          ? 'First post body.'
          : overrides.bodyText,
    },
  ],
  topic: {
    title: overrides.title ?? 'Hello Thread',
    ...(overrides.topicId === undefined ? {} : { topicId: overrides.topicId }),
  },
})

describe('buildSocialPreviewExcerpt', () => {
  test('returns empty string for null/undefined body', () => {
    expect(buildSocialPreviewExcerpt(null)).toBe('')
    expect(buildSocialPreviewExcerpt(undefined)).toBe('')
  })

  test('strips markdown noise, code fences, and bare URLs', () => {
    const body = [
      '# Heading',
      '',
      'Some **bold** and `inline code` text with a [link](https://example.com).',
      '',
      '```ts',
      'const secret = 1',
      '```',
      '',
      '> a quote',
      '- a list item',
      'See https://openagents.com/forum for more.',
    ].join('\n')

    const excerpt = buildSocialPreviewExcerpt(body)

    expect(excerpt).not.toContain('#')
    expect(excerpt).not.toContain('**')
    expect(excerpt).not.toContain('`')
    expect(excerpt).not.toContain('https://')
    expect(excerpt).not.toContain('const secret')
    expect(excerpt).toContain('Heading')
    expect(excerpt).toContain('bold')
    expect(excerpt).toContain('inline code')
    expect(excerpt).toContain('link')
    expect(excerpt).toContain('a quote')
    expect(excerpt).toContain('a list item')
  })

  test('truncates near the max length on a word boundary with an ellipsis', () => {
    const body = 'word '.repeat(200).trim()
    const excerpt = buildSocialPreviewExcerpt(body, 40)

    expect(excerpt.length).toBeLessThanOrEqual(41)
    expect(excerpt.endsWith('…')).toBe(true)
    expect(excerpt).not.toContain('  ')
  })
})

describe('forumThreadSocialPreviewFromDetail', () => {
  test('uses the topic title and first-post excerpt', () => {
    const preview = forumThreadSocialPreviewFromDetail('t1', detailWith())

    expect(preview.title).toBe('Hello Thread')
    expect(preview.description).toBe('First post body.')
    expect(preview.url).toBe('https://openagents.com/forum/t/t1')
    expect(preview.imageUrl).toBe('https://openagents.com/og/forum/t1.svg')
  })

  test('falls back to a branded default card for a missing topic', () => {
    const preview = forumThreadSocialPreviewFromDetail('missing-id', null)

    expect(preview.title).toBe('OpenAgents Forum')
    expect(preview.description.length).toBeGreaterThan(0)
    expect(preview.url).toBe('https://openagents.com/forum/t/missing-id')
    expect(preview.imageUrl).toBe('https://openagents.com/og/forum/default.svg')
  })

  test('falls back to the default description when the first post is empty', () => {
    const preview = forumThreadSocialPreviewFromDetail(
      't1',
      detailWith({ bodyText: null }),
    )

    expect(preview.title).toBe('Hello Thread')
    expect(preview.description.length).toBeGreaterThan(0)
  })

  test('percent-encodes the topic id in derived URLs', () => {
    const preview = forumThreadSocialPreviewFromDetail('a b/c', detailWith())

    expect(preview.url).toBe('https://openagents.com/forum/t/a%20b%2Fc')
    expect(preview.imageUrl).toBe(
      'https://openagents.com/og/forum/a%20b%2Fc.svg',
    )
  })

  test('uses the resolved topic id for the canonical thread URL', () => {
    const preview = forumThreadSocialPreviewFromDetail(
      'hello-thread',
      detailWith({ topicId: '55555555-5555-4555-8555-555555555555' }),
    )

    expect(preview.url).toBe(
      'https://openagents.com/forum/t/55555555-5555-4555-8555-555555555555',
    )
    expect(preview.imageUrl).toBe(
      'https://openagents.com/og/forum/hello-thread.svg',
    )
  })
})

describe('renderForumThreadSocialPreviewMeta', () => {
  test('emits the full og + twitter tag set', () => {
    const meta = renderForumThreadSocialPreviewMeta(
      forumThreadSocialPreviewFromDetail('t1', detailWith()),
    )

    expect(meta).toContain('property="og:type" content="article"')
    expect(meta).toContain('property="og:site_name" content="OpenAgents"')
    expect(meta).toContain('property="og:title" content="Hello Thread"')
    expect(meta).toContain(
      'property="og:description" content="First post body."',
    )
    expect(meta).toContain(
      'property="og:url" content="https://openagents.com/forum/t/t1"',
    )
    expect(meta).toContain(
      'property="og:image" content="https://openagents.com/og/forum/t1.svg"',
    )
    expect(meta).toContain('property="og:image:width" content="1200"')
    expect(meta).toContain('property="og:image:height" content="630"')
    expect(meta).toContain('name="twitter:card" content="summary_large_image"')
    expect(meta).toContain('name="twitter:title" content="Hello Thread"')
    expect(meta).toContain(
      'name="twitter:description" content="First post body."',
    )
    expect(meta).toContain(
      'name="twitter:image" content="https://openagents.com/og/forum/t1.svg"',
    )
  })

  test('HTML-attribute escapes title and description (XSS-safe)', () => {
    const meta = renderForumThreadSocialPreviewMeta(
      forumThreadSocialPreviewFromDetail(
        't1',
        detailWith({
          title: 'Pwn "><script>alert(1)</script>',
          bodyText: 'Body & <b>bold</b> "quotes" with \'apostrophe\'',
        }),
      ),
    )

    expect(meta).not.toContain('<script>')
    expect(meta).toContain('&lt;script&gt;')
    expect(meta).toContain('&quot;')
    expect(meta).toContain('&amp;')
    expect(meta).toContain('&#39;')
  })
})

describe('injectForumThreadSocialPreviewMeta', () => {
  const shell =
    '<!doctype html><html><head><title>OpenAgents</title></head><body><div id="root"></div></body></html>'

  test('injects the meta block before </head>', () => {
    const out = injectForumThreadSocialPreviewMeta(
      shell,
      forumThreadSocialPreviewFromDetail('t1', detailWith()),
    )

    expect(out).toContain('property="og:title" content="Hello Thread"')
    expect(out.indexOf('og:title')).toBeLessThan(out.indexOf('</head>'))
    expect(out).toContain('<div id="root">')
  })

  test('is idempotent', () => {
    const preview = forumThreadSocialPreviewFromDetail('t1', detailWith())
    const once = injectForumThreadSocialPreviewMeta(shell, preview)
    const twice = injectForumThreadSocialPreviewMeta(once, preview)

    expect(twice).toBe(once)
  })
})

describe('buildForumThreadOgImageSvg', () => {
  test('produces a 1200x630 svg with the escaped title', () => {
    const svg = buildForumThreadOgImageSvg('Hello <Thread> & "Friends"')

    expect(svg.startsWith('<svg')).toBe(true)
    expect(svg).toContain('width="1200"')
    expect(svg).toContain('height="630"')
    expect(svg).toContain('OPENAGENTS FORUM')
    expect(svg).not.toContain('<Thread>')
    expect(svg).toContain('&lt;Thread&gt;')
    expect(svg).toContain('&amp;')
  })

  test('uses the default title when none is provided', () => {
    expect(buildForumThreadOgImageSvg(null)).toContain('OpenAgents Forum')
    expect(buildForumThreadOgImageSvg('   ')).toContain('OpenAgents Forum')
  })
})

describe('forumThreadOgImageResponse', () => {
  test('returns an svg image response', async () => {
    const response = forumThreadOgImageResponse('Hello Thread')

    expect(response.headers.get('content-type')).toBe(
      'image/svg+xml; charset=utf-8',
    )
    const body = await response.text()
    expect(body).toContain('<svg')
    expect(body).toContain('Hello Thread')
  })
})
