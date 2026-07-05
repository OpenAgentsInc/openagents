import {
  CHAT_MESSAGE_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeRuntimeEventEntity,
  decodeRuntimeTurnEntity,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
  threadScope,
  type ChatMessageEntity,
  type KhalaRuntimeLane,
  type RuntimeEventEntity,
  type RuntimeTurnEntity
} from "@openagentsinc/khala-sync"
import * as Clipboard from "expo-clipboard"
import { useLocalSearchParams } from "expo-router"
import { useEffect, useMemo, useRef, useState } from "react"
import { FlatList, KeyboardAvoidingView, Text, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { AppHeader } from "../../src/components/app-header"
import { type PopupOptionType, TouchablePopupHandler } from "../../src/components/blurred-popup"
import { ChatComposer, chatComposerKeyboardVerticalOffset } from "../../src/components/chat-composer"
import { SwipeableItem } from "../../src/components/swipeable-item"
import { TranscriptPartRow } from "../../src/components/transcript-part-row"
import { buildCopyMarkdown, buildCopyText } from "../../src/sync/blurred-popup-menu-core"
import { buildHandoffPromptBody, summarizeTurnEventsForHandoff } from "../../src/sync/khala-cross-agent-handoff-core"
import {
  buildChatAppendMessageArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  findActiveTurn,
  mostRecentTurnLane
} from "../../src/sync/khala-runtime-compose-core"
import { sortByKeyAsc } from "../../src/sync/khala-sync-entities-core"
import { makeSafeRef } from "../../src/sync/khala-sync-push-core"
import {
  reduceRuntimeTranscript,
  sortEventsBySequence,
  type TranscriptPart
} from "../../src/sync/khala-runtime-transcript-core"
import { buildQuoteSnippet } from "../../src/sync/swipe-quote-core"
import { useKhalaSyncCollection } from "../../src/sync/use-khala-sync-collection"
import { useKhalaSyncPush } from "../../src/sync/use-khala-sync-push"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../../src/theme/motion"

/** Builds the long-press "Blurred Popup" menu (issue #8395) for one
 * quotable transcript part: "Copy" (plain text), "Copy as Markdown" (only
 * shown when it differs from the plain-text payload — a plain `text` part
 * has no markdown decoration, so showing both would be a redundant no-op
 * duplicate item), and "Quote" (reuses the exact same swipe-to-quote action
 * `SwipeableItem` already wires up (#8393), so long-press and swipe both
 * feed the same `onQuote` callback rather than building a second quoting
 * mechanism). "Re-run a tool call" (named in the issue) was considered and
 * dropped: `useKhalaSyncPush`'s mutation surface only has
 * `chat.appendUserMessage`/`runtime.startTurn`/`runtime.interruptTurn` — no
 * retry/rerun mutation exists to wire a real menu item to, and the issue is
 * explicit that a non-wireable action should be skipped rather than faked. */
const buildTranscriptPartPopupOptions = (
  part: TranscriptPart,
  onQuote: () => void
): ReadonlyArray<PopupOptionType> => {
  const copyText = buildCopyText(part)
  const copyMarkdown = buildCopyMarkdown(part)
  const options: Array<PopupOptionType> = []
  if (copyText !== undefined) {
    options.push({ label: "Copy", onPress: () => void Clipboard.setStringAsync(copyText) })
  }
  if (copyMarkdown !== undefined && copyMarkdown !== copyText) {
    options.push({ label: "Copy as Markdown", onPress: () => void Clipboard.setStringAsync(copyMarkdown) })
  }
  options.push({ label: "Quote", onPress: onQuote })
  return options
}

/** One pending swipe-to-quote request, handed to `ChatComposer` (see issue
 * #8393). `id` is the swiped transcript part's own id — reused as the
 * composer's dedupe key so re-renders here can never cause a double-merge. */
type QuoteRequest = Readonly<{ id: string; snippet: string }>

// `reduceRuntimeTranscript` re-folds the FULL event list on every render, but
// it's a deterministic left-to-right fold over an append-only event log, so
// previously-produced parts keep the exact same `id` (and position) across
// recomputations — only the newly appended tail is new. Combined with
// `keyExtractor={part => part.id}`, React/FlatList reuse the same row
// component instance for every already-rendered part (no remount), so
// Reanimated's `entering=` (which only fires on a component's first mount)
// naturally animates ONLY newly-appended parts, not the whole list, on every
// streaming update. See `khala-runtime-transcript-core.test.ts` for a
// regression test asserting that id-stability property.
//
// The per-row stagger delay is capped (rather than `index * STAGGER_MS`
// uncapped) because `index` here is the item's absolute position in a
// potentially long-running thread: an uncapped delay would make a part
// appended at index 300 wait 18s+ before fading in. Capping bounds the delay
// to a small, still-cascading amount for the common "many parts mount at
// once on initial thread load" case while staying snappy for the common
// "one part streams in at a time" case.
const TRANSCRIPT_STAGGER_CAP = 8
const transcriptEntranceDelay = (index: number): number =>
  MOTION_STAGGER_MS * Math.min(index, TRANSCRIPT_STAGGER_CAP)

const messageIdOf = (message: ChatMessageEntity): string => message.messageId
const createdAtOf = (message: ChatMessageEntity): string => message.createdAt
const runtimeEventIdOf = (event: RuntimeEventEntity): string => event.eventId
const runtimeTurnIdOf = (turn: RuntimeTurnEntity): string => turn.turnId

const formatClockTime = (iso: string): string => {
  const parsed = new Date(iso)
  if (Number.isNaN(parsed.getTime())) return iso
  return parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
}

export default function ThreadMessagesScreen() {
  const { threadId, title } = useLocalSearchParams<{ threadId: string; title?: string }>()
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- shared scroll ref across two independently-typed FlatLists (chat vs. transcript)
  const listRef = useRef<FlatList<any>>(null)
  const scope = threadId === undefined ? "" : String(threadScope(threadId))

  const chatState = useKhalaSyncCollection(
    scope,
    CHAT_MESSAGE_ENTITY_TYPE,
    decodeChatMessageEntity,
    messageIdOf
  )
  const runtimeState = useKhalaSyncCollection(
    scope,
    RUNTIME_EVENT_ENTITY_TYPE,
    decodeRuntimeEventEntity,
    runtimeEventIdOf
  )
  const turnState = useKhalaSyncCollection(
    scope,
    RUNTIME_TURN_ENTITY_TYPE,
    decodeRuntimeTurnEntity,
    runtimeTurnIdOf
  )
  const activeTurn = useMemo(() => findActiveTurn(turnState.items), [turnState.items])
  const defaultLane = useMemo(() => mostRecentTurnLane(turnState.items), [turnState.items])
  const push = useKhalaSyncPush()
  const [quoteRequest, setQuoteRequest] = useState<QuoteRequest | undefined>(undefined)
  const [handoffPendingTurnId, setHandoffPendingTurnId] = useState<string | undefined>(undefined)
  const [handoffError, setHandoffError] = useState<string | null>(null)

  // "Ask [other provider] to review this" (#8407) — starts a brand-new turn
  // on the OTHER lane, its prompt carrying a bounded summary of the
  // just-completed turn (built by re-folding that turn's OWN events through
  // `summarizeTurnEventsForHandoff`, never the full raw stream). Persists the
  // summary as an ordinary `chat_message` first (same `chatMessageBodyRef`
  // convention the composer's own send flow uses in
  // `khala-runtime-compose-core.ts`), then starts the new turn referencing
  // it — no new schema or mutator, just a second `target.lane`.
  const requestHandoff = async (input: {
    turnId: string
    sourceLane: KhalaRuntimeLane
    targetLane: KhalaRuntimeLane
  }) => {
    if (threadId === undefined || handoffPendingTurnId !== undefined) return
    setHandoffPendingTurnId(input.turnId)
    setHandoffError(null)
    try {
      const turnEvents = sortEventsBySequence(
        runtimeState.items.filter(entity => entity.turnId === input.turnId)
      ).map(entity => entity.event)
      const summary = summarizeTurnEventsForHandoff(turnEvents)
      const body = buildHandoffPromptBody({ sourceLane: input.sourceLane, summary, targetLane: input.targetLane })
      const nowIso = new Date().toISOString()
      const messageId = makeSafeRef("msg")
      const bodyRef = chatMessageBodyRef(messageId)
      const newTurnId = makeSafeRef("turn")
      await push([
        { args: buildChatAppendMessageArgs({ body, messageId, threadId }), name: "chat.appendMessage" },
        {
          args: buildStartTurnIntentArgs({
            bodyRef,
            nowIso,
            target: { lane: input.targetLane },
            threadId,
            turnId: newTurnId
          }),
          name: "runtime.startTurn"
        }
      ])
    } catch (error) {
      setHandoffError(error instanceof Error ? error.message : String(error))
    } finally {
      setHandoffPendingTurnId(undefined)
    }
  }

  const messages = sortByKeyAsc(
    chatState.items.filter(message => message.deletedAt === null),
    createdAtOf
  )
  const transcriptParts = useMemo(
    () =>
      reduceRuntimeTranscript(
        sortEventsBySequence(runtimeState.items).map(entity => entity.event)
      ),
    [runtimeState.items]
  )
  const hasRichTranscript = transcriptParts.length > 0

  useEffect(() => {
    if (messages.length === 0 && transcriptParts.length === 0) return
    listRef.current?.scrollToEnd({ animated: true })
  }, [messages.length, transcriptParts.length])

  const status = hasRichTranscript ? runtimeState.status : chatState.status
  const loading = status === "loading" && messages.length === 0 && transcriptParts.length === 0

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <AppHeader showBack title={title ?? "Thread"} />
      <KeyboardAvoidingView
        behavior={chatComposerKeyboardVerticalOffset === 0 ? "height" : "padding"}
        className="flex-1"
        keyboardVerticalOffset={chatComposerKeyboardVerticalOffset}
      >
        <View className="flex-1">
          {status === "missing_token" ? (
            <View className="flex-1 items-center justify-center px-8">
              <Text className="text-center font-mono text-sm text-textFaint">
                Not signed in. Restart the app to sign in again.
              </Text>
            </View>
          ) : status === "error" ? (
            <View className="flex-1 items-center justify-center px-8">
              <Text className="text-center font-sans text-base text-danger">
                {chatState.error ?? runtimeState.error}
              </Text>
            </View>
          ) : loading ? (
            <View className="flex-1 items-center justify-center">
              <Text className="font-sans text-base text-textMuted">loading messages…</Text>
            </View>
          ) : hasRichTranscript ? (
            <FlatList
              contentContainerClassName="gap-2 px-4 py-4"
              data={transcriptParts}
              keyExtractor={part => part.id}
              ref={listRef}
              renderItem={({ index, item: part }) => {
                const row = (
                  <Animated.View entering={FadeIn.delay(transcriptEntranceDelay(index)).duration(MOTION_MEDIUM)}>
                    <TranscriptPartRow
                      handoffDisabled={activeTurn !== undefined}
                      handoffPending={part.kind === "turn-status" && handoffPendingTurnId === part.turnId}
                      onRequestHandoff={requestHandoff}
                      part={part}
                    />
                  </Animated.View>
                )
                // Only wrap parts with meaningful quotable content (text,
                // reasoning, tool calls) in the swipe-to-quote gesture —
                // `usage`/`turn-status` rows are plain centered divider
                // text with nothing to quote (see `buildQuoteSnippet`).
                const quoteSnippet = buildQuoteSnippet(part)
                if (quoteSnippet === undefined) return row
                const onQuote = () => setQuoteRequest({ id: part.id, snippet: quoteSnippet })
                return (
                  <TouchablePopupHandler options={buildTranscriptPartPopupOptions(part, onQuote)}>
                    <SwipeableItem onSwipeComplete={onQuote}>{row}</SwipeableItem>
                  </TouchablePopupHandler>
                )
              }}
            />
          ) : messages.length === 0 ? (
            <View className="flex-1 items-center justify-center">
              <Text className="font-sans text-base text-textMuted">No messages yet</Text>
            </View>
          ) : (
            <FlatList
              contentContainerClassName="gap-2 px-4 py-4"
              data={messages}
              keyExtractor={message => message.messageId}
              ref={listRef}
              renderItem={({ item: message }) => (
                <View className="rounded-xl border border-border bg-surfaceRaised px-3 py-2">
                  <Text className="font-mono text-xs text-textFaint">
                    {formatClockTime(message.createdAt)}
                  </Text>
                  <Text className="mt-1 font-sans text-base text-text">{message.body}</Text>
                </View>
              )}
            />
          )}
        </View>
        {handoffError === null ? null : (
          <Text className="px-3 pb-1 font-mono text-xs text-danger" numberOfLines={2}>
            {handoffError}
          </Text>
        )}
        {threadId === undefined || status === "missing_token" ? null : (
          <ChatComposer
            activeTurn={activeTurn}
            defaultLane={defaultLane}
            onQuoteConsumed={() => setQuoteRequest(undefined)}
            push={push}
            quoteRequest={quoteRequest}
            threadId={threadId}
          />
        )}
      </KeyboardAvoidingView>
    </SafeAreaView>
  )
}
