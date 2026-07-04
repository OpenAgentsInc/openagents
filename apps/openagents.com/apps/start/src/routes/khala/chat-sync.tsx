import { createFileRoute } from '@tanstack/react-router'
import { MonitorSmartphone, RefreshCw, Search } from 'lucide-react'
import { useMemo, useState } from 'react'

import {
  decodeChatThreadEntity,
  type ChatThreadEntity,
} from '@openagentsinc/khala-sync'
import { PageShell } from '../-funnel-components'
import {
  WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE,
  projectWebChatThreadSidebar,
} from '../-chat-sync-collection'

export const Route = createFileRoute('/khala/chat-sync')({
  component: WebChatSyncPanel,
  head: () => ({
    meta: [
      { title: 'Khala chat sync - OpenAgents' },
      {
        name: 'description',
        content: 'Khala chat thread sync panel for the Start staging app.',
      },
    ],
  }),
})

const ownerUserId = 'web-start-demo-owner'

const seedThread = (
  input: Readonly<{
    createdAt: string
    lastMessageAt: string | null
    messageCount: number
    threadId: string
    title: string
    updatedAt: string
  }>,
): ChatThreadEntity =>
  decodeChatThreadEntity({
    ...input,
    ownerUserId,
    status: 'active',
  })

const initialThreads: ReadonlyArray<ChatThreadEntity> = [
  seedThread({
    createdAt: '2026-07-04T15:10:00.000Z',
    lastMessageAt: '2026-07-04T15:14:00.000Z',
    messageCount: 4,
    threadId: 'thread.local.kh-101',
    title: 'Desktop handoff',
    updatedAt: '2026-07-04T15:14:00.000Z',
  }),
  seedThread({
    createdAt: '2026-07-04T14:02:00.000Z',
    lastMessageAt: null,
    messageCount: 0,
    threadId: 'thread.local.kh-099',
    title: 'New support thread',
    updatedAt: '2026-07-04T14:02:00.000Z',
  }),
]

const messageLabel = (count: number): string =>
  count === 1 ? '1 message' : `${count} messages`

export function WebChatSyncPanel() {
  const [threads, setThreads] = useState(initialThreads)
  const [searchTerm, setSearchTerm] = useState('')
  const visibleThreads = useMemo(
    () => projectWebChatThreadSidebar(threads, searchTerm),
    [threads, searchTerm],
  )

  const createRemoteThread = () => {
    const remote = seedThread({
      createdAt: '2026-07-04T17:00:00.000Z',
      lastMessageAt: null,
      messageCount: 0,
      threadId: 'thread.remote.kh-202',
      title: 'Remote device thread',
      updatedAt: '2026-07-04T17:00:00.000Z',
    })
    setThreads(current =>
      current.some(thread => thread.threadId === remote.threadId)
        ? current
        : [remote, ...current],
    )
  }

  return (
    <PageShell dataRoute="khala-chat-sync">
      <main className="mx-auto grid w-[min(100%,1120px)] gap-6 px-4 py-8 text-khala-text">
        <section className="grid gap-5 border border-khala-border/80 bg-khala-surface p-4 sm:p-5">
          <div className="flex flex-col gap-4 border-b border-khala-border/70 pb-4 lg:flex-row lg:items-center lg:justify-between">
            <div className="grid gap-1">
              <p className="m-0 font-mono text-sm uppercase tracking-wide text-khala-energy-soft">
                {WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE}
              </p>
              <h1 className="m-0 text-2xl font-semibold text-white sm:text-xl">
                Khala chat threads
              </h1>
            </div>
            <button
              className="khala-focus inline-flex min-h-12 items-center justify-center gap-2 border border-khala-border-strong/70 bg-khala-surface-raised px-4 font-mono text-base font-semibold text-khala-text sm:min-h-10 sm:text-sm"
              type="button"
              onClick={createRemoteThread}
            >
              <RefreshCw aria-hidden="true" className="size-5 sm:size-4" />
              Simulate remote create
            </button>
          </div>
          <label className="grid min-h-12 grid-cols-[auto_1fr] items-center gap-3 border border-khala-border/80 bg-khala-void px-3 text-base text-khala-text focus-within:border-khala-energy-cyan sm:min-h-10 sm:text-sm">
            <Search aria-hidden="true" className="size-5 text-khala-text-faint sm:size-4" />
            <input
              className="min-w-0 bg-transparent font-mono text-khala-text outline-none placeholder:text-khala-text-faint"
              value={searchTerm}
              placeholder="Search threads"
              aria-label="Search threads"
              onChange={event => setSearchTerm(event.target.value)}
            />
          </label>
          <ol className="m-0 grid list-none gap-2 p-0">
            {visibleThreads.map(thread => (
              <li
                className="grid gap-2 border border-khala-border/70 bg-khala-surface-muted p-3"
                data-thread-id={thread.threadId}
                key={thread.threadId}
              >
                <div className="flex min-w-0 items-center gap-3">
                  <span className="grid size-9 shrink-0 place-items-center border border-khala-border/80 bg-khala-surface-raised text-khala-energy-cyan">
                    <MonitorSmartphone aria-hidden="true" className="size-4" />
                  </span>
                  <div className="min-w-0">
                    <p className="m-0 truncate text-base font-semibold text-white sm:text-sm">
                      {thread.title}
                    </p>
                    <p className="m-0 truncate font-mono text-base text-khala-text-muted sm:text-sm">
                      {messageLabel(thread.messageCount)}
                    </p>
                  </div>
                </div>
                <p className="m-0 truncate font-mono text-base text-khala-text-faint sm:text-sm">
                  {thread.updatedAt}
                </p>
              </li>
            ))}
          </ol>
        </section>
      </main>
    </PageShell>
  )
}
