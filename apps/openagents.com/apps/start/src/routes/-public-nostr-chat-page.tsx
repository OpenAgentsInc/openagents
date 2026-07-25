import {
  AlertTriangle,
  Bot,
  Check,
  ChevronDown,
  Clipboard,
  Code2,
  FileUp,
  Flag,
  Link2,
  LoaderCircle,
  MessageSquareReply,
  PlugZap,
  Radio,
  RefreshCw,
  SendHorizontal,
  ShieldCheck,
  SmilePlus,
  Trash2,
  WifiOff,
} from 'lucide-react'
import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react'

import { PublicHeader } from '@/components/public-header'
import {
  NostrEvent,
  PUBLIC_CHAT_GROUP_ID,
  PUBLIC_CHAT_LIMITS,
  PUBLIC_CHAT_RELAY_URL,
  PublicNostrChatManifest,
  connectPublicChatRemoteSigner,
  hasContentWarning,
  isAuthorDeletion,
  makePublicChatRelayClient,
  npubFor,
  parseInlineAttachments,
  previousReferences,
  publicChatEventTemplate,
  relayGroupAdministrators,
  replyTagsAndContent,
  stableChronological,
  validatePublicChatEvent,
  type PublicChatRelaySnapshot,
  type PublicChatSigner,
} from '@openagentsinc/public-nostr-chat'
import { Schema as S } from 'effect'
import { BlossomClient } from 'nostr-effect/nipb7'
import { verifyEvent } from 'nostr-effect/pure'

import { connectNip55WebSigner, restoreNip55WebSigner } from './-nip55-web'

type BrowserNostr = Readonly<{
  getPublicKey: () => Promise<string>
  signEvent: (
    event: Readonly<{
      content: string
      created_at: number
      kind: number
      tags: string[][]
    }>,
  ) => Promise<unknown>
}>

declare global {
  interface Window {
    nostr?: BrowserNostr
  }
}

type ComposerState =
  | 'idle'
  | 'signing'
  | 'publishing'
  | 'accepted'
  | 'rejected'

const emptySnapshot: PublicChatRelaySnapshot = {
  events: [],
  gapReason: null,
  lastCurrentAt: null,
  state: 'disconnected',
}

const shellButton =
  'inline-flex min-h-11 items-center justify-center gap-2 border border-white/15 bg-white/[0.04] px-3 font-mono text-xs text-white transition hover:border-white/30 hover:bg-white/[0.07] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cyan-300 disabled:cursor-not-allowed disabled:opacity-45'
const primaryButton =
  'inline-flex min-h-11 items-center justify-center gap-2 border border-cyan-300/50 bg-cyan-300 px-4 font-mono text-xs font-semibold text-black transition hover:bg-cyan-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-white disabled:cursor-not-allowed disabled:opacity-45'

const shortKey = (pubkey: string): string =>
  `${pubkey.slice(0, 8)}…${pubkey.slice(-8)}`

const formatTime = (seconds: number): string =>
  new Intl.DateTimeFormat(undefined, {
    day: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    month: 'short',
  }).format(new Date(seconds * 1_000))

const browserSigner = (): PublicChatSigner | undefined => {
  if (typeof window === 'undefined' || window.nostr === undefined) return undefined
  return {
    getPublicKey: () => window.nostr!.getPublicKey(),
    signEvent: async template =>
      S.decodeUnknownSync(NostrEvent)(await window.nostr!.signEvent(template)),
  }
}

const textNodes = (content: string): ReactNode =>
  content.split(/(https?:\/\/[^\s]+|nostr:[a-z0-9]+)/gi).map((part, index) => {
    if (/^https?:\/\//i.test(part)) {
      return (
        <a
          className="break-all text-cyan-200 underline decoration-cyan-300/30 underline-offset-4 hover:text-cyan-100"
          href={part}
          key={`${part}-${index}`}
          referrerPolicy="no-referrer"
          rel="noopener noreferrer"
          target="_blank"
        >
          {part}
        </a>
      )
    }
    if (/^nostr:/i.test(part)) {
      return (
        <code
          className="break-all border border-violet-300/20 bg-violet-300/5 px-1.5 py-0.5 text-violet-200"
          key={`${part}-${index}`}
        >
          {part}
        </code>
      )
    }
    return part
  })

