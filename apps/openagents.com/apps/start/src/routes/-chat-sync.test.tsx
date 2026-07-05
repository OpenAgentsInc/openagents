import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { decodeChatThreadEntity } from '@openagentsinc/khala-sync'
import {
  WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE,
  projectWebChatThreadSidebar,
} from './-chat-sync-collection'
import { WebChatSyncPanel } from './khala/chat-sync'

const ownerUserId = 'web-start-test-owner'

const thread = (
  threadId: string,
  title: string,
  updatedAt: string,
) =>
  decodeChatThreadEntity({
    createdAt: updatedAt,
    lastMessageAt: null,
    messageCount: 0,
    ownerUserId,
    status: 'active',
    threadId,
    title,
    updatedAt,
  })

describe('Start Khala chat sync route', () => {
  test('renders the real (pre-session-check) CSR panel shell — issue #8413 replaced the fixture demo with a real Khala Sync client', () => {
    // `useKhalaSyncSession`'s effect never runs under `renderToStaticMarkup`
    // (no browser, no effects), so this captures the deterministic FIRST
    // paint before the session-status fetch resolves — no network call
    // happens during this render. The real signed-in/signed-out UI is
    // exercised via the proxy-level tests in `../khala-sync-proxy.test.ts`
    // and the pure wire-protocol tests in `./-chat-sync-web-core.test.ts`;
    // fully driving the effectful hooks needs a browser (or an injected
    // fetch/WebSocket harness) this render helper does not provide.
    const html = renderToStaticMarkup(<WebChatSyncPanel />)

    expect(html).toContain('data-route="khala-chat-sync"')
    expect(html).toContain('Loading Khala Sync session')
  })

  test('sorts a remote device thread ahead of older local rows with the shared projection', () => {
    const projected = projectWebChatThreadSidebar([
      thread('thread.local', 'Local thread', '2026-07-04T14:00:00.000Z'),
      thread('thread.remote', 'Remote device thread', '2026-07-04T17:00:00.000Z'),
    ])

    expect(projected.map(row => row.threadId)).toEqual([
      'thread.remote',
      'thread.local',
    ])
    expect(projectWebChatThreadSidebar(projected, 'remote').map(row => row.threadId)).toEqual([
      'thread.remote',
    ])
  })

  test('WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE still names the chat_thread entity used by the real bootstrap/live-tail collection', () => {
    expect(WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE).toBe('chat_thread')
  })
})
