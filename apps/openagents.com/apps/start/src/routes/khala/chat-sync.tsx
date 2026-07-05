import { createFileRoute } from '@tanstack/react-router'
import { LogOut, MonitorSmartphone, Plus, Search, SendHorizontal } from 'lucide-react'
import { useMemo, useState, type FormEvent } from 'react'

import {
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  personalScope,
  threadScope,
  type ChatMessageEntity,
  type ChatThreadEntity,
} from '@openagentsinc/khala-sync'
import { chatMessagesForTranscript } from '@openagentsinc/khala-sync-db-collection'

import { PageShell } from '../-funnel-components'
import {
  WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE,
  projectWebChatThreadSidebar,
} from '../-chat-sync-collection'
import { makeSafeRef } from '../-chat-sync-web-core'
import { useKhalaSyncSession } from '../-khala-sync-session'
import { useKhalaSyncWebCollection } from '../-use-khala-sync-collection'
import { useKhalaSyncWebPush, type PendingMutation } from '../-use-khala-sync-push'

export const Route = createFileRoute('/khala/chat-sync')({
  component: WebChatSyncPanel,
  head: () => ({
    meta: [
      { title: 'Khala chat sync - OpenAgents' },
      {
        name: 'description',
        content: 'Real Khala Sync chat client for the Start staging app.',
      },
    ],
  }),
})

const CHAT_MESSAGE_ENTITY_TYPE = 'chat_message'

const inputClass =
  'min-h-11 w-full border border-khala-border bg-black px-3 py-2 font-mono text-base text-khala-text outline-none placeholder:text-khala-text-faint focus:border-khala-energy-cyan sm:text-sm'

const buttonClass =
  'khala-focus inline-flex min-h-11 items-center justify-center gap-2 border border-khala-border-strong/70 bg-khala-surface-raised px-4 font-mono text-sm font-semibold text-khala-text disabled:opacity-50'

const primaryButtonClass =
  'khala-focus inline-flex min-h-11 items-center justify-center gap-2 border border-khala-energy-cyan/60 bg-khala-energy px-4 font-mono text-sm font-semibold text-black disabled:opacity-50'

function KhalaSyncSignInForm({
  onSignIn,
}: Readonly<{
  onSignIn: (input: {
    ownerUserId: string
    token: string
  }) => Promise<{ ok: true } | { ok: false; messageSafe: string }>
}>) {
  const [ownerUserId, setOwnerUserId] = useState('')
  const [token, setToken] = useState('')
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    setPending(true)
    setError(null)
    const result = await onSignIn({ ownerUserId, token })
    setPending(false)
    if (!result.ok) setError(result.messageSafe)
  }

  return (
    <section className="grid w-full max-w-[440px] gap-5 border border-khala-border bg-khala-surface p-6 font-mono sm:p-8">
      <div className="grid gap-2">
        <h1 className="m-0 text-xl font-semibold text-white">Sign in to Khala Sync</h1>
        <p className="m-0 text-sm/6 text-khala-text-muted">
          Enter an owner user id and its Khala Sync bearer token. This validates a real
          bootstrap call against that owner&apos;s personal scope before saving anything —
          the token is stored only in an httpOnly cookie on this Worker, never in
          browser-readable storage.
        </p>
      </div>
      <form className="grid gap-3" onSubmit={submit}>
        <label className="grid gap-1.5 text-sm text-khala-text-muted">
          <span>Owner user id</span>
          <input
            autoComplete="off"
            className={inputClass}
            name="ownerUserId"
            onChange={event => setOwnerUserId(event.target.value)}
            placeholder="user.abc123"
            value={ownerUserId}
          />
        </label>
        <label className="grid gap-1.5 text-sm text-khala-text-muted">
          <span>Bearer token</span>
          <input
            autoComplete="off"
            className={inputClass}
            name="token"
            onChange={event => setToken(event.target.value)}
            placeholder="oa_agent_..."
            type="password"
            value={token}
          />
        </label>
        {error === null ? null : (
          <p className="m-0 text-sm text-khala-danger" role="alert">
            {error}
          </p>
        )}
        <button className={primaryButtonClass} disabled={pending} type="submit">
          {pending ? 'Checking…' : 'Sign in'}
        </button>
      </form>
    </section>
  )
}

const messageLabel = (count: number): string =>
  count === 1 ? '1 message' : `${count} messages`

