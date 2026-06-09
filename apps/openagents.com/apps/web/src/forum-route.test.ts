import { Option } from 'effect'
import { Scene } from 'foldkit'
import { describe, expect, test } from 'vitest'

import { Flags, init } from './main'
import { LoggedOut } from './model'
import { forumScript } from './page/forum'
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
      Scene.expect(Scene.text('Fetching the Forum API.')).toExist(),
      Scene.expect(Scene.text('No listed forums yet.')).not.toExist(),
    )
  })

  test('renders the explicit void Forum path without a browser composer', () => {
    Scene.scene(
      { update, view },
      Scene.with(LoggedOut.init(ForumForumRoute({ forumRef: 'void' }))),
      Scene.expect(Scene.role('heading', { name: 'Forum' })).not.toExist(),
      Scene.expect(Scene.role('link', { name: 'Void' })).not.toExist(),
      Scene.expect(Scene.role('button', { name: 'Save token' })).not.toExist(),
      Scene.expect(Scene.role('button', { name: 'Clear' })).not.toExist(),
      Scene.expect(Scene.text('Fetching the Forum API.')).toExist(),
    )
  })

  test('renders the Forum topic route shell without duplicate topic chrome', () => {
    Scene.scene(
      { update, view },
      Scene.with(
        LoggedOut.init(
          ForumTopicRoute({
            topicId: '55555555-5555-4555-8555-555555555555',
          }),
        ),
      ),
      Scene.expect(Scene.role('heading', { name: 'Forum' })).not.toExist(),
      Scene.expect(Scene.text('Topic 55555555')).not.toExist(),
      Scene.expect(Scene.role('link', { name: 'Void' })).not.toExist(),
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
    expect(script).toContain("postControlLink(postHref(post), 'Permalink')")
    expect(script).toContain(
      "window.addEventListener('hashchange', scrollPostAnchorIntoView);",
    )
    expect(script).toContain('requestAnimationFrame(scrollPostAnchorIntoView);')
    expect(script).toContain(
      "href=\"/forum/t/' + encodeURIComponent(target.topicId) + '#post-",
    )
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
    expect(script).toContain('sats paid')
    expect(script).toContain('settled')
    expect(script).toContain('data-forum-tip-amount')
    expect(script).toContain('Send tip')
    expect(script).toContain('const defaultPostRewardSats = 10')
    expect(script).toContain(
      "const postRewardCaveat = 'Content reward; receipt separates payment from settlement.'",
    )
    expect(script).toContain('Tip payments pending')
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
    expect(script).toContain("api('/api/forum/launch-status')")
    expect(script).toContain('checkoutLaunchPath')
    expect(script).toContain(
      "result.receiptRef ? 'success' : 'payment_required'",
    )
    expect(script).toContain('Tip paid · <a')
    expect(script).toContain('Payment required · ')
    expect(script).toContain('Agent L402 challenge issued')
    expect(script).toContain("setTipPanel(postId, 'failed'")
    expect(script).not.toContain('raw_invoice')
    expect(script).not.toContain('payment_preimage')
    expect(script).not.toContain('mnemonic')
    expect(script).not.toContain('wallet_secret')
    expect(script).not.toContain('private_key')
  })

  test('renders receipt state without overclaiming creator settlement', () => {
    const script = forumScript(
      ForumTopicRoute({
        topicId: '55555555-5555-4555-8555-555555555555',
      }),
      'LoggedIn',
    )

    expect(script).toContain("paid: 'Tip paid'")
    expect(script).toContain("recipient_pending: 'Creator settlement pending'")
    expect(script).toContain("settled: 'Tip paid'")
    expect(script).toContain('MDK payment confirmed')
    expect(script).toContain(
      'receipt?.targetPostPermalink) links.push(\'<a class="text-forum-link underline underline-offset-4 hover:text-forum-link-hover" href="\' + escapeHtml(receipt.targetPostPermalink) + \'">Post</a>\')',
    )
    expect(script).toContain('renderReceiptTarget(receipt)')
  })

  test('renders Forum tip leaderboards on the board index', () => {
    const script = forumScript(ForumRoute(), 'LoggedOut')

    expect(script).toContain("api('/api/forum/tip-leaderboards')")
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
    expect(script).toContain('<div class="min-w-0"><span class="text-sm font-bold">')
    expect(script).toContain('<div class="\' + lastPostCellClass + \'">')
    expect(script).not.toContain('<span class="min-w-0"><span class="text-sm font-bold">')
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
    expect(script).toContain("actionBar('<a class=\"min-h-8 rounded")
    expect(script).toContain("pageSummary(posts.length, 'post')")
  })
})
