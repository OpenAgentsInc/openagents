// APP-FORUM (#8635) — write-policy, auth, and exact-tip receipt coverage for
// the retained /forum* Effect Native routes.
//
// The conversion is presentation-only: the Worker keeps every Forum write,
// moderation, lock, idempotency, identity, and tipping authority. These tests
// pin the browser surface to that boundary — the ONLY mutation it may issue is
// the legacy tip-ladder POST with the legacy idempotency-key shape.

import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test, vi } from 'vitest'

import { Effect, Exit, Scope } from '@effect-native/core/effect'

import {
  defaultPostRewardSats,
  forumTipReceiptStateLabel,
  forumTipUiProjectionForPost,
  makeForumTipIdempotencyKeyFactory,
  normalizeTipAmount,
  postTipStatsBadge,
  type ForumLaunchStatusProjection,
  type ForumPostProjection,
  type ForumTopicProjection,
} from './-forum-data'
import { mountForumSurface } from './-forum-page'

const TOPIC: ForumTopicProjection = {
  topicId: 'topic-1',
  forumId: 'product-promises',
  title: 'A live promise report',
  postCount: 1,
  author: { displayName: 'Raynor' },
}

const TIPPABLE_POST: ForumPostProjection = {
  postId: 'post-a',
  postNumber: 1,
  subject: 'First post',
  bodyText: 'Hello world.',
  createdAt: '2026-07-09T10:00:00Z',
  author: { actorId: 'actor-1', slug: 'raynor', displayName: 'Raynor' },
  tipRecipientReadiness: { tippingAvailable: true },
}

const LAUNCH_READY: ForumLaunchStatusProjection = {
  publicTipping: { postTips: 'ready', remainingBeforeLiveTips: [] },
}

const LAUNCH_GATED: ForumLaunchStatusProjection = {
  publicTipping: {
    postTips: 'pending',
    remainingBeforeLiveTips: ['live tip smoke pending'],
  },
}

const jsonResponse = (body: unknown, status = 200): Response =>
  new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })

type FetchCall = Readonly<{
  url: string
  method: string
  headers: Headers
  body: string | null
}>

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
    calls.push({
      url,
      method: init?.method ?? 'GET',
      headers: new Headers(init?.headers),
      body: typeof init?.body === 'string' ? init.body : null,
    })
    const pathname = url.split('?')[0] ?? url
    const body = routes[pathname]
    if (body === undefined) {
      return new Response('{}', { status: 404 })
    }
    return jsonResponse(body)
  }) as typeof fetch

const mountTopic = async (options: {
  fetchFn: typeof fetch
  container: HTMLElement
  scope: Scope.Scope
}) =>
  Effect.runPromise(
    Scope.provide(options.scope)(
      mountForumSurface(
        options.container,
        { kind: 'topic', topicId: 'topic-1', sortDirection: 'asc' },
        { fetchFn: options.fetchFn, sessionNonce: 'nonce-1' },
      ),
    ),
  )

const waitFor = async (predicate: () => boolean): Promise<void> => {
  await vi.waitFor(() => {
    if (!predicate()) {
      throw new Error('condition not met')
    }
  })
}

