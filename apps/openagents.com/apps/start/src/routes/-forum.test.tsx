// APP-FORUM (#8635) — route, view, auth, moderation-visibility, accessibility,
// and deep-link coverage for the retained /forum* Effect Native routes.

import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test, vi } from 'vitest'

import { Effect, Exit, Scope } from '@effect-native/core/effect'

import {
  parseTopicPostSortDirection,
  type ForumLaunchStatusProjection,
  type ForumPostProjection,
  type ForumReceiptProjection,
  type ForumSummaryProjection,
  type ForumTopicProjection,
} from './-forum-data'
import {
  ForumIndexPage,
  ForumReceiptPage,
  forumPageView,
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
    tipStats: { totalPaidSats: 21, totalSettledSats: 21, tipCount: 2 },
    tipRecipientReadiness: { tippingAvailable: true },
  },
  {
    postId: 'post-b',
    postNumber: 2,
    bodyText: 'A reply with an [unsafe link](javascript:alert(1)).',
    createdAt: '2026-07-09T11:00:00Z',
    author: { displayName: 'Zeratul' },
    tipRecipientReadiness: {
      tippingAvailable: false,
      blockerRef: 'recipient wallet pending',
    },
  },
]

const LAUNCH_READY: ForumLaunchStatusProjection = {
  publicTipping: { postTips: 'ready', remainingBeforeLiveTips: [] },
}

const RECEIPT: ForumReceiptProjection = {
  receiptRef: 'receipt_1',
  actionKind: 'post_tip',
  createdAt: '2026-07-10T11:00:00Z',
  amount: { amount: 25, asset: 'sats' },
  target: { topicId: 'topic-1', postId: 'post-a' },
  targetPostPermalink: '/forum/t/topic-1#post-post-a',
  recipientActorRef: 'agent:raynor',
  tipSettlement: {
    state: 'settled',
    wording: { publicPage: 'Recipient wallet paid.' },
    creatorReceivedSpendableValue: true,
  },
}

const readyIndexState: ForumPageState = {
  ...initialForumPageState({ kind: 'index' }),
  phase: 'ready',
  forums: FORUMS,
  tipLeaderboards: {
    posts: [
      {
        postPermalink: '/forum/t/topic-1#post-post-a',
        postTitle: 'First post',
        totalPaidSats: 21,
        totalSettledSats: 21,
        tipCount: 2,
      },
    ],
    creators: [
      {
        actor: { actorId: 'actor-1', slug: 'raynor', displayName: 'Raynor' },
        totalPaidSats: 21,
        totalSettledSats: 10,
        tipCount: 2,
      },
    ],
  },
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
  launchStatus: LAUNCH_READY,
  authMode: 'LoggedIn',
}

const readyReceiptState: ForumPageState = {
  ...initialForumPageState({ kind: 'receipt', receiptRef: 'receipt_1' }),
  phase: 'ready',
  receipt: RECEIPT,
}

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

// The DOM renderer applies view-stream emissions on its own fiber; poll until
// the hydrated render lands.
const waitFor = async (predicate: () => boolean): Promise<void> => {
  await vi.waitFor(() => {
    if (!predicate()) {
      throw new Error('condition not met')
    }
  })
}