function ThreadListPanel({
  ownerUserId,
  selectedThreadId,
  onSelectThread,
  push,
}: Readonly<{
  ownerUserId: string
  selectedThreadId: string | null
  onSelectThread: (threadId: string) => void
  push: ReturnType<typeof useKhalaSyncWebPush>
}>) {
  const [searchTerm, setSearchTerm] = useState('')
  const [newThreadTitle, setNewThreadTitle] = useState('')
  const [creating, setCreating] = useState(false)
  const [createError, setCreateError] = useState<string | null>(null)

  const collection = useKhalaSyncWebCollection<ChatThreadEntity>({
    decode: decodeChatThreadEntity,
    entityType: WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE,
    idOf: thread => thread.threadId,
    scope: String(personalScope(ownerUserId)),
    signedIn: ownerUserId !== '',
  })

  const visibleThreads = useMemo(
    () => projectWebChatThreadSidebar(collection.items, searchTerm),
    [collection.items, searchTerm],
  )

  const createThread = async (event: FormEvent) => {
    event.preventDefault()
    const title = newThreadTitle.trim()
    if (title === '') return
    setCreating(true)
    setCreateError(null)
    const threadId = makeSafeRef('thread')
    try {
      const mutation: PendingMutation = { args: { threadId, title }, name: 'chat.createThread' }
      await push([mutation])
      setNewThreadTitle('')
      onSelectThread(threadId)
    } catch (error) {
      setCreateError(error instanceof Error ? error.message : String(error))
    } finally {
      setCreating(false)
    }
  }

  return (
    <section
      aria-label="Khala chat threads"
      className="grid content-start gap-4 border border-khala-border/80 bg-khala-surface p-4 sm:p-5"
    >
      <div className="grid gap-1 border-b border-khala-border/70 pb-3">
        <p className="m-0 font-mono text-xs uppercase tracking-wide text-khala-energy-soft">
          {WEB_CHAT_THREAD_COLLECTION_ENTITY_TYPE} · {String(personalScope(ownerUserId))}
        </p>
        <h2 className="m-0 text-lg font-semibold text-white">Threads</h2>
        <p className="m-0 font-mono text-xs text-khala-text-faint" data-collection-status={collection.status}>
          {collection.status === 'loading'
            ? 'bootstrapping…'
            : collection.status === 'error'
              ? `error: ${collection.error ?? 'unknown'}`
              : 'live'}
        </p>
      </div>

      <form className="grid grid-cols-[1fr_auto] gap-2" onSubmit={createThread}>
        <input
          className={inputClass}
          onChange={event => setNewThreadTitle(event.target.value)}
          placeholder="New thread title"
          value={newThreadTitle}
        />
        <button className={buttonClass} disabled={creating || newThreadTitle.trim() === ''} type="submit">
          <Plus aria-hidden="true" className="size-4" />
          {creating ? 'Creating…' : 'New'}
        </button>
      </form>
      {createError === null ? null : (
        <p className="m-0 text-sm text-khala-danger" role="alert">
          {createError}
        </p>
      )}

      <label className="grid min-h-11 grid-cols-[auto_1fr] items-center gap-3 border border-khala-border/80 bg-khala-void px-3 text-sm text-khala-text focus-within:border-khala-energy-cyan">
        <Search aria-hidden="true" className="size-4 text-khala-text-faint" />
        <input
          className="min-w-0 bg-transparent font-mono text-khala-text outline-none placeholder:text-khala-text-faint"
          value={searchTerm}
          placeholder="Search threads"
          aria-label="Search threads"
          onChange={event => setSearchTerm(event.target.value)}
        />
      </label>

      <ol className="m-0 grid list-none gap-2 p-0">
        {visibleThreads.length === 0 ? (
          <li className="border border-dashed border-khala-border/60 p-3 font-mono text-sm text-khala-text-faint">
            No threads yet — create one above.
          </li>
        ) : null}
        {visibleThreads.map(thread => (
          <li key={thread.threadId}>
            <button
              className={`grid w-full gap-2 border p-3 text-left ${
                thread.threadId === selectedThreadId
                  ? 'border-khala-energy-cyan bg-khala-surface-muted'
                  : 'border-khala-border/70 bg-khala-surface-muted/60'
              }`}
              data-thread-id={thread.threadId}
              onClick={() => onSelectThread(thread.threadId)}
              type="button"
            >
              <div className="flex min-w-0 items-center gap-3">
                <span className="grid size-9 shrink-0 place-items-center border border-khala-border/80 bg-khala-surface-raised text-khala-energy-cyan">
                  <MonitorSmartphone aria-hidden="true" className="size-4" />
                </span>
                <div className="min-w-0">
                  <p className="m-0 truncate text-sm font-semibold text-white">{thread.title}</p>
                  <p className="m-0 truncate font-mono text-xs text-khala-text-muted">
                    {messageLabel(thread.messageCount)}
                  </p>
                </div>
              </div>
              <p className="m-0 truncate font-mono text-xs text-khala-text-faint">{thread.updatedAt}</p>
            </button>
          </li>
        ))}
      </ol>
    </section>
  )
}

