// APP-FORUM (#8635) — route, view, auth, moderation-visibility, accessibility,
// and deep-link coverage for the retained /forum* routes.
//
// Converted from an Effect Native view tree to plain React (#9325): the
// assertions that used to inspect the serialized view tree now inspect the
// rendered React output instead.

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { act } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test, vi } from 'vitest'

import {
  parseTopicPostSortDirection,
  type ForumPostProjection,
  type ForumReceiptProjection,
  type ForumSummaryProjection,
  type ForumTopicProjection,
} from './-forum-data'
import {
  ForumForumPage,
  ForumIndexPage,
  ForumPageView,
  ForumReceiptPage,
  ForumTopicPage,
  forumReturnPath,
  initialForumPageState,
  mountForumSurface,
  type ForumPageState,
} from './-forum-page'

const NOW = Date.parse('2026-07-10T12:00:00Z')

const FORUMS: ReadonlyArray<ForumSummaryProjection> = [
  {
    forumId: 'f_products',
    slug: 'product-promises',
    title: 'Product Promises',
    description: 'Reports and discussion about live product promises.',
    topicCount: 2,
    postCount: 5,
    lastPost: {
      subject: 'Latest report',
      author: { displayName: 'Raynor' },
      createdAt: '2026-07-10T11:30:00Z',
    },
  },
  {
    forumId: 'f_void',
    slug: 'void',
    title: 'Void',
    discoverability: 'unlisted',
    locked: true,
    topicCount: 1,
    postCount: 1,
  },
]

const TOPIC: ForumTopicProjection = {
  topicId: 'topic-1',
  forumId: 'product-promises',
  title: 'A live promise report',
  postCount: 2,
  author: { displayName: 'Raynor' },
  createdAt: '2026-07-09T10:00:00Z',
}

const LOCKED_TOPIC: ForumTopicProjection = {
  ...TOPIC,
  topicId: 'topic-locked',
  title: 'A locked thread',
  locked: true,
  state: 'locked',
}

const POSTS: ReadonlyArray<ForumPostProjection> = [
  {
    postId: 'post-a',
    postNumber: 1,
    subject: 'First post',
    bodyText:
      'Hello **world** with `inline code` and a [safe link](/forum).\n\n```ts\nconst a = 1\n```\n\n- item one\n- item two',
    createdAt: '2026-07-09T10:00:00Z',
    author: {
      actorId: 'actor-1',
      slug: 'raynor',
      displayName: 'Raynor',
      role: 'agent',
      postCount: 10,
      joinedAt: '2026-06-01T00:00:00Z',
    },
  },
  {
    postId: 'post-b',
    postNumber: 2,
    bodyText: 'A reply with an [unsafe link](javascript:alert(1)).',
    createdAt: '2026-07-09T11:00:00Z',
    author: { displayName: 'Zeratul' },
  },
]

const RECEIPT: ForumReceiptProjection = {
  receiptRef: 'receipt_1',
  actionKind: 'post_tip',
  createdAt: '2026-07-10T11:00:00Z',
  amount: { amount: 25, asset: 'sats' },
  target: { topicId: 'topic-1', postId: 'post-a' },
  targetPostPermalink: '/forum/t/topic-1#post-post-a',
  recipientActorRef: 'agent:raynor',
}

const readyIndexState: ForumPageState = {
  ...initialForumPageState({ kind: 'index' }),
  phase: 'ready',
  forums: FORUMS,
}

const readyForumState: ForumPageState = {
  ...initialForumPageState({ kind: 'forum', forumRef: 'product-promises' }),
  phase: 'ready',
  forum: FORUMS[0]!,
  topics: [TOPIC, LOCKED_TOPIC],
}

const readyTopicState: ForumPageState = {
  ...initialForumPageState({
    kind: 'topic',
    topicId: 'topic-1',
    sortDirection: 'asc',
  }),
  phase: 'ready',
  topic: TOPIC,
  posts: POSTS,
  authMode: 'LoggedIn',
}

const readyReceiptState: ForumPageState = {
  ...initialForumPageState({ kind: 'receipt', receiptRef: 'receipt_1' }),
  phase: 'ready',
  receipt: RECEIPT,
}

const markup = (state: ForumPageState): string =>
  renderToStaticMarkup(<ForumPageView nowMs={NOW} state={state} />)

const jsonResponse = (body: unknown): Response =>
  new Response(JSON.stringify(body), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  })

type FetchCall = Readonly<{ url: string; init: RequestInit | undefined }>

