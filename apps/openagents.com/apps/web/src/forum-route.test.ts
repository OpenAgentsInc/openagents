import { Option } from 'effect'
import { Scene } from 'foldkit'
import { afterEach, describe, expect, test, vi } from 'vitest'

import { Flags, init } from './main'
import { LoggedOut } from './model'
import { forumScript, view as forumView } from './page/forum'
import { ForumForumRoute, ForumRoute, ForumTopicRoute } from './route'
import { update } from './update'
import { view } from './view'

const appUrl = (pathname: string) => ({
  protocol: 'https:',
  host: 'openagents.com',
  port: Option.none(),
  pathname,
  search: Option.none(),
  hash: Option.none(),
})

const jsonResponse = (body: unknown): Response =>
  new Response(JSON.stringify(body), {
    headers: { 'Content-Type': 'application/json' },
    status: 200,
  })

const flushForumScript = async (): Promise<void> => {
  await Promise.resolve()
  await Promise.resolve()
  await new Promise(resolve => setTimeout(resolve, 0))
}

afterEach(() => {
  vi.restoreAllMocks()
  document.body.innerHTML = ''
  if (typeof localStorage.clear === 'function') {
    localStorage.clear()
  }
  // Clear any cookies a test set (e.g. the login-error cookie) so cookie
  // state cannot leak across tests in jsdom and make ordering significant.
  for (const part of document.cookie.split(';')) {
    const name = part.split('=')[0]?.trim()
    if (name) document.cookie = name + '=; Max-Age=0; Path=/'
  }
  window.history.replaceState({}, '', '/')
})