describe('APP-FORUM write policy + tipping (#8635)', () => {
  test('tip gating projection matches the legacy contract exactly', () => {
    const gated = forumTipUiProjectionForPost({
      authMode: 'LoggedIn',
      launchStatus: LAUNCH_GATED,
      post: TIPPABLE_POST,
    })
    expect(gated.reason).toBe('launch_gated')
    expect(gated.buttonVisible).toBe(false)
    expect(gated.statusLabel).toBe('Live smoke pending')
    expect(gated.detail).toBe('live tip smoke pending')

    const walletPending = forumTipUiProjectionForPost({
      authMode: 'LoggedIn',
      launchStatus: LAUNCH_READY,
      post: {
        ...TIPPABLE_POST,
        tipRecipientReadiness: {
          tippingAvailable: false,
          blockerRef: 'recipient wallet pending',
        },
      },
    })
    expect(walletPending.reason).toBe('recipient_not_ready')
    expect(walletPending.statusLabel).toBe('Wallet pending')
    expect(walletPending.detail).toBe('recipient wallet pending')

    const loginRequired = forumTipUiProjectionForPost({
      authMode: 'LoggedOut',
      launchStatus: LAUNCH_READY,
      post: TIPPABLE_POST,
    })
    expect(loginRequired.reason).toBe('login_required')
    expect(loginRequired.authRequired).toBe(true)
    expect(loginRequired.statusLabel).toBe('Log in required')

    const ready = forumTipUiProjectionForPost({
      authMode: 'LoggedIn',
      launchStatus: LAUNCH_READY,
      post: TIPPABLE_POST,
    })
    expect(ready.reason).toBe('ready')
    expect(ready.buttonVisible).toBe(true)
    expect(ready.buttonLabel).toBe('Tip')
    expect(ready.caveat).toBe(
      'Content reward; receipt separates payment from settlement.',
    )
  })

  test('receipt state labels keep the exact paid-versus-settled vocabulary', () => {
    expect(forumTipReceiptStateLabel('paid')).toBe('Payment recorded')
    expect(forumTipReceiptStateLabel('settled')).toBe('Recipient wallet paid')
    expect(forumTipReceiptStateLabel('recipient_pending')).toBe(
      'Creator settlement pending',
    )
    expect(forumTipReceiptStateLabel('dispatched')).toBe('Payout dispatched')
    expect(forumTipReceiptStateLabel('evidence_only')).toBe('Receipt evidence only')
    expect(forumTipReceiptStateLabel('failed')).toBe('Payment failed')
    expect(forumTipReceiptStateLabel('payment_required')).toBe('Payment required')
    expect(forumTipReceiptStateLabel('previewed')).toBe('Previewed')
    expect(forumTipReceiptStateLabel('refunded')).toBe('Refunded')
    expect(forumTipReceiptStateLabel('reversed')).toBe('Reversed')
    expect(forumTipReceiptStateLabel('unknown-state')).toBe('Payment state')
  })

  test('post tip badge separates paid from settled and never conflates them', () => {
    expect(
      postTipStatsBadge({
        postId: 'post-a',
        tipStats: { totalPaidSats: 21, totalSettledSats: 21, tipCount: 2 },
      }),
    ).toMatchObject({
      settlement: 'settled',
      label: '21 sats ✓',
      detail: '21 sats paid · 21 sats settled · 2 payments',
    })
    expect(
      postTipStatsBadge({
        postId: 'post-a',
        tipStats: { totalPaidSats: 21, totalSettledSats: 0, tipCount: 1 },
      }),
    ).toMatchObject({
      settlement: 'pending',
      label: '21 sats ◷',
      detail: '21 sats paid · 0 sats settled · settlement pending',
    })
    expect(postTipStatsBadge({ postId: 'post-a', tipStats: null })).toBeNull()
  })

  test('idempotency keys keep the exact legacy browser-tip shape and never repeat', () => {
    const keyFor = makeForumTipIdempotencyKeyFactory('nonce-1')
    expect(keyFor('post-a', 10)).toBe('forum:browser_tip:post-a:10:nonce-1:1')
    expect(keyFor('post-a', 10)).toBe('forum:browser_tip:post-a:10:nonce-1:2')
    expect(keyFor('post-b', 25)).toBe('forum:browser_tip:post-b:25:nonce-1:3')
  })

  test('tip amounts sanitize like the legacy page: positive integer sats only', () => {
    expect(normalizeTipAmount(25)).toBe(25)
    expect(normalizeTipAmount('42')).toBe(42)
    expect(normalizeTipAmount(9.9)).toBe(9)
    expect(normalizeTipAmount(0)).toBe(defaultPostRewardSats)
    expect(normalizeTipAmount(-5)).toBe(defaultPostRewardSats)
    expect(normalizeTipAmount('nope')).toBe(defaultPostRewardSats)
  })

  test('write policy: the data layer can only issue the tip-ladder POST', () => {
    const dataSource = readFileSync(join(__dirname, '-forum-data.ts'), 'utf8')
    const pageSource = readFileSync(join(__dirname, '-forum-page.tsx'), 'utf8')
    const combined = dataSource + pageSource

    // Exactly one mutating call site, and it is the tips ladder.
    expect(dataSource.match(/method: 'POST'/g)).toHaveLength(1)
    expect(dataSource).toContain('/tips/ladder')
    expect(dataSource).toContain("'Idempotency-Key'")

    // No content-write, moderation, or work-request authority in the browser.
    for (const forbidden of [
      "method: 'PATCH'",
      "method: 'DELETE'",
      "method: 'PUT'",
      '/api/forum/moderation',
      '/api/forum/work-requests',
      '/api/forum/paid-actions',
      '/api/forum/tip-recipient-wallets',
      '/watches',
      '/bookmarks',
      '/reports',
      '/follows',
    ]) {
      expect(combined).not.toContain(forbidden)
    }
  })

  test('logged-out tip click never POSTs; it offers the GitHub login deep link', async () => {
    const calls: FetchCall[] = []
    const fetchStub = makeFetchStub(
      {
        '/api/forum/topics/topic-1': { topic: TOPIC, posts: [TIPPABLE_POST] },
        '/api/forum/launch-status': LAUNCH_READY,
        '/api/auth/session': { authenticated: false },
      },
      calls,
    )

    const container = document.createElement('div')
    document.body.appendChild(container)
    const scope = await Effect.runPromise(Scope.make())
    try {
      await mountTopic({ fetchFn: fetchStub, container, scope })
      await waitFor(
        () =>
          container.querySelector('[data-en-key="post-post-a-tip-send"]') !==
          null,
      )
      const button = container.querySelector(
        '[data-en-key="post-post-a-tip-send"]',
      )
      ;(button as HTMLElement).click()

      await waitFor(() =>
        (container.textContent ?? '').includes('Log in with GitHub to tip'),
      )
      const loginLink = container.querySelector(
        '[data-en-key="post-post-a-tip-login"]',
      )
      expect(loginLink?.getAttribute('href')).toBe(
        '/login/github?returnTo=%2Fforum%2Ft%2Ftopic-1',
      )
      // Auth boundary: no tip POST was issued.
      expect(calls.filter((call) => call.method === 'POST')).toHaveLength(0)
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      container.remove()
    }
  })

  test('logged-in tip posts the exact ladder contract and links the receipt', async () => {
    const calls: FetchCall[] = []
    const fetchStub = makeFetchStub(
      {
        '/api/forum/topics/topic-1': { topic: TOPIC, posts: [TIPPABLE_POST] },
        '/api/forum/launch-status': LAUNCH_READY,
        '/api/auth/session': { authenticated: true },
        '/api/forum/posts/post-a/tips/ladder': { receiptRef: 'receipt_1' },
      },
      calls,
    )

    const container = document.createElement('div')
    document.body.appendChild(container)
    const scope = await Effect.runPromise(Scope.make())
    try {
      await mountTopic({ fetchFn: fetchStub, container, scope })
      await waitFor(
        () =>
          container.querySelector('[data-en-key="post-post-a-tip-send"]') !==
          null,
      )
      const button = container.querySelector(
        '[data-en-key="post-post-a-tip-send"]',
      )
      ;(button as HTMLElement).click()

      await waitFor(() =>
        (container.textContent ?? '').includes('Payment recorded'),
      )

      const tipCalls = calls.filter((call) => call.method === 'POST')
      expect(tipCalls).toHaveLength(1)
      const tipCall = tipCalls[0]!
      expect(tipCall.url).toBe('/api/forum/posts/post-a/tips/ladder')
      expect(tipCall.body).toBe(JSON.stringify({ amountSat: 10 }))
      expect(tipCall.headers.get('Idempotency-Key')).toBe(
        'forum:browser_tip:post-a:10:nonce-1:1',
      )
      expect(tipCall.headers.get('Content-Type')).toBe('application/json')

      // The success panel links the exact receipt deep link.
      const receiptLink = container.querySelector(
        '[data-en-key="post-post-a-tip-receipt"]',
      )
      expect(receiptLink?.getAttribute('href')).toBe('/forum/receipts/receipt_1')
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      container.remove()
    }
  })

  test('failed tip surfaces the honest failure message without fabricating a receipt', async () => {
    const calls: FetchCall[] = []
    const fetchStub = (async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = typeof input === 'string' ? input : String(input)
      calls.push({
        url,
        method: init?.method ?? 'GET',
        headers: new Headers(init?.headers),
        body: typeof init?.body === 'string' ? init.body : null,
      })
      const pathname = url.split('?')[0] ?? url
      if (pathname === '/api/forum/topics/topic-1') {
        return jsonResponse({ topic: TOPIC, posts: [TIPPABLE_POST] })
      }
      if (pathname === '/api/forum/launch-status') {
        return jsonResponse(LAUNCH_READY)
      }
      if (pathname === '/api/auth/session') {
        return jsonResponse({ authenticated: true })
      }
      if (pathname === '/api/forum/posts/post-a/tips/ladder') {
        return jsonResponse({ reason: 'payment rail unavailable' }, 502)
      }
      return new Response('{}', { status: 404 })
    }) as typeof fetch

    const container = document.createElement('div')
    document.body.appendChild(container)
    const scope = await Effect.runPromise(Scope.make())
    try {
      await mountTopic({ fetchFn: fetchStub, container, scope })
      await waitFor(
        () =>
          container.querySelector('[data-en-key="post-post-a-tip-send"]') !==
          null,
      )
      const button = container.querySelector(
        '[data-en-key="post-post-a-tip-send"]',
      )
      ;(button as HTMLElement).click()

      await waitFor(() =>
        (container.textContent ?? '').includes(
          'Payment failed · payment rail unavailable',
        ),
      )
      expect(container.textContent).not.toContain('Payment recorded')
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      container.remove()
    }
  })

  test('launch-gated topics keep the tip button hidden end to end', async () => {
    const fetchStub = makeFetchStub(
      {
        '/api/forum/topics/topic-1': { topic: TOPIC, posts: [TIPPABLE_POST] },
        '/api/forum/launch-status': LAUNCH_GATED,
        '/api/auth/session': { authenticated: true },
      },
      [],
    )

    const container = document.createElement('div')
    document.body.appendChild(container)
    const scope = await Effect.runPromise(Scope.make())
    try {
      await mountTopic({ fetchFn: fetchStub, container, scope })
      await waitFor(() =>
        (container.textContent ?? '').includes('Live smoke pending'),
      )
      expect(
        container.querySelector('[data-en-key="post-post-a-tip-send"]'),
      ).toBeNull()
    } finally {
      await Effect.runPromise(Scope.close(scope, Exit.void))
      container.remove()
    }
  })
})