const makeFetchStub = (
  routes: Readonly<Record<string, unknown>>,
  calls: FetchCall[],
): typeof fetch =>
  (async (input: RequestInfo | URL, init?: RequestInit) => {
    const url =
      typeof input === 'string'
        ? input
        : input instanceof URL
          ? input.toString()
          : input.url
    calls.push({ url, init })
    const pathname = url.split('?')[0] ?? url
    const body = routes[pathname]
    if (body === undefined) {
      return new Response('{}', { status: 404 })
    }
    return jsonResponse(body)
  }) as typeof fetch

// The live projections resolve on a microtask after mount; poll until the
// loaded render lands.
const waitFor = async (predicate: () => boolean): Promise<void> => {
  await vi.waitFor(() => {
    if (!predicate()) {
      throw new Error('condition not met')
    }
  })
}

describe('APP-FORUM /forum* routes (#8635)', () => {
  test('all four route components server-render the real forum surface', () => {
    const cases: ReadonlyArray<readonly [string, string]> = [
      [renderToStaticMarkup(<ForumIndexPage />), 'forum'],
      [
        renderToStaticMarkup(<ForumForumPage forumRef="product-promises" />),
        'forum-forum',
      ],
      [renderToStaticMarkup(<ForumTopicPage topicId="topic-1" />), 'forum-topic'],
      [
        renderToStaticMarkup(<ForumReceiptPage receiptRef="receipt_1" />),
        'forum-receipt',
      ],
    ]
    for (const [html, route] of cases) {
      expect(html).toContain(`data-route="${route}"`)
      expect(html).toContain('aria-label="OpenAgents Forum"')
      // The mount shim is gone: the server renders the honest loading state,
      // not an empty container.
      expect(html).toContain('Loading…')
      expect(html).toContain('href="/forum"')
    }
  })

  test('deep-link URL contract: the four legacy paths stay the registered routes', () => {
    const routesDir = join(__dirname, 'forum')
    expect(readFileSync(join(routesDir, 'index.tsx'), 'utf8')).toContain(
      "createFileRoute('/forum/')",
    )
    expect(readFileSync(join(routesDir, 'f.$forumRef.tsx'), 'utf8')).toContain(
      "createFileRoute('/forum/f/$forumRef')",
    )
    expect(readFileSync(join(routesDir, 't.$topicId.tsx'), 'utf8')).toContain(
      "createFileRoute('/forum/t/$topicId')",
    )
    expect(
      readFileSync(join(routesDir, 'receipts.$receiptRef.tsx'), 'utf8'),
    ).toContain("createFileRoute('/forum/receipts/$receiptRef')")
  })

  test('index view renders the board with real headings, links and badges', () => {
    const html = markup(readyIndexState)

    expect(html).toContain('<h1 class="forum-text forum-heading">OpenAgents Forum</h1>')
    expect(html).toContain('Product Promises')
    expect(html).toContain('href="/forum/f/product-promises"')
    expect(html).toContain('2 topics')
    expect(html).toContain('5 posts')
    // Moderation/discoverability state is visible but read-only.
    expect(html).toContain('Unlisted')
    expect(html).toContain('Locked')
    // Read-only surface: no forms and no write controls.
    expect(html).not.toContain('<form')
    expect(html).not.toContain('<input')
  })

  test('keeps Khala decoration bounded to the board and breadcrumb without changing semantic rows', () => {
    const html = markup(readyIndexState)

    expect(html.match(/data-en-khala-decoration="true"/gu)).toHaveLength(2)
    expect(html.match(/data-en-khala="cut-corner-surface"/gu)).toHaveLength(1)
    expect(html.match(/data-en-khala="signal-separator"/gu)).toHaveLength(1)
    expect(html).toContain('id="en-khala-forum-board-index"')
    expect(html).toContain('data-en-key="forum-index-panel"')
    // Every decoration is inert.
    expect(html.match(/aria-hidden="true"/gu)).toHaveLength(2)
    expect(html).toContain('Product Promises')
    expect(html).toContain('Void')
  })

  test('forum view renders the topic list with moderation state labels', () => {
    const html = markup(readyForumState)
    expect(html).toContain('A live promise report')
    expect(html).toContain('href="/forum/t/topic-1"')
    expect(html).toContain('by Raynor')
    // The locked topic keeps its lock label (write policy stays visible even
    // though this surface exposes no moderation controls).
    expect(html).toContain('Locked topic')
    expect(html).toContain('1 reply')
  })

  test('topic view renders post markdown as elements, never as raw HTML', () => {
    const html = markup(readyTopicState)
    expect(html).toContain('<strong>')
    expect(html).toContain('<code>inline code</code>')
    expect(html).toContain('<ul>')
    expect(html).toContain('<figure class="forum-code" data-forum-language="ts">')
    expect(html).toContain('const a = 1')
    // Safe same-origin links stay RELATIVE — no serving origin is baked into
    // a rendered post body. javascript: links are stripped to plain text.
    expect(html).toContain('href="/forum" rel="noopener noreferrer"')
    expect(html).not.toContain(`href="${window.location.origin}/forum"`)
    expect(html).not.toContain('javascript:alert')
    expect(html).toContain('unsafe link')
    // Post anchors keep the exact legacy anchor names for #post- deep links.
    expect(html).toContain('id="post-post-a"')
    expect(html).toContain('id="post-1"')
    // Author identity column links to the served /forum/u profile page.
    expect(html).toContain('href="/forum/u/actor-1/raynor"')
    // Sort toggle offers the legacy ?sortDir deep link.
    expect(html).toContain('href="/forum/t/topic-1?sortDir=desc"')
    expect(html).toContain('Oldest first')
    expect(html).toContain('Newest first')
    // Permalink stays a real button.
    expect(html).toContain('<button class="forum-permalink" type="button">Permalink</button>')
  })

  test('post bodies never reach the DOM as markup', () => {
    const injected: ForumPageState = {
      ...readyTopicState,
      posts: [
        {
          postId: 'post-x',
          postNumber: 1,
          subject: 'Injection attempt',
          bodyText:
            '<script>alert(1)</script> and <img src=x onerror=alert(1)> plus [x](javascript:alert(2))',
          createdAt: '2026-07-09T10:00:00Z',
          author: { displayName: 'Kerrigan' },
        },
      ],
    }
    const html = markup(injected)
    expect(html).not.toContain('<script>')
    expect(html).not.toContain('<img')
    expect(html).not.toContain('javascript:alert')
    // The literal text is still shown to the reader, fully escaped — the
    // `onerror` substring only ever appears inside escaped text, never as an
    // attribute.
    expect(html).toContain('&lt;script&gt;alert(1)&lt;/script&gt;')
    expect(html).toContain('&lt;img src=x onerror=alert(1)&gt;')

    // Same body through the real DOM: no element or handler survives.
    const host = document.createElement('div')
    host.innerHTML = html
    expect(host.querySelectorAll('script, img')).toHaveLength(0)
    for (const node of host.querySelectorAll('*')) {
      for (const attribute of node.getAttributeNames()) {
        expect(attribute.startsWith('on')).toBe(false)
      }
    }
  })

  test('receipt view preserves the immutable historical payment record', () => {
    const html = markup(readyReceiptState)
    expect(html).toContain('post tip')
    expect(html).toContain('25 sats of bitcoin')
    expect(html).toContain('receipt_1')
    expect(html).toContain('href="/forum/t/topic-1#post-post-a"')
    expect(html).toContain('agent:raynor')
  })

  test('loading and unavailable states stay honest (no fabricated content)', () => {
    expect(markup(initialForumPageState({ kind: 'index' }))).toContain('Loading…')

    const unavailable = markup({
      ...initialForumPageState({ kind: 'index' }),
      phase: 'unavailable',
      errorMessage: 'Board index unavailable',
    })
    expect(unavailable).toContain('role="alert"')
    expect(unavailable).toContain('Forum unavailable · Board index unavailable')
  })

  test('sortDir query parsing matches the legacy contract (sortDir + sd)', () => {
    expect(parseTopicPostSortDirection('')).toBe('asc')
    expect(parseTopicPostSortDirection('?sortDir=desc')).toBe('desc')
    expect(parseTopicPostSortDirection('?sortDir=ASC')).toBe('asc')
    expect(parseTopicPostSortDirection('?sd=d')).toBe('desc')
    expect(parseTopicPostSortDirection('?sd=a')).toBe('asc')
  })

  test('login return paths preserve the visited forum location', () => {
    expect(forumReturnPath({ kind: 'index' })).toBe('/forum')
    expect(forumReturnPath({ kind: 'forum', forumRef: 'product-promises' })).toBe(
      '/forum/f/product-promises',
    )
    expect(
      forumReturnPath({ kind: 'topic', topicId: 'topic-1', sortDirection: 'asc' }),
    ).toBe('/forum/t/topic-1')
    expect(forumReturnPath({ kind: 'receipt', receiptRef: 'receipt_1' })).toBe(
      '/forum/receipts/receipt_1',
    )
  })

  test('DOM mount smoke: index renders live projections into a real container', async () => {
    const calls: FetchCall[] = []
    const fetchStub = makeFetchStub(
      {
        '/api/forum': { forums: FORUMS },
      },
      calls,
    )

    const container = document.createElement('div')
    document.body.appendChild(container)
    let surface!: Readonly<{ unmount: () => void }>
    try {
      await act(async () => {
        surface = mountForumSurface(
          container,
          { kind: 'index' },
          { fetchFn: fetchStub },
        )
      })
      await waitFor(() =>
        (container.textContent ?? '').includes('Product Promises'),
      )
      // The board-assembly animation is still wired to the committed board
      // decoration. This host has no Web Animations, so it takes the
      // zero-work stable path — but it does run.
      await waitFor(
        () =>
          container
            .querySelector('#en-khala-forum-board-index')
            ?.getAttribute('data-khala-motion') === 'unsupported-static',
      )
      expect(container.textContent).toContain('OpenAgents Forum')
      expect(container.querySelector('[data-route="forum"]')).not.toBeNull()
      expect(container.querySelectorAll('[data-en-khala-decoration]')).toHaveLength(2)
      expect(
        container.querySelectorAll('[data-en-khala="cut-corner-surface"]'),
      ).toHaveLength(1)
      expect(
        container.querySelectorAll('[data-en-khala="signal-separator"]'),
      ).toHaveLength(1)
      expect(
        container.querySelectorAll('[data-en-key^="forum-row-"] [data-en-khala-decoration]'),
      ).toHaveLength(0)
      const undecorated = container.cloneNode(true) as HTMLElement
      undecorated
        .querySelectorAll('[data-en-khala-decoration]')
        .forEach((node) => node.remove())
      expect(undecorated.textContent).toContain('OpenAgents Forum')
      expect(undecorated.textContent).toContain('Product Promises')
      // Forum links are real anchors with real hrefs (crawlable deep links).
      const link = container.querySelector(
        '[data-en-key="forum-row-product-promises"] a',
      )
      expect(link?.getAttribute('href')).toBe('/forum/f/product-promises')
      expect(calls.map((call) => call.url.split('?')[0])).toEqual(['/api/forum'])
    } finally {
      await act(async () => {
        surface.unmount()
      })
      container.remove()
    }
  })

  test('DOM mount smoke: unavailable board renders the honest failure banner', async () => {
    const failingFetch = (async () => {
      throw new Error('offline')
    }) as unknown as typeof fetch

    const container = document.createElement('div')
    document.body.appendChild(container)
    let surface!: Readonly<{ unmount: () => void }>
    try {
      await act(async () => {
        surface = mountForumSurface(
          container,
          { kind: 'index' },
          { fetchFn: failingFetch },
        )
      })
      await waitFor(() =>
        (container.textContent ?? '').includes('Forum unavailable'),
      )
    } finally {
      await act(async () => {
        surface.unmount()
      })
      container.remove()
    }
  })

  test('deep-link anchor: #post-<id> scrolls to the rendered post on mount', async () => {
    const calls: FetchCall[] = []
    const fetchStub = makeFetchStub(
      {
        '/api/forum/topics/topic-1': { topic: TOPIC, posts: POSTS },
        '/api/auth/session': { authenticated: false },
      },
      calls,
    )

    window.location.hash = '#post-post-b'
    const scrolled: string[] = []
    const container = document.createElement('div')
    document.body.appendChild(container)
    let surface!: Readonly<{ unmount: () => void }>
    try {
      await act(async () => {
        surface = mountForumSurface(
          container,
          { kind: 'topic', topicId: 'topic-1', sortDirection: 'asc' },
          {
            fetchFn: fetchStub,
            scrollToAnchor: (anchor) => {
              scrolled.push(anchor)
            },
          },
        )
      })
      await waitFor(() => scrolled.length > 0)
      expect(scrolled).toEqual(['post-post-b'])
      // The anchor target exists in the rendered DOM.
      expect(container.querySelector('[id="post-post-b"]')).not.toBeNull()
    } finally {
      await act(async () => {
        surface.unmount()
      })
      container.remove()
      window.location.hash = ''
    }
  })

  test('source boundary: the page is plain React with no framework renderer', () => {
    const source = readFileSync(join(__dirname, '-forum-page.tsx'), 'utf8')
    const markdownSource = readFileSync(
      join(__dirname, '-forum-markdown.ts'),
      'utf8',
    )
    expect(source).not.toContain('@effect-native')
    expect(markdownSource).not.toContain('@effect-native')
    // No arbitrary HTML path exists on this surface, and the general-purpose
    // markdown dependency is deliberately not reachable from it.
    expect(source).not.toContain('dangerouslySetInnerHTML')
    expect(markdownSource).not.toContain('dangerouslySetInnerHTML')
    expect(source).not.toContain("from 'marked'")
    expect(markdownSource).not.toContain("from 'marked'")
    // React stays free of UI component libraries and legacy imports here.
    expect(source).not.toContain('lucide-react')
    expect(source).not.toContain('foldkit')
  })
})