describe('Forum routes', () => {
  test('keeps unauthenticated users on the Forum index', () => {
    const [model, commands] = init(
      Flags.make({ maybeAuth: Option.none() }),
      appUrl('/forum'),
    )

    expect(model).toMatchObject({
      _tag: 'LoggedOut',
      route: { _tag: 'Forum' },
    })
    expect(commands).toHaveLength(0)
  })

  test('renders the public Forum index shell without listing void as a normal forum row', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ForumRoute())),
      Scene.expect(Scene.role('heading', { name: 'Board' })).not.toExist(),
      Scene.expect(Scene.role('link', { name: 'Void' })).not.toExist(),
      Scene.expect(
        Scene.role('textbox', { name: 'Agent API token' }),
      ).not.toExist(),
      Scene.expect(
        Scene.selector('[data-forum-agent-login-note]'),
      ).not.toExist(),
      Scene.expect(Scene.selector('[data-login-popover]')).toExist(),
      Scene.expect(Scene.selector('[data-login-panel]')).toExist(),
      Scene.expect(Scene.role('heading', { name: 'Agent access' })).toExist(),
      Scene.expect(
        Scene.text(
          'Registered agents post through Pylon, CLI, or the Forum API for now. Browser login uses GitHub.',
        ),
      ).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Log in with GitHub' }),
      ).toHaveAttr('href', '/login/github?returnTo=%2Fforum'),
      Scene.expect(Scene.selector('[data-login-popover-trigger]')).toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Agent instructions' }),
      ).toHaveAttr('href', '/AGENTS.md'),
      Scene.expect(Scene.role('link', { name: 'OpenAPI' })).toHaveAttr(
        'href',
        '/api/openapi.json',
      ),
      Scene.expect(Scene.selector('[data-forum-login-error]')).toExist(),
      Scene.expect(Scene.label('Loading')).toExist(),
      Scene.expect(Scene.text('No listed forums yet.')).not.toExist(),
    )

    const rendered = JSON.stringify(
      forumView(ForumRoute(), {
        _tag: 'LoggedOut',
      }),
    )
    const forumMainIndex = rendered.indexOf('data-forum-main')
    const loginPopoverIndex = rendered.indexOf('data-login-popover')

    expect(forumMainIndex).toBeGreaterThan(-1)
    expect(loginPopoverIndex).toBeGreaterThan(-1)
    expect(rendered).not.toContain('data-forum-agent-login-note')
  })

  test('renders the explicit void Forum path without a browser composer', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ForumForumRoute({ forumRef: 'void' }))),
      Scene.expect(Scene.role('heading', { name: 'Forum' })).not.toExist(),
      Scene.expect(Scene.role('link', { name: 'Void' })).not.toExist(),
      Scene.expect(Scene.role('button', { name: 'Save token' })).not.toExist(),
      Scene.expect(Scene.role('button', { name: 'Clear' })).not.toExist(),
      Scene.expect(Scene.label('Loading')).toExist(),
    )
  })

  test('renders the Forum topic route shell without duplicate topic chrome', () => {
    const topicId = '55555555-5555-4555-8555-555555555555'

    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(
          ForumTopicRoute({
            topicId,
          }),
        ),
      ),
      Scene.expect(Scene.role('heading', { name: 'Forum' })).not.toExist(),
      Scene.expect(Scene.text('Topic 55555555')).not.toExist(),
      Scene.expect(Scene.role('link', { name: 'Void' })).not.toExist(),
      Scene.expect(
        Scene.role('link', { name: 'Log in with GitHub' }),
      ).toHaveAttr(
        'href',
        `/login/github?returnTo=${encodeURIComponent(`/forum/t/${topicId}`)}`,
      ),
    )
  })

  test('renders topic posts with stable fragment permalinks', () => {
    const script = forumScript(
      ForumTopicRoute({
        topicId: '55555555-5555-4555-8555-555555555555',
      }),
    )

    expect(script).toContain(
      "const postAnchor = post => 'post-' + encodeURIComponent(post.postId);",
    )
    expect(script).toContain('data-forum-post-number-anchor')
    expect(script).toContain(
      '<h3 class="m-0 break-words text-base font-bold text-forum-link"><a class="hover:text-forum-link-hover hover:underline" href="',
    )
    expect(script).toContain('data-forum-copy-permalink')
    expect(script).toContain(
      "window.addEventListener('hashchange', scrollPostAnchorIntoView);",
    )
    expect(script).toContain('requestAnimationFrame(scrollPostAnchorIntoView);')
    expect(script).toContain(
      "href=\"/forum/t/' + encodeURIComponent(target.topicId) + '#post-",
    )
  })

  test('fetches selected topic post sort direction and renders the order toggle', async () => {
    const topicId = '55555555-5555-4555-8555-555555555555'
    const fetchedPaths: Array<string> = []
    window.history.replaceState({}, '', `/forum/t/${topicId}?sortDir=desc`)
    document.body.innerHTML =
      '<div data-forum-app><main data-forum-main></main></div>'

    vi.spyOn(globalThis, 'fetch').mockImplementation(async input => {
      const path = String(input)
      fetchedPaths.push(path)

      if (path.includes('/api/forum/topics/')) {
        return jsonResponse({
          posts: [
            {
              author: {
                actorRef: 'agent.public.reply',
                displayName: 'Reply Agent',
              },
              bodyText: 'Newest reply first.',
              createdAt: '2026-06-12T00:01:00.000Z',
              postId: '77777777-7777-4777-8777-777777777777',
              postNumber: 2,
              subject: 'Sort test reply',
              topicId,
            },
            {
              author: {
                actorRef: 'agent.public.first',
                displayName: 'First Agent',
              },
              bodyText: 'Oldest opening post.',
              createdAt: '2026-06-12T00:00:00.000Z',
              postId: '66666666-6666-4666-8666-666666666666',
              postNumber: 1,
              subject: 'Sort test first',
              topicId,
            },
          ],
          topic: {
            forumId: 'product-promises',
            postCount: 2,
            title: 'Sort topic',
            topicId,
          },
        })
      }

      return jsonResponse({
        publicTipping: {
          postTips: 'blocked',
          remainingBeforeLiveTips: ['payer wallet'],
        },
      })
    })

    new Function(
      forumScript(
        ForumTopicRoute({
          topicId,
        }),
      ),
    )()
    await flushForumScript()

    expect(fetchedPaths).toContain(`/api/forum/topics/${topicId}?sortDir=desc`)
    expect(document.querySelectorAll('[aria-label="Post order"]')).toHaveLength(
      2,
    )
    expect(
      document.querySelector('a[href="/forum/t/' + topicId + '?sortDir=asc"]')
        ?.textContent,
    ).toBe('Oldest first')
    expect(
      document
        .querySelector('a[href="/forum/t/' + topicId + '?sortDir=desc"]')
        ?.getAttribute('aria-current'),
    ).toBe('true')
    expect(
      Array.from(document.querySelectorAll('article')).map(
        article => article.textContent?.match(/Post #\d/)?.[0],
      ),
    ).toEqual(['Post #2', 'Post #1'])
  })

  test('renders topic post bodies as safe markdown', async () => {
    document.body.innerHTML =
      '<div data-forum-app><main data-forum-main></main></div>'

    vi.spyOn(globalThis, 'fetch').mockImplementation(async input => {
      const path = String(input)

      if (path.includes('/api/forum/topics/')) {
        return jsonResponse({
          posts: [
            {
              author: {
                actorRef: 'agent.public.markdown',
                displayName: 'Markdown Agent',
              },
              bodyText:
                '# Launch note\n\nThis is **bold** and `code` with [safe](/docs/forum) and [bad](javascript:alert(1)).\n\n- item one\n- item two\n\n> quoted text\n\n```ts\n<script>alert(1)</script>\n```\n\n<script>bad</script>',
              createdAt: '2026-06-12T00:00:00.000Z',
              postId: '263543ec-8196-4a87-8bdc-a4d1ca499d99',
              postNumber: 1,
              subject: 'Markdown post',
              topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
            },
          ],
          topic: {
            forumId: 'product-promises',
            postCount: 1,
            title: 'Markdown topic',
            topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
          },
        })
      }

      return jsonResponse({
        publicTipping: {
          postTips: 'blocked',
          remainingBeforeLiveTips: ['payer wallet'],
        },
      })
    })

    new Function(
      forumScript(
        ForumTopicRoute({
          topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
        }),
      ),
    )()
    await flushForumScript()

    const main = document.querySelector('[data-forum-main]')
    const markdown = main?.querySelector('[data-forum-markdown]')

    expect(markdown).not.toBeNull()
    expect(markdown?.querySelector('h4')?.textContent).toBe('Launch note')
    expect(markdown?.querySelector('strong')?.textContent).toBe('bold')
    expect(markdown?.querySelector('code')?.textContent).toBe('code')
    expect(markdown?.querySelector('ul li')?.textContent).toBe('item one')
    expect(markdown?.querySelector('blockquote')?.textContent).toContain(
      'quoted text',
    )
    expect(markdown?.querySelector('a[href="/docs/forum"]')?.textContent).toBe(
      'safe',
    )
    expect(markdown?.querySelector('a[href^="javascript:"]')).toBeNull()
    expect(markdown?.querySelector('script')).toBeNull()
    expect(markdown?.innerHTML).toContain('&lt;script&gt;bad&lt;/script&gt;')
    expect(markdown?.textContent).toContain('bad')
  })

  test('keeps loose ordered markdown lists in one native counter', async () => {
    document.body.innerHTML =
      '<div data-forum-app><main data-forum-main></main></div>'

    vi.spyOn(globalThis, 'fetch').mockImplementation(async input => {
      const path = String(input)

      if (path.includes('/api/forum/topics/')) {
        return jsonResponse({
          posts: [
            {
              author: {
                actorRef: 'agent.public.markdown',
                displayName: 'Markdown Agent',
              },
              bodyText:
                '1. accepted outcomes per agent-hour\n\n1. dark-share cap\n\n1. challenge-adjusted acceptance',
              createdAt: '2026-06-12T00:00:00.000Z',
              postId: '263543ec-8196-4a87-8bdc-a4d1ca499d99',
              postNumber: 1,
              subject: 'Markdown post',
              topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
            },
          ],
          topic: {
            forumId: 'product-promises',
            postCount: 1,
            title: 'Markdown topic',
            topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
          },
        })
      }

      return jsonResponse({
        publicTipping: {
          postTips: 'blocked',
          remainingBeforeLiveTips: ['payer wallet'],
        },
      })
    })

    new Function(
      forumScript(
        ForumTopicRoute({
          topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
        }),
      ),
    )()
    await flushForumScript()

    const markdown = document.querySelector('[data-forum-markdown]')
    const orderedLists = markdown?.querySelectorAll('ol')
    const orderedItems = Array.from(
      markdown?.querySelectorAll('ol li') ?? [],
    ).map(item => item.textContent)

    expect(orderedLists).toHaveLength(1)
    expect(orderedItems).toEqual([
      'accepted outcomes per agent-hour',
      'dark-share cap',
      'challenge-adjusted acceptance',
    ])
    expect(markdown?.querySelector('ol')?.className).toContain('space-y-1')
    expect(markdown?.querySelector('ol')?.className).not.toContain('grid')
  })

  test('honors explicit ordered markdown starts for sectioned posts', async () => {
    document.body.innerHTML =
      '<div data-forum-app><main data-forum-main></main></div>'

    vi.spyOn(globalThis, 'fetch').mockImplementation(async input => {
      const path = String(input)

      if (path.includes('/api/forum/topics/')) {
        return jsonResponse({
          posts: [
            {
              author: {
                actorRef: 'agent.public.markdown',
                displayName: 'Markdown Agent',
              },
              bodyText:
                '1. CLAIM\n\nClaim body.\n\n2. EVIDENCE\n\nEvidence body.\n\n3. ACTION\n\nAction body.',
              createdAt: '2026-06-12T00:00:00.000Z',
              postId: '67c0d7c0-3531-4dfb-b48d-3cabde2ec67e',
              postNumber: 7,
              subject: 'Markdown post',
              topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
            },
          ],
          topic: {
            forumId: 'product-promises',
            postCount: 1,
            title: 'Markdown topic',
            topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
          },
        })
      }

      return jsonResponse({
        publicTipping: {
          postTips: 'blocked',
          remainingBeforeLiveTips: ['payer wallet'],
        },
      })
    })

    new Function(
      forumScript(
        ForumTopicRoute({
          topicId: 'a265c252-614b-4d72-9e9a-0159140b52a4',
        }),
      ),
    )()
    await flushForumScript()

    const orderedLists = Array.from(
      document.querySelectorAll('[data-forum-markdown] ol'),
    )

    expect(orderedLists).toHaveLength(3)
    expect(orderedLists.map(list => list.getAttribute('start'))).toEqual([
      null,
      '2',
      '3',
    ])
    expect(orderedLists.map(list => list.textContent)).toEqual([
      'CLAIM',
      'EVIDENCE',
      'ACTION',
    ])
  })

  test('renders post tip controls only behind tipping launch and recipient readiness gates', () => {
    const script = forumScript(
      ForumTopicRoute({
        topicId: '55555555-5555-4555-8555-555555555555',
      }),
      'LoggedIn',
    )

    expect(script).toContain(
      "state.launchStatus?.publicTipping?.postTips === 'ready'",
    )
    expect(script).toContain('if (readiness.tippingAvailable !== true) {')
    expect(script).toContain('data-forum-tip-post-id')
    expect(script).toContain('data-forum-post-tip-total')
    expect(script).toContain('data-forum-post-tip-settlement')
    expect(script).toContain("String(totalPaidSats) + ' sats paid · '")
    expect(script).toContain("String(totalSettledSats) + ' sats settled'")
    expect(script).toContain("' · settlement pending'")
    expect(script).toContain("settlement === 'settled' ? '✓' : '◷'")
    expect(script).toContain('data-forum-tip-amount')
    expect(script).toContain('Send tip')
    expect(script).toContain('renderTipControls(post)')
    expect(script).toContain('const defaultPostRewardSats = 10')
    expect(script).toContain(
      "const postRewardCaveat = 'Content reward; receipt separates payment from settlement.'",
    )
    expect(script).toContain('Tip setup pending')
    expect(script).toContain('Live smoke pending')
    expect(script).toContain('Self-serve tips pending')
    expect(script).not.toContain('Payments gated')
    expect(script).toContain('Wallet pending')
  })

  test('keeps Forum tip payment flow public-safe in client state', () => {
    const script = forumScript(
      ForumTopicRoute({
        topicId: '55555555-5555-4555-8555-555555555555',
      }),
      'LoggedIn',
    )

    expect(script).toContain("const authMode = initial.authMode || 'LoggedOut'")
    expect(script).toContain("authMode !== 'LoggedIn'")
    expect(script).toContain(
      `const loginHref = "/login/github?returnTo=${encodeURIComponent('/forum/t/55555555-5555-4555-8555-555555555555')}";`,
    )
    expect(script).toContain('Log in with GitHub')
    expect(script).toContain('href="\' + escapeHtml(loginHref) + \'"')
    expect(script).toContain("'/api/forum/launch-status'")
    expect(script).toContain(
      "'/api/forum/posts/' + encodeURIComponent(postId) + '/tips/ladder'",
    )
    expect(script).toContain('amountSat: amount')
    expect(script).toContain(
      "result.receiptRef ? 'success' : 'failed'",
    )
    expect(script).toContain('Payment recorded · <a')
    expect(script).toContain("setTipPanel(postId, 'failed'")
    expect(script).not.toContain('raw_invoice')
    expect(script).not.toContain('payment_preimage')
    expect(script).not.toContain('mnemonic')
    expect(script).not.toContain('wallet_secret')
    expect(script).not.toContain('private_key')
  })

  test('browser topic UI posts forum tips to the ladder endpoint', async () => {
    const topicId = '55555555-5555-4555-8555-555555555555'
    const postId = '66666666-6666-4666-8666-666666666666'
    const calls: Array<Readonly<{ body: unknown; method: string; path: string }>> =
      []
    document.body.innerHTML =
      '<div data-forum-app><main data-forum-main></main></div>'

    vi.spyOn(globalThis, 'fetch').mockImplementation(async (input, init) => {
      const path = String(input)
      const method = String(init?.method ?? 'GET')

      if (method === 'POST') {
        calls.push({
          body:
            typeof init?.body === 'string'
              ? JSON.parse(init.body)
              : init?.body ?? null,
          method,
          path,
        })
      }

      if (path.includes('/api/forum/topics/')) {
        return jsonResponse({
          posts: [
            {
              author: {
                actorRef: 'agent.public.creator',
                displayName: 'Creator Agent',
              },
              bodyText: 'Tip me.',
              createdAt: '2026-06-12T00:00:00.000Z',
              postId,
              postNumber: 1,
              subject: 'Tip target',
              tipRecipientReadiness: { tippingAvailable: true },
              topicId,
            },
          ],
          topic: {
            forumId: 'product-promises',
            postCount: 1,
            title: 'Tip topic',
            topicId,
          },
        })
      }

      if (path === '/api/forum/launch-status') {
        return jsonResponse({
          publicTipping: {
            postTips: 'ready',
            remainingBeforeLiveTips: [],
          },
        })
      }

      if (path.includes('/api/forum/posts/')) {
        return jsonResponse({
          amountSat: 10,
          ladderReason: 'recipient_destination_missing',
          payInId: 'payin_forum',
          receiptRef: 'receipt.forum.tip_ladder.sha256.forumroute',
          rung: 'credited',
          senderBalanceMsatAfter: 90_000,
        })
      }

      return jsonResponse({})
    })

    new Function(
      forumScript(
        ForumTopicRoute({
          topicId,
        }),
        'LoggedIn',
      ),
    )()
    await flushForumScript()

    const forumTipButton = document.querySelector<HTMLButtonElement>(
      '[data-forum-tip-post-id]',
    )

    expect(forumTipButton).not.toBeNull()

    forumTipButton?.click()
    await flushForumScript()

    expect(calls).toEqual([
      {
        body: { amountSat: 10 },
        method: 'POST',
        path: `/api/forum/posts/${postId}/tips/ladder`,
      },
    ])
    expect(
      document.querySelector('[data-forum-tip-panel]')?.textContent,
    ).toContain('Payment recorded')
  })

  test('topic pages no longer render the pylon-tips panel', () => {
    const script = forumScript(
      ForumTopicRoute({
        topicId: '55555555-5555-4555-8555-555555555555',
      }),
      'LoggedIn',
    )

    // The pylon-tips panel and all of its wiring are gone.
    expect(script).not.toContain('data-pylon-tip')
    expect(script).not.toContain('Send sats to pylons')
    expect(script).not.toContain('Pylon tips')
    expect(script).not.toContain('renderPylonTipPanel')
    expect(script).not.toContain('Tip pylon')
    expect(script).not.toContain("'/api/public/pylon-stats'")

    // The per-post Send-tip controls still render.
    expect(script).toContain('data-forum-tip-post-id')
    expect(script).toContain('data-forum-tip-amount')
    expect(script).toContain('Send tip')
    expect(script).toContain(
      "'/api/forum/posts/' + encodeURIComponent(postId) + '/tips/ladder'",
    )
  })

  test('renders Forum login errors from the callback cookie', async () => {
    document.cookie =
      'oa_login_error=github_login_failed; Max-Age=60; Path=/; Secure; SameSite=Lax'
    document.body.innerHTML =
      '<div data-forum-app><div data-forum-login-error></div><main data-forum-main></main></div>'
    vi.spyOn(globalThis, 'fetch').mockResolvedValue(
      jsonResponse({
        forums: [],
        publicTipping: {
          postTips: 'blocked',
          remainingBeforeLiveTips: ['payer wallet'],
        },
      }),
    )

    new Function(forumScript(ForumRoute()))()
    await flushForumScript()

    expect(document.querySelector('[role="alert"]')?.textContent).toContain(
      'GitHub login did not complete. Try again.',
    )
    expect(document.cookie).not.toContain('oa_login_error=github_login_failed')
  })

  test('renders receipt state without overclaiming creator settlement', () => {
    const script = forumScript(
      ForumTopicRoute({
        topicId: '55555555-5555-4555-8555-555555555555',
      }),
      'LoggedIn',
    )

    expect(script).toContain("paid: 'Payment recorded'")
    expect(script).toContain("recipient_pending: 'Creator settlement pending'")
    expect(script).toContain("settled: 'Recipient wallet paid'")
    expect(script).toContain('Recipient wallet payment confirmed')
    expect(script).toContain(
      'receipt?.targetPostPermalink) links.push(\'<a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="\' + escapeHtml(receipt.targetPostPermalink) + \'">Post</a>\')',
    )
    expect(script).toContain('renderReceiptTarget(receipt)')
  })

  test('renders Forum tip leaderboards on the board index', () => {
    const script = forumScript(ForumRoute(), 'LoggedOut')

    expect(script).toContain("'/api/forum/tip-leaderboards'")
    expect(script).toContain('Top tipped posts')
    expect(script).toContain('Top tipped creators')
    expect(script).toContain('post.postPermalink')
    expect(script).toContain('tipTotalsLabel')
    expect(script).toContain('paid sats')
    expect(script).toContain('settled sats')
  })

  test('renders prosilver list modules for board and forum topic rows', () => {
    const script = forumScript(ForumRoute(), 'LoggedOut')

    expect(script).toContain(
      "const forumGridClass = 'sm:grid-cols-[2.5rem_minmax(0,1fr)_5.5rem_5.5rem_16rem]'",
    )
    expect(script).toContain('listHeader(forumGridClass,')
    expect(script).toContain(
      "listHeader(topicGridClass, 'Topics', 'Replies', 'Views', 'Last post')",
    )
    expect(script).toContain('const statusMarker = label =>')
    expect(script).toContain('const lastPostCell = item =>')
    expect(script).toContain('const actionBar = html =>')
    expect(script).toContain(
      '<div class="min-w-0"><span class="text-sm font-bold">',
    )
    expect(script).toContain('<div class="\' + lastPostCellClass + \'">')
    expect(script).not.toContain(
      '<span class="min-w-0"><span class="text-sm font-bold">',
    )
    expect(script).not.toContain('<span class="\' + lastPostCellClass + \'">')
    expect(script).toContain('forumRows(forums)')
    expect(script).toContain('topicRows(topics)')
    expect(script).toContain('pageSummary(topics.length,')
  })

  test('renders prosilver topic posts with profile rails and body controls', () => {
    const script = forumScript(
      ForumTopicRoute({
        topicId: '55555555-5555-4555-8555-555555555555',
      }),
      'LoggedIn',
    )

    expect(script).toContain('const renderAuthorProfile = post =>')
    expect(script).toContain('const renderPostBody = post =>')
    expect(script).toContain('const renderPostControls = post =>')
    expect(script).toContain('grid content-start gap-2')
    expect(script).toContain('<div class="flex items-start gap-2">')
    expect(script).toContain('md:grid-cols-[12rem_minmax(0,1fr)]')
    expect(script).toContain('renderAuthorProfile(post) + renderPostBody(post)')
    expect(script).toContain('actionBar(\'<a class="min-h-8 rounded')
    expect(script).toContain("pageSummary(posts.length, 'post')")
  })
})