function ThreadMessagesPanel({
  ownerUserId,
  threadId,
  push,
}: Readonly<{
  ownerUserId: string
  threadId: string
  push: ReturnType<typeof useKhalaSyncWebPush>
}>) {
  const [draft, setDraft] = useState('')
  const [sending, setSending] = useState(false)
  const [sendError, setSendError] = useState<string | null>(null)

  const collection = useKhalaSyncWebCollection<ChatMessageEntity>({
    decode: decodeChatMessageEntity,
    entityType: CHAT_MESSAGE_ENTITY_TYPE,
    idOf: message => message.messageId,
    scope: String(threadScope(threadId)),
    signedIn: ownerUserId !== '',
  })

  const messages = useMemo(() => chatMessagesForTranscript(collection.items), [collection.items])

  const sendMessage = async (event: FormEvent) => {
    event.preventDefault()
    const body = draft.trim()
    if (body === '') return
    setSending(true)
    setSendError(null)
    const messageId = makeSafeRef('msg')
    try {
      const mutation: PendingMutation = {
        args: { body, messageId, threadId },
        name: 'chat.appendMessage',
      }
      await push([mutation])
      setDraft('')
    } catch (error) {
      setSendError(error instanceof Error ? error.message : String(error))
    } finally {
      setSending(false)
    }
  }

  return (
    <section
      aria-label="Thread messages"
      className="grid content-start gap-4 border border-khala-border/80 bg-khala-surface p-4 sm:p-5"
    >
      <div className="grid gap-1 border-b border-khala-border/70 pb-3">
        <p className="m-0 font-mono text-xs uppercase tracking-wide text-khala-energy-soft">
          {CHAT_MESSAGE_ENTITY_TYPE} · {String(threadScope(threadId))}
        </p>
        <h2 className="m-0 truncate text-lg font-semibold text-white">{threadId}</h2>
      </div>

      <ol className="m-0 grid max-h-[420px] list-none gap-2 overflow-y-auto p-0">
        {messages.length === 0 ? (
          <li className="border border-dashed border-khala-border/60 p-3 font-mono text-sm text-khala-text-faint">
            No messages yet.
          </li>
        ) : null}
        {messages.map(message => (
          <li
            className="grid gap-1 border border-khala-border/70 bg-khala-surface-muted p-3"
            data-message-id={message.messageId}
            key={message.messageId}
          >
            <p className="m-0 whitespace-pre-wrap text-sm text-khala-text">{message.body}</p>
            <p className="m-0 font-mono text-xs text-khala-text-faint">{message.createdAt}</p>
          </li>
        ))}
      </ol>

      <form className="grid grid-cols-[1fr_auto] gap-2" onSubmit={sendMessage}>
        <input
          className={inputClass}
          onChange={event => setDraft(event.target.value)}
          placeholder="Message…"
          value={draft}
        />
        <button className={primaryButtonClass} disabled={sending || draft.trim() === ''} type="submit">
          <SendHorizontal aria-hidden="true" className="size-4" />
          {sending ? 'Sending…' : 'Send'}
        </button>
      </form>
      {sendError === null ? null : (
        <p className="m-0 text-sm text-khala-danger" role="alert">
          {sendError}
        </p>
      )}
    </section>
  )
}

export function WebChatSyncPanel() {
  const session = useKhalaSyncSession()
  const push = useKhalaSyncWebPush()
  const [selectedThreadId, setSelectedThreadId] = useState<string | null>(null)

  return (
    <PageShell dataRoute="khala-chat-sync">
      <main className="mx-auto grid w-[min(100%,1120px)] gap-6 px-4 py-8 text-khala-text">
        {session.status === 'loading' ? (
          <p className="m-0 font-mono text-sm text-khala-text-faint">Loading Khala Sync session…</p>
        ) : session.status === 'signed_out' ? (
          <div className="grid place-items-center py-8">
            <KhalaSyncSignInForm onSignIn={session.signIn} />
          </div>
        ) : (
          <>
            <div className="flex items-center justify-between gap-3 border-b border-khala-border/70 pb-4">
              <div className="grid gap-1">
                <h1 className="m-0 text-2xl font-semibold text-white sm:text-xl">
                  Khala chat sync
                </h1>
                <p className="m-0 font-mono text-xs text-khala-text-faint">
                  signed in as {session.ownerUserId}
                </p>
              </div>
              <button className={buttonClass} onClick={() => void session.signOut()} type="button">
                <LogOut aria-hidden="true" className="size-4" />
                Sign out
              </button>
            </div>
            <div className="grid gap-4 lg:grid-cols-[320px_1fr]">
              <ThreadListPanel
                onSelectThread={setSelectedThreadId}
                ownerUserId={session.ownerUserId ?? ''}
                push={push}
                selectedThreadId={selectedThreadId}
              />
              {selectedThreadId === null ? (
                <section className="grid place-items-center border border-dashed border-khala-border/60 p-8 font-mono text-sm text-khala-text-faint">
                  Select a thread, or create one, to see its messages.
                </section>
              ) : (
                <ThreadMessagesPanel
                  ownerUserId={session.ownerUserId ?? ''}
                  push={push}
                  threadId={selectedThreadId}
                />
              )}
            </div>
          </>
        )}
      </main>
    </PageShell>
  )
}