describe('APP-FORUM Effect Native routes (#8635)', () => {
  test('React route shells are thin mount shims, never forum-content React', () => {
    const indexHtml = renderToStaticMarkup(<ForumIndexPage />)
    expect(indexHtml).toContain('data-route="forum"')
    expect(indexHtml).toContain('data-forum-en-root=""')
    expect(indexHtml).toContain('aria-label="OpenAgents Forum"')
    expect(indexHtml).not.toContain('OpenAgents Forum</') // content lives in the EN tree

    const receiptHtml = renderToStaticMarkup(
      <ForumReceiptPage receiptRef="receipt_1" />,
    )
    expect(receiptHtml).toContain('data-route="forum-receipt"')
    expect(receiptHtml).not.toContain('Forum receipt')
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

  test('index view is one typed Effect Native tree from the catalog', () => {
    const tree = forumPageView(readyIndexState, NOW)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({ tag: 'Stack', key: 'forum-root' })
    expect(serialized).toContain('"catalogVersion":"effect-native/v30"')
    for (const tag of ['Stack', 'Card', 'Link', 'Text', 'Badge']) {
      expect(serialized).toContain(`"_tag":"${tag}"`)
    }
    expect(serialized).toContain('OpenAgents Forum')
    expect(serialized).toContain('Product Promises')
    expect(serialized).toContain('"path":"/forum/f/product-promises"')
    expect(serialized).toContain('2 topics')
    expect(serialized).toContain('5 posts')
    // Moderation/discoverability state is visible but read-only.
    expect(serialized).toContain('"label":"Unlisted"')
    expect(serialized).toContain('"label":"Locked"')
    // Tip leaderboards render with exact paid-versus-settled wording.
    expect(serialized).toContain('Top tipped posts')
    expect(serialized).toContain('21 paid sats · 10 settled sats · 2 tips')
    // Adapter rule: no React class names inside the typed tree.
    expect(serialized).not.toContain('className')
  })

  test('forum view renders the topic list with moderation state labels', () => {
    const serialized = JSON.stringify(forumPageView(readyForumState, NOW))
    expect(serialized).toContain('A live promise report')
    expect(serialized).toContain('"path":"/forum/t/topic-1"')
    expect(serialized).toContain('by Raynor')
    // The locked topic keeps its lock label (write policy stays visible even
    // though this surface exposes no moderation controls).
    expect(serialized).toContain('Locked topic')
    expect(serialized).toContain('1 reply')
  })

  test('topic view renders markdown through the catalog Markdown/CodeBlock components', () => {
    const serialized = JSON.stringify(forumPageView(readyTopicState, NOW))
    expect(serialized).toContain('"_tag":"Markdown"')
    expect(serialized).toContain('"_tag":"CodeBlock"')
    expect(serialized).toContain('"kind":"strong"')
    expect(serialized).toContain('"kind":"code"')
    expect(serialized).toContain('const a = 1')
    // Safe same-origin links stay RELATIVE in the tree — effect-native v28
    // (issue #71) admits rooted paths on markdown link hrefs, so the old
    // EN-2 origin-resolution workaround is gone and no serving origin is
    // baked into the view. javascript: links are stripped to plain text.
    expect(serialized).toContain('"href":"/forum"')
    expect(serialized).not.toContain(`"href":"${window.location.origin}/forum"`)
    expect(serialized).not.toContain('javascript:alert')
    // Post anchors keep the exact legacy anchor names for #post- deep links.
    expect(serialized).toContain('"key":"post-post-a"')
    expect(serialized).toContain('"key":"post-1"')
    // Author identity column links to the served /forum/u profile page.
    expect(serialized).toContain('"path":"/forum/u/actor-1/raynor"')
    // Sort toggle offers the legacy ?sortDir deep link.
    expect(serialized).toContain('"path":"/forum/t/topic-1?sortDir=desc"')
    expect(serialized).toContain('Oldest first')
    expect(serialized).toContain('Newest first')
  })

  test('receipt view renders the exact payment/settlement wording', () => {
    const serialized = JSON.stringify(forumPageView(readyReceiptState, NOW))
    expect(serialized).toContain('post tip')
    expect(serialized).toContain('25 sats of bitcoin')
    expect(serialized).toContain('Recipient wallet paid')
    expect(serialized).toContain('Recipient wallet payment confirmed')
    expect(serialized).toContain('receipt_1')
    expect(serialized).toContain('"path":"/forum/t/topic-1#post-post-a"')
    expect(serialized).toContain('agent:raynor')
  })

  test('loading and unavailable states stay honest (no fabricated content)', () => {
    const loading = JSON.stringify(
      forumPageView(initialForumPageState({ kind: 'index' }), NOW),
    )
    expect(loading).toContain('Loading…')

    const unavailable = JSON.stringify(
      forumPageView(
        {
          ...initialForumPageState({ kind: 'index' }),
          phase: 'unavailable',
          errorMessage: 'Board index unavailable',
        },
        NOW,
      ),
    )
    expect(unavailable).toContain('"_tag":"StatusBanner"')
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

  test('DOM mount smoke: index renders live projections through the DOM renderer', async () => {
    const calls: FetchCall[] = []
    const fetchStub = makeFetchStub(
      {
        '/api/forum': { forums: FORUMS },
        '/api/forum/tip-leaderboards': readyIndexState.tipLeaderboards,
      },
      calls,
    )

    const container = document.createElement('div')
    document.body.appendChild(container)
    const scope = await Effect.runPromise(Scope.make())
    try {
      await Effect.runPromise(
        Scope.provide(scope)(
          mountForumSurface(container, { kind: 'index' }, { fetchFn: fetchStub }),
        ),
      )
      await waitFor(() =>
        (container.textContent ?? '').includes('Product Promises'),
      )
      expect(container.textContent).toContain('OpenAgents Forum')
      expect(container.querySelector('[data-en-key="forum-root"]')).not.toBeNull()
      // Forum links are real anchors with real hrefs (crawlable deep links).
      const link = container.querySelector(
        '[data-en-key="forum-row-product-promises-title"]',
      )
      expect(link?.getAttribute('href')).toBe('/forum/f/product-promises')
      expect(calls.map((call) => call.url.split('?')[0])).toEqual(
        expect.arrayContaining(['/api/forum', '/api/forum/tip-leaderboards']),
      )
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      container.remove()
    }
  })

  test('DOM mount smoke: unavailable board renders the honest failure banner', async () => {
    const failingFetch = (async () => {
      throw new Error('offline')
    }) as unknown as typeof fetch

    const container = document.createElement('div')
    document.body.appendChild(container)
    const scope = await Effect.runPromise(Scope.make())
    try {
      await Effect.runPromise(
        Scope.provide(scope)(
          mountForumSurface(container, { kind: 'index' }, { fetchFn: failingFetch }),
        ),
      )
      await waitFor(() =>
        (container.textContent ?? '').includes('Forum unavailable'),
      )
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      container.remove()
    }
  })

  test('deep-link anchor: #post-<id> scrolls to the rendered post on mount', async () => {
    const calls: FetchCall[] = []
    const fetchStub = makeFetchStub(
      {
        '/api/forum/topics/topic-1': { topic: TOPIC, posts: POSTS },
        '/api/forum/launch-status': LAUNCH_READY,
        '/api/auth/session': { authenticated: false },
      },
      calls,
    )

    window.location.hash = '#post-post-b'
    const scrolled: string[] = []
    const container = document.createElement('div')
    document.body.appendChild(container)
    const scope = await Effect.runPromise(Scope.make())
    try {
      await Effect.runPromise(
        Scope.provide(scope)(
          mountForumSurface(
            container,
            { kind: 'topic', topicId: 'topic-1', sortDirection: 'asc' },
            {
              fetchFn: fetchStub,
              scrollToAnchor: (anchor) => {
                scrolled.push(anchor)
              },
            },
          ),
        ),
      )
      expect(scrolled).toEqual(['post-post-b'])
      // The anchor target exists in the rendered DOM.
      await waitFor(
        () => container.querySelector('[data-en-key="post-post-b"]') !== null,
      )
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      container.remove()
      window.location.hash = ''
    }
  })

  test('source boundary: the page is authored from the vendored EN packages only', () => {
    const source = readFileSync(join(__dirname, '-forum-page.tsx'), 'utf8')
    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).toContain("from '@effect-native/tokens'")
    expect(source).toContain('makeDomRenderer({ theme: khalaTheme })')
    // React stays a thin host: no UI component libraries, no legacy imports.
    expect(source).not.toContain('lucide-react')
    expect(source).not.toContain('@/components')
    expect(source).not.toContain('foldkit')
  })
})