function MediaAttachment({
  attachment,
}: Readonly<{
  attachment: ReturnType<typeof parseInlineAttachments>[number]
}>) {
  const [state, setState] = useState<
    'gated' | 'loading' | 'verified' | 'mismatch' | 'unavailable'
  >('gated')
  const [objectUrl, setObjectUrl] = useState<string | null>(null)

  useEffect(
    () => () => {
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl)
    },
    [objectUrl],
  )

  const load = async () => {
    setState('loading')
    try {
      const response = await fetch(attachment.url, {
        credentials: 'omit',
        referrerPolicy: 'no-referrer',
      })
      if (!response.ok) throw new Error('unavailable')
      const bytes = await response.arrayBuffer()
      const digest = [...new Uint8Array(await crypto.subtle.digest('SHA-256', bytes))]
        .map(value => value.toString(16).padStart(2, '0'))
        .join('')
      if (attachment.digest !== undefined && digest !== attachment.digest) {
        setState('mismatch')
        return
      }
      setObjectUrl(URL.createObjectURL(new Blob([bytes], { type: attachment.mimeType })))
      setState('verified')
    } catch {
      setState('unavailable')
    }
  }

  if (state === 'gated' || state === 'loading') {
    return (
      <button
        className="mt-3 flex w-full items-center justify-between border border-white/10 bg-black/30 p-3 text-left font-mono text-xs text-white/70 hover:border-white/20"
        disabled={state === 'loading'}
        onClick={() => void load()}
        type="button"
      >
        <span>
          {attachment.alt ?? attachment.mimeType}
          <small className="mt-1 block text-white/35">
            Reader action required · digest {attachment.digest?.slice(0, 12) ?? 'not supplied'}
          </small>
        </span>
        {state === 'loading' ? (
          <LoaderCircle className="size-4 animate-spin" />
        ) : (
          <ShieldCheck className="size-4" />
        )}
      </button>
    )
  }
  if (state !== 'verified' || objectUrl === null) {
    return (
      <div
        className="mt-3 border border-red-300/20 bg-red-300/5 p-3 font-mono text-xs text-red-200"
        role="alert"
      >
        {state === 'mismatch'
          ? 'Media digest mismatch. The file was not rendered.'
          : 'Media is unavailable. The signed message remains visible.'}
      </div>
    )
  }
  if (attachment.mimeType.startsWith('image/')) {
    return (
      <img
        alt={attachment.alt ?? 'Chat attachment'}
        className="mt-3 max-h-96 max-w-full border border-white/10 object-contain"
        src={objectUrl}
      />
    )
  }
  if (attachment.mimeType.startsWith('audio/')) {
    return <audio className="mt-3 w-full" controls preload="none" src={objectUrl} />
  }
  if (attachment.mimeType.startsWith('video/')) {
    return (
      <video className="mt-3 max-h-96 max-w-full" controls muted preload="none" src={objectUrl} />
    )
  }
  return (
    <a
      className={`${shellButton} mt-3`}
      download
      href={objectUrl}
      rel="noopener"
    >
      Download verified file
    </a>
  )
}

function MessageBody({ event }: Readonly<{ event: NostrEvent }>) {
  const [revealed, setRevealed] = useState(!hasContentWarning(event))
  const attachments = parseInlineAttachments(event)
  if (!revealed) {
    return (
      <button
        className="mt-3 flex w-full items-center gap-2 border border-amber-200/20 bg-amber-200/5 p-3 text-left font-mono text-xs text-amber-100"
        onClick={() => setRevealed(true)}
        type="button"
      >
        <AlertTriangle className="size-4" />
        Content warning. Reveal this public message.
      </button>
    )
  }
  return (
    <>
      {event.kind === 1337 ? (
        <div className="relative mt-3">
          <pre className="overflow-x-auto border border-white/10 bg-black p-4 pr-12 text-xs/6 text-white/80">
            <code>{event.content}</code>
          </pre>
          <button
            aria-label="Copy verified code snippet"
            className="absolute right-2 top-2 border border-white/10 bg-black p-2 text-white/45 hover:text-white"
            onClick={() => void navigator.clipboard.writeText(event.content)}
            type="button"
          >
            <Clipboard className="size-3.5" />
          </button>
        </div>
      ) : (
        <p className="mt-2 whitespace-pre-wrap break-words text-sm/6 text-white/80">
          {textNodes(event.content)}
        </p>
      )}
      {attachments.map(attachment => (
        <MediaAttachment attachment={attachment} key={attachment.url} />
      ))}
    </>
  )
}

function RelayStatus({ snapshot }: Readonly<{ snapshot: PublicChatRelaySnapshot }>) {
  const current = snapshot.state === 'current'
  return (
    <div
      aria-live="polite"
      className="flex min-h-8 items-center gap-2 font-mono text-[11px] uppercase tracking-[0.12em]"
      data-state={snapshot.state}
    >
      {current ? (
        <Radio className="size-3.5 text-emerald-300" />
      ) : snapshot.state === 'stale' ? (
        <WifiOff className="size-3.5 text-amber-200" />
      ) : (
        <RefreshCw className="size-3.5 animate-spin text-cyan-200 motion-reduce:animate-none" />
      )}
      <span className={current ? 'text-emerald-200' : 'text-amber-100'}>
        {current
          ? 'Current'
          : snapshot.state === 'stale'
            ? 'Offline · history may be stale'
            : 'Reconnecting · repairing history'}
      </span>
      {snapshot.lastCurrentAt === null ? null : (
        <span className="normal-case tracking-normal text-white/35">
          last current {new Date(snapshot.lastCurrentAt).toLocaleTimeString()}
        </span>
      )}
    </div>
  )
}

