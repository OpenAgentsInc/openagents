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
  test('renders the CSR panel against the chat_thread collection contract', () => {
    const html = renderToStaticMarkup(<WebChatSyncPanel />)

    expect(html).toContain('data-route="khala-chat-sync"')
    expect(html).toContain(WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE)
    expect(html).toContain('Desktop handoff')
    expect(html).toContain('Simulate remote create')
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
})