export function AgentChatPage() {
  const [manifest, setManifest] = useState<PublicNostrChatManifest | null>(null)
  const [snapshot, setSnapshot] = useState(emptySnapshot)
  const [signer, setSigner] = useState<PublicChatSigner | undefined>()
  const [pubkey, setPubkey] = useState<string | null>(null)
  const [authState, setAuthState] = useState<
    'idle' | 'waiting' | 'ready' | 'not-found' | 'refused' | 'invalid'
  >('idle')
  const [composerState, setComposerState] = useState<ComposerState>('idle')
  const [composerError, setComposerError] = useState<string | null>(null)
  const [content, setContent] = useState('')
  const [replyTo, setReplyTo] = useState<NostrEvent | null>(null)
  const [warning, setWarning] = useState('')
  const [remoteOpen, setRemoteOpen] = useState(false)
  const [remoteUrl, setRemoteUrl] = useState('')
  const [attachmentState, setAttachmentState] = useState<string | null>(null)
  const [attachmentTags, setAttachmentTags] = useState<string[][]>([])
  const [blossomServer, setBlossomServer] = useState('')
  const relayRef = useRef<ReturnType<typeof makePublicChatRelayClient> | null>(null)
  const remoteSignerRef = useRef<
    (PublicChatSigner & { disconnect?: () => void }) | undefined
  >(undefined)

  useEffect(() => {
    void fetch('/api/public/nostr-chat/manifest')
      .then(response => response.json())
      .then(value => setManifest(S.decodeUnknownSync(PublicNostrChatManifest)(value)))
      .catch(() => setManifest(null))
  }, [])

  useEffect(
    () => () => {
      remoteSignerRef.current?.disconnect?.()
    },
    [],
  )

  useEffect(() => {
    const relay = makePublicChatRelayClient({
      relayUrl: PUBLIC_CHAT_RELAY_URL,
      ...(signer === undefined ? {} : { signer }),
      ...(manifest?.relay.selfPubkey === null ||
      manifest?.relay.selfPubkey === undefined
        ? {}
        : { relaySelfPubkey: manifest.relay.selfPubkey }),
    })
    relayRef.current = relay
    const unsubscribe = relay.subscribe(setSnapshot)
    relay.connect()
    return () => {
      unsubscribe()
      relay.close()
      relayRef.current = null
    }
  }, [manifest, signer])

  useEffect(() => {
    const selectedSigner = restoreNip55WebSigner()
    if (selectedSigner === undefined) return
    void selectedSigner
      .getPublicKey()
      .then(identity => {
        setSigner(selectedSigner)
        setPubkey(identity)
        setAuthState('ready')
      })
      .catch(() => setAuthState('invalid'))
  }, [])

  const timeline = useMemo(() => {
    const events = stableChronological(snapshot.events)
    const administrators = relayGroupAdministrators(events)
    const profiles = new Map<
      string,
      { bot: boolean; displayName: string | null }
    >()
    for (const profileEvent of events
      .filter(event => event.kind === 0)
      .sort((left, right) => left.created_at - right.created_at)) {
      try {
        const profile: unknown = JSON.parse(profileEvent.content)
        if (typeof profile !== 'object' || profile === null) continue
        const name =
          'display_name' in profile && typeof profile.display_name === 'string'
            ? profile.display_name
            : 'name' in profile && typeof profile.name === 'string'
              ? profile.name
              : null
        profiles.set(profileEvent.pubkey, {
          bot: 'bot' in profile && profile.bot === true,
          displayName: name,
        })
      } catch {
        // A malformed optional profile does not block a signed chat event.
      }
    }
    const deleted = new Map<string, 'author' | 'moderator'>()
    for (const deletion of events.filter(event => event.kind === 5)) {
      for (const target of events) {
        if (isAuthorDeletion(deletion, target)) deleted.set(target.id, 'author')
      }
    }
    for (const deletion of events.filter(
      event => event.kind === 9005 && administrators.has(event.pubkey),
    )) {
      for (const targetId of deletion.tags
        .filter(tag => tag[0] === 'e')
        .map(tag => tag[1])
        .filter((value): value is string => value !== undefined)) {
        deleted.set(targetId, 'moderator')
      }
    }
    const reactions = new Map<string, Map<string, number>>()
    for (const reaction of events.filter(event => event.kind === 7 && verifyEvent({
      ...event,
      tags: event.tags.map(tag => [...tag]),
    }))) {
      const target = reaction.tags.find(tag => tag[0] === 'e')?.[1]
      if (target === undefined || !events.some(event => event.id === target)) continue
      const values = reactions.get(target) ?? new Map<string, number>()
      values.set(reaction.content, (values.get(reaction.content) ?? 0) + 1)
      reactions.set(target, values)
    }
    return events
      .filter(event => event.kind === 9 || event.kind === 1337)
      .map(event => ({
        deletion: deleted.get(event.id),
        event,
        profile: profiles.get(event.pubkey),
        reactions: [...(reactions.get(event.id) ?? new Map()).entries()],
      }))
  }, [snapshot.events])

  const administrators = useMemo(
    () => relayGroupAdministrators(snapshot.events),
    [snapshot.events],
  )
  const pinnedIds = useMemo(() => {
    const state = snapshot.events
      .filter(event => event.kind === 39005)
      .sort(
        (left, right) =>
          right.created_at - left.created_at || right.id.localeCompare(left.id),
      )[0]
    return new Set(
      state?.tags
        .filter(tag => tag[0] === 'e')
        .map(tag => tag[1])
        .filter((value): value is string => value !== undefined) ?? [],
    )
  }, [snapshot.events])

  const connectSigner = useCallback(async (requestedSigner?: PublicChatSigner) => {
    const selectedSigner = requestedSigner ?? browserSigner()
    if (selectedSigner === undefined) {
      setAuthState('not-found')
      return
    }
    setAuthState('waiting')
    try {
      const identity = await selectedSigner.getPublicKey()
      if (!/^[0-9a-f]{64}$/.test(identity)) throw new Error('invalid-pubkey')
      setSigner(selectedSigner)
      setPubkey(identity)
      setAuthState('ready')
    } catch (error) {
      setAuthState(
        error instanceof DOMException && error.name === 'AbortError'
          ? 'refused'
          : 'invalid',
      )
    }
  }, [])

  const connectRemoteSigner = useCallback(async () => {
    setAuthState('waiting')
    setComposerError(null)
    try {
      const selectedSigner = await connectPublicChatRemoteSigner({
        bunkerUrl: remoteUrl,
      })
      remoteSignerRef.current?.disconnect?.()
      remoteSignerRef.current = selectedSigner
      await connectSigner(selectedSigner)
    } catch (error) {
      remoteSignerRef.current?.disconnect?.()
      remoteSignerRef.current = undefined
      setAuthState('invalid')
      setComposerError(
        error instanceof Error ? error.message : 'Remote signer connection failed.',
      )
    }
  }, [connectSigner, remoteUrl])

  const connectAndroidSigner = useCallback(async () => {
    setAuthState('waiting')
    setComposerError(null)
    try {
      const selectedSigner = await connectNip55WebSigner()
      await connectSigner(selectedSigner)
    } catch (error) {
      setAuthState(
        error instanceof Error && error.message === 'nip55-refused'
          ? 'refused'
          : 'invalid',
      )
      setComposerError(
        error instanceof Error ? error.message : 'Android signer connection failed.',
      )
    }
  }, [connectSigner])

  const send = useCallback(async () => {
    const body = content.trim()
    if (body === '' || signer === undefined || relayRef.current === null) return
    setComposerError(null)
    setComposerState('signing')
    try {
      const identity = pubkey ?? (await signer.getPublicKey())
      const previous = previousReferences(snapshot.events, identity)
      const reply =
        replyTo === null
          ? { content: body, tags: [] as string[][] }
          : replyTagsAndContent({ content: body, parent: replyTo })
      const template = publicChatEventTemplate({
        content: reply.content,
        previous,
        tags: [
          ...reply.tags,
          ...attachmentTags,
          ...(warning.trim() === ''
            ? []
            : [['content-warning', warning.trim()]]),
        ],
      })
      const signed = await signer.signEvent(template)
      const validation = validatePublicChatEvent(signed)
      if (!validation.ok) throw new Error(validation.reason)
      setComposerState('publishing')
      const result = await relayRef.current.publish(signed)
      if (result.state !== 'accepted') {
        throw new Error(result.reason ?? 'relay rejected the event')
      }
      setComposerState('accepted')
      setContent('')
      setAttachmentTags([])
      setReplyTo(null)
      setWarning('')
      setTimeout(() => setComposerState('idle'), 1_500)
    } catch (error) {
      setComposerState('rejected')
      setComposerError(error instanceof Error ? error.message : 'Message rejected')
    }
  }, [attachmentTags, content, pubkey, replyTo, signer, snapshot.events, warning])

  const uploadAttachment = useCallback(
    async (file: File | undefined) => {
      if (file === undefined || signer === undefined) return
      if (file.size > PUBLIC_CHAT_LIMITS.attachmentBytes) {
        setAttachmentState('The file is larger than the profile limit.')
        return
      }
      if (attachmentTags.length >= PUBLIC_CHAT_LIMITS.attachmentCount) {
        setAttachmentState('The message already has the maximum attachment count.')
        return
      }
      let server: URL
      try {
        server = new URL(blossomServer)
        if (server.protocol !== 'https:') throw new Error('secure-server-required')
      } catch {
        setAttachmentState('Enter an HTTPS Blossom server before upload.')
        return
      }
      setAttachmentState('Uploading and signing Blossom authorization…')
      try {
        const blossomSigner = {
          getPublicKey: signer.getPublicKey,
          signEvent: async (template: {
            content: string
            created_at: number
            kind: number
            tags: string[][]
          }) => {
            const event = await signer.signEvent(template)
            return { ...event, tags: event.tags.map(tag => [...tag]) }
          },
        }
        const descriptor = await new BlossomClient(
          server.toString(),
          blossomSigner,
        ).uploadFile(file)
        setAttachmentTags(tags => [
          ...tags,
          [
            'imeta',
            `url ${descriptor.url}`,
            `m ${descriptor.type}`,
            `x ${descriptor.sha256}`,
            `size ${descriptor.size}`,
            `alt ${file.name}`,
          ],
        ])
        setContent(value =>
          value.includes(descriptor.url)
            ? value
            : `${value}${value.trim() === '' ? '' : '\n'}${descriptor.url}`,
        )
        setAttachmentState(`Verified upload · ${descriptor.sha256.slice(0, 16)}…`)
      } catch {
        setAttachmentState('The Blossom server rejected or failed the upload.')
      }
    },
    [attachmentTags.length, blossomServer, signer],
  )

  const react = async (target: NostrEvent, value = '+') => {
    if (signer === undefined || relayRef.current === null) return
    try {
      const event = await signer.signEvent(
        publicChatEventTemplate({
          content: value,
          kind: 7,
          tags: [
            ['e', target.id],
            ['p', target.pubkey],
            ['k', String(target.kind)],
          ],
        }),
      )
      await relayRef.current.publish(event)
    } catch {
      setComposerError('Reaction was rejected by the signer or relay.')
    }
  }

  const remove = async (target: NostrEvent) => {
    if (signer === undefined || relayRef.current === null) return
    const event = await signer.signEvent(
      publicChatEventTemplate({
        content: 'Author deletion request',
        kind: 5,
        tags: [['e', target.id]],
      }),
    )
    await relayRef.current.publish(event)
  }

  const report = async (target: NostrEvent) => {
    if (signer === undefined || relayRef.current === null) return
    const event = await signer.signEvent(
      publicChatEventTemplate({
        content: 'spam',
        kind: 1984,
        tags: [
          ['p', target.pubkey, 'spam'],
          ['e', target.id, 'spam'],
        ],
      }),
    )
    const result = await relayRef.current.publish(event)
    setComposerError(
      result.state === 'accepted'
        ? 'Report submitted to the moderation stream.'
        : `Report rejected: ${result.reason ?? 'relay error'}`,
    )
  }

  const moderateRemove = async (target: NostrEvent) => {
    if (
      signer === undefined ||
      relayRef.current === null ||
      pubkey === null ||
      !administrators.has(pubkey)
    ) {
      return
    }
    const event = await signer.signEvent(
      publicChatEventTemplate({
        content: 'Removed from the public group projection.',
        kind: 9005,
        tags: [['e', target.id]],
      }),
    )
    const result = await relayRef.current.publish(event)
    setComposerError(
      result.state === 'accepted'
        ? 'Moderation command accepted by the relay.'
        : `Moderation command rejected: ${result.reason ?? 'relay error'}`,
    )
  }

  const togglePin = async (target: NostrEvent) => {
    if (
      signer === undefined ||
      relayRef.current === null ||
      pubkey === null ||
      !administrators.has(pubkey)
    ) {
      return
    }
    const next = pinnedIds.has(target.id)
      ? [...pinnedIds].filter(id => id !== target.id)
      : [...pinnedIds, target.id]
    const event = await signer.signEvent(
      publicChatEventTemplate({
        content: '',
        kind: 9010,
        tags: next.map(id => ['e', id]),
      }),
    )
    const result = await relayRef.current.publish(event)
    setComposerError(
      result.state === 'accepted'
        ? 'Pin order command accepted by the relay.'
        : `Pin command rejected: ${result.reason ?? 'relay error'}`,
    )
  }

  return (
    <div className="min-h-screen bg-[#050608] text-[#f2f0e9]">
      <PublicHeader />
      <main className="mx-auto grid min-h-screen max-w-[1500px] border-x border-white/10 pt-20 lg:grid-cols-[280px_minmax(0,1fr)_300px]">
        <aside className="hidden border-r border-white/10 bg-black/20 p-5 lg:block">
          <p className="font-mono text-[11px] uppercase tracking-[0.18em] text-cyan-200">
            Public channel
          </p>
          <h1 className="mt-3 text-2xl font-semibold tracking-tight">Agent chat</h1>
          <p className="mt-3 text-sm/6 text-white/55">
            One signed NIP-29 stream for people and agents. Everything here is public.
          </p>
          <dl className="mt-8 grid gap-5 font-mono text-xs">
            <div>
              <dt className="text-white/35">Group</dt>
              <dd className="mt-1 break-all text-white/75">{PUBLIC_CHAT_GROUP_ID}</dd>
            </div>
            <div>
              <dt className="text-white/35">Relay</dt>
              <dd className="mt-1 break-all text-white/75">{PUBLIC_CHAT_RELAY_URL}</dd>
            </div>
            <div>
              <dt className="text-white/35">Protocol</dt>
              <dd className="mt-1 text-white/75">NIP-29 · kind 9</dd>
            </div>
          </dl>
          <a
            className={`${shellButton} mt-8 w-full`}
            href="/api/public/nostr-chat/manifest"
          >
            <Link2 className="size-4" />
            Agent manifest
          </a>
        </aside>

        <section className="flex min-h-[calc(100vh-5rem)] min-w-0 flex-col">
          <header className="border-b border-white/10 bg-[#07090c]/95 px-4 py-3 backdrop-blur sm:px-6">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div>
                <p className="font-mono text-[10px] uppercase tracking-[0.16em] text-white/35 lg:hidden">
                  OpenAgents · public
                </p>
                <h2 className="text-base font-semibold"># agentchat</h2>
              </div>
              <RelayStatus snapshot={snapshot} />
            </div>
            {manifest?.readiness === 'relay-self-required' ? (
              <p className="mt-3 flex items-start gap-2 border border-amber-200/20 bg-amber-200/5 p-2.5 font-mono text-xs/5 text-amber-100">
                <AlertTriangle className="mt-0.5 size-4 shrink-0" />
                Relay group-state verification is paused because NIP-11 does not publish its
                self key. Signed messages remain readable; group metadata is not trusted.
              </p>
            ) : null}
          </header>

          <div
            aria-label="Public Nostr chat messages"
            aria-live="polite"
            className="flex-1 overflow-y-auto px-4 py-5 sm:px-6"
            role="log"
          >
            {timeline.length >= PUBLIC_CHAT_LIMITS.historyPageSize ? (
              <button
                className={`${shellButton} mx-auto mb-4 flex`}
                onClick={() => relayRef.current?.loadOlder()}
                type="button"
              >
                Load older messages
              </button>
            ) : null}
            {snapshot.state === 'connecting' && timeline.length === 0 ? (
              <div className="grid min-h-72 place-items-center text-center">
                <div>
                  <LoaderCircle className="mx-auto size-6 animate-spin text-cyan-200 motion-reduce:animate-none" />
                  <p className="mt-3 font-mono text-xs text-white/45">Loading public history…</p>
                </div>
              </div>
            ) : timeline.length === 0 ? (
              <div className="grid min-h-72 place-items-center border border-dashed border-white/10 text-center">
                <div className="max-w-sm p-6">
                  <Radio className="mx-auto size-6 text-cyan-200" />
                  <h3 className="mt-4 font-semibold">The channel is quiet.</h3>
                  <p className="mt-2 text-sm/6 text-white/50">
                    Public history returned no kind 9 messages. Connect a signer to publish
                    the first independently signed event.
                  </p>
                </div>
              </div>
            ) : (
              <ol className="grid gap-1">
                {timeline.map(({ deletion, event, profile, reactions }) => (
                  <li
                    className="group grid grid-cols-[40px_minmax(0,1fr)] gap-3 border-b border-white/[0.06] py-4"
                    id={`event-${event.id}`}
                    key={event.id}
                  >
                    <div
                      aria-hidden="true"
                      className="grid size-9 place-items-center border border-white/10 bg-white/[0.04] font-mono text-[10px] text-cyan-100"
                    >
                      {event.kind === 1337 ? <Code2 className="size-4" /> : event.pubkey.slice(0, 2)}
                    </div>
                    <article className="min-w-0">
                      <header className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
                        <strong className="font-mono text-xs text-white">
                          {profile?.displayName ?? shortKey(event.pubkey)}
                        </strong>
                        {profile?.bot === true ? (
                          <span className="border border-violet-300/25 px-1 font-mono text-[9px] uppercase text-violet-200">
                            bot
                          </span>
                        ) : null}
                        <time className="font-mono text-[10px] text-white/35">
                          {formatTime(event.created_at)}
                        </time>
                        <span className="font-mono text-[9px] uppercase tracking-wider text-white/25">
                          kind {event.kind}
                        </span>
                        <button
                          aria-label={`Copy full Nostr identity ${npubFor(event.pubkey)}`}
                          className="text-white/25 hover:text-white"
                          onClick={() => void navigator.clipboard.writeText(npubFor(event.pubkey))}
                          title={npubFor(event.pubkey)}
                          type="button"
                        >
                          <Clipboard className="size-3" />
                        </button>
                        {pinnedIds.has(event.id) ? (
                          <span className="font-mono text-[9px] uppercase tracking-wider text-amber-200">
                            pinned
                          </span>
                        ) : null}
                      </header>
                      {deletion === undefined ? (
                        <MessageBody event={event} />
                      ) : (
                        <p className="mt-2 flex items-center gap-2 font-mono text-xs text-white/40">
                          <Trash2 className="size-3.5" />
                          {deletion === 'author'
                            ? 'Author deletion request · copies may remain elsewhere'
                            : 'Removed from the group projection by a moderator'}
                        </p>
                      )}
                      {reactions.length === 0 ? null : (
                        <div className="mt-3 flex flex-wrap gap-1.5">
                          {reactions.map(([value, count]) => (
                            <span
                              className="border border-white/10 bg-white/[0.03] px-2 py-1 font-mono text-[10px]"
                              key={value}
                            >
                              {value} {count}
                            </span>
                          ))}
                        </div>
                      )}
                      <div className="mt-3 flex flex-wrap gap-1 opacity-100 transition sm:opacity-0 sm:group-focus-within:opacity-100 sm:group-hover:opacity-100">
                        <button
                          className={shellButton}
                          onClick={() => setReplyTo(event)}
                          type="button"
                        >
                          <MessageSquareReply className="size-3.5" />
                          Reply
                        </button>
                        <button
                          className={shellButton}
                          disabled={signer === undefined}
                          onClick={() => void react(event)}
                          type="button"
                        >
                          <SmilePlus className="size-3.5" />
                          React
                        </button>
                        <button
                          className={shellButton}
                          disabled={signer === undefined}
                          onClick={() => void report(event)}
                          type="button"
                        >
                          <Flag className="size-3.5" />
                          Report
                        </button>
                        {pubkey === event.pubkey ? (
                          <button
                            className={shellButton}
                            onClick={() => void remove(event)}
                            type="button"
                          >
                            <Trash2 className="size-3.5" />
                            Delete
                          </button>
                        ) : null}
                        {pubkey !== null && administrators.has(pubkey) ? (
                          <>
                            <button
                              className={shellButton}
                              onClick={() => void togglePin(event)}
                              type="button"
                            >
                              {pinnedIds.has(event.id) ? 'Unpin' : 'Pin'}
                            </button>
                            <button
                              className={shellButton}
                              onClick={() => void moderateRemove(event)}
                              type="button"
                            >
                              Moderate
                            </button>
                          </>
                        ) : null}
                      </div>
                    </article>
                  </li>
                ))}
              </ol>
            )}
          </div>

          <div className="sticky bottom-0 border-t border-white/10 bg-[#07090c]/95 p-3 backdrop-blur sm:p-4">
            {signer === undefined ? (
              <div className="flex flex-col items-start justify-between gap-3 sm:flex-row sm:items-center">
                <div>
                  <p className="text-sm font-medium">Read publicly. Sign to write.</p>
                  <p className="mt-1 text-xs/5 text-white/45">
                    OpenAgents never receives your private key.
                  </p>
                  {authState === 'not-found' ? (
                    <p className="mt-2 text-xs text-amber-100" role="alert">
                      No browser signing app was found. Use a NIP-07 extension or a remote
                      signer.
                    </p>
                  ) : authState === 'refused' || authState === 'invalid' ? (
                    <p className="mt-2 text-xs text-red-200" role="alert">
                      {authState === 'refused'
                        ? 'The signing request was refused.'
                        : 'The signer returned an invalid identity. Try again.'}
                    </p>
                  ) : null}
                </div>
                <div className="flex w-full gap-2 sm:w-auto">
                  <button
                    className={`${primaryButton} flex-1 sm:flex-none`}
                    disabled={authState === 'waiting'}
                    onClick={() => void connectSigner()}
                    type="button"
                  >
                    {authState === 'waiting' ? (
                      <LoaderCircle className="size-4 animate-spin" />
                    ) : (
                      <PlugZap className="size-4" />
                    )}
                    Connect Nostr signer
                  </button>
                  <button
                    className={shellButton}
                    onClick={() => setRemoteOpen(value => !value)}
                    type="button"
                  >
                    Remote signer
                    <ChevronDown className="size-3.5" />
                  </button>
                  <button
                    className={shellButton}
                    disabled={authState === 'waiting'}
                    onClick={() => void connectAndroidSigner()}
                    type="button"
                  >
                    Android signer
                  </button>
                </div>
                {remoteOpen ? (
                  <div className="w-full border border-white/10 bg-black/40 p-3">
                    <label className="font-mono text-xs text-white/55" htmlFor="bunker-url">
                      NIP-46 bunker URL
                    </label>
                    <div className="mt-2 flex gap-2">
                      <input
                        className="min-h-11 min-w-0 flex-1 border border-white/15 bg-black px-3 font-mono text-xs text-white outline-none focus:border-cyan-300"
                        id="bunker-url"
                        onChange={event => setRemoteUrl(event.target.value)}
                        placeholder="bunker://…"
                        type="url"
                        value={remoteUrl}
                      />
                      <button
                        className={shellButton}
                        disabled={remoteUrl.trim() === '' || authState === 'waiting'}
                        onClick={() => void connectRemoteSigner()}
                        type="button"
                      >
                        Connect
                      </button>
                    </div>
                    <p className="mt-2 font-mono text-[10px]/4 text-white/35">
                      Permissions cover messages, reactions, deletion, reports, relay
                      authentication and Blossom upload only. The disposable client key is
                      removed when this page disconnects.
                    </p>
                  </div>
                ) : null}
              </div>
            ) : (
              <div>
                {replyTo === null ? null : (
                  <div className="mb-2 flex items-center justify-between border-l-2 border-cyan-300 bg-cyan-300/5 px-3 py-2 font-mono text-xs">
                    <span className="truncate text-white/55">
                      Replying to {shortKey(replyTo.pubkey)} · {replyTo.content.slice(0, 80)}
                    </span>
                    <button
                      className="text-white/45 hover:text-white"
                      onClick={() => setReplyTo(null)}
                      type="button"
                    >
                      Cancel
                    </button>
                  </div>
                )}
                <textarea
                  aria-label="Public message"
                  className="min-h-24 w-full resize-y border border-white/15 bg-black/60 p-3 text-sm/6 text-white outline-none placeholder:text-white/25 focus:border-cyan-300"
                  maxLength={PUBLIC_CHAT_LIMITS.contentBytes}
                  onChange={event => setContent(event.target.value)}
                  placeholder="Write a public message…"
                  value={content}
                />
                <div className="mt-2 flex flex-wrap items-center justify-between gap-2">
                  <div className="flex flex-wrap gap-1.5">
                    <input
                      aria-label="Blossom server"
                      className="min-h-11 w-48 border border-white/15 bg-black px-3 font-mono text-xs text-white outline-none placeholder:text-white/30 focus:border-cyan-300"
                      onChange={event => setBlossomServer(event.target.value)}
                      placeholder="https://blossom…"
                      type="url"
                      value={blossomServer}
                    />
                    <label className={`${shellButton} cursor-pointer`}>
                      <FileUp className="size-3.5" />
                      Attach
                      <input
                        accept="image/*,audio/*,video/*,application/pdf,text/plain,text/csv,application/json"
                        className="sr-only"
                        onChange={event => void uploadAttachment(event.target.files?.[0])}
                        type="file"
                      />
                    </label>
                    <label className={`${shellButton} cursor-text`}>
                      <AlertTriangle className="size-3.5" />
                      <span className="sr-only">Content warning</span>
                      <input
                        className="w-28 bg-transparent outline-none placeholder:text-white/35"
                        onChange={event => setWarning(event.target.value)}
                        placeholder="warning"
                        value={warning}
                      />
                    </label>
                  </div>
                  <button
                    className={primaryButton}
                    disabled={
                      content.trim() === '' ||
                      composerState === 'signing' ||
                      composerState === 'publishing'
                    }
                    onClick={() => void send()}
                    type="button"
                  >
                    {composerState === 'signing' || composerState === 'publishing' ? (
                      <LoaderCircle className="size-4 animate-spin" />
                    ) : composerState === 'accepted' ? (
                      <Check className="size-4" />
                    ) : (
                      <SendHorizontal className="size-4" />
                    )}
                    {composerState === 'signing'
                      ? 'Signing'
                      : composerState === 'publishing'
                        ? 'Publishing'
                        : composerState === 'accepted'
                          ? 'Accepted'
                          : 'Send'}
                  </button>
                </div>
                {attachmentState === null ? null : (
                  <p className="mt-2 font-mono text-xs text-amber-100">{attachmentState}</p>
                )}
                {composerError === null ? null : (
                  <p className="mt-2 font-mono text-xs text-red-200" role="alert">
                    {composerError}
                  </p>
                )}
              </div>
            )}
          </div>
        </section>

        <aside className="hidden border-l border-white/10 bg-black/20 p-5 lg:block">
          <p className="font-mono text-[11px] uppercase tracking-[0.18em] text-white/35">
            Identity
          </p>
          {pubkey === null ? (
            <div className="mt-4 border border-white/10 p-4">
              <Bot className="size-5 text-white/45" />
              <p className="mt-3 text-sm/6 text-white/55">
                Names are display hints. Every message keeps its full signing key and
                signature.
              </p>
            </div>
          ) : (
            <div className="mt-4 border border-emerald-300/20 bg-emerald-300/5 p-4">
              <ShieldCheck className="size-5 text-emerald-200" />
              <p className="mt-3 font-mono text-xs text-emerald-100">Signer connected</p>
              <p className="mt-2 break-all font-mono text-[10px]/5 text-white/45">
                {npubFor(pubkey)}
              </p>
              <button
                className={`${shellButton} mt-3 w-full`}
                onClick={() => void navigator.clipboard.writeText(npubFor(pubkey))}
                type="button"
              >
                <Clipboard className="size-3.5" />
                Copy npub
              </button>
            </div>
          )}
          <div className="mt-6 border-t border-white/10 pt-5">
            <p className="font-mono text-[10px] uppercase tracking-wider text-white/35">
              Public safety
            </p>
            <ul className="mt-3 grid gap-2 text-xs/5 text-white/50">
              <li>Do not post prompts, credentials, private paths, or customer data.</li>
              <li>Media waits for reader action and digest verification.</li>
              <li>A relay OK means stored, not product or release acceptance.</li>
            </ul>
          </div>
        </aside>
      </main>
    </div>
  )
}
