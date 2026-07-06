import {
  CHAT_MESSAGE_ENTITY_TYPE,
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeChatThreadEntity,
  decodeRuntimeEventEntity,
  decodeRuntimeTurnEntity,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
  threadScope,
  type ChatMessageEntity,
  type KhalaRuntimeLane,
} from "@openagentsinc/khala-sync"
import * as Clipboard from "expo-clipboard"
import { useEffect, useMemo, useRef, useState } from "react"
import {
  FlatList,
  KeyboardAvoidingView,
  Pressable,
  type NativeScrollEvent,
  type NativeSyntheticEvent,
  View,
} from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { type PopupOptionType, TouchablePopupHandler } from "../components/blurred-popup"
import { ChatComposer, chatComposerKeyboardVerticalOffset } from "../components/chat-composer"
import { CreditsBalanceChip } from "../components/credits-balance-chip"
import { KhalaEmptyState } from "../components/khala-empty-state"
import { KhalaScrollToLatestButton } from "../components/khala-scroll-to-latest-button"
import { KhalaText } from "../components/khala-text"
import { KhalaThreadHeader } from "../components/khala-thread-header"
import { SwipeableItem } from "../components/swipeable-item"
import { TranscriptPartRow } from "../components/transcript-part-row"
import type { AppStackScreenProps } from "../navigators/navigationTypes"
import { buildCopyMarkdown, buildCopyText } from "../sync/blurred-popup-menu-core"
import { buildHandoffPromptBody, summarizeTurnEventsForHandoff } from "../sync/khala-cross-agent-handoff-core"
import {
  buildChatAppendMessageArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  findActiveTurn,
  mostRecentTurnLane,
} from "../sync/khala-runtime-compose-core"
import { sortByKeyAsc } from "../sync/khala-sync-entities-core"
import { makeSafeRef } from "../sync/khala-sync-push-core"
import {
  reduceRuntimeTranscript,
  sortEventsBySequence,
  type TranscriptPart,
} from "../sync/khala-runtime-transcript-core"
import { buildQuoteSnippet } from "../sync/swipe-quote-core"
import { useKhalaMobileSyncPrimitives } from "../sync/khala-mobile-sync-runtime-context"
import { useKhalaSyncScopeEntities } from "../sync/use-khala-sync-scope-entities"
import { useKhalaSyncPush } from "../sync/use-khala-sync-push"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"

const buildTranscriptPartPopupOptions = (
  part: TranscriptPart,
  onQuote: () => void,
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

type QuoteRequest = Readonly<{ id: string; snippet: string }>

const TRANSCRIPT_STAGGER_CAP = 8
const transcriptEntranceDelay = (index: number): number =>
  MOTION_STAGGER_MS * Math.min(index, TRANSCRIPT_STAGGER_CAP)

const createdAtOf = (message: ChatMessageEntity): string => message.createdAt

const formatClockTime = (iso: string): string => {
  const parsed = new Date(iso)
  if (Number.isNaN(parsed.getTime())) return iso
  return parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
}

type ThreadMessagesScreenProps = AppStackScreenProps<"ThreadMessages">

const atBottomFromScroll = (event: NativeSyntheticEvent<NativeScrollEvent>): boolean => {
  const { contentOffset, contentSize, layoutMeasurement } = event.nativeEvent
  const distanceFromBottom = contentSize.height - (contentOffset.y + layoutMeasurement.height)
  return distanceFromBottom < 120
}

export const ThreadMessagesScreen = ({ navigation, route }: ThreadMessagesScreenProps) => {
  const { threadId, title } = route.params
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- shared scroll ref across two independently-typed FlatLists (chat vs. transcript)
  const listRef = useRef<FlatList<any>>(null)
  const scope = String(threadScope(threadId))
  // Local-first, delta-synced (issue: "every time i open a new thread
  // session in the app it loads the messages from scratch"): these read
  // straight from the durable Expo SQLite store opened once at app root
  // (`khala-mobile-sync-runtime-context.tsx`), rendering whatever is already
  // on-device for this thread immediately, while the shared session's
  // durable cursor resumes with only the entries new since the last visit —
  // never a full history re-bootstrap once this thread has been opened
  // before. See `use-khala-sync-scope-entities.ts`.
  const { error: syncRuntimeError, overlay, session, status: syncRuntimeStatus, store } =
    useKhalaMobileSyncPrimitives()
  const chatState = useKhalaSyncScopeEntities({
    decode: decodeChatMessageEntity,
    entityType: CHAT_MESSAGE_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store
  })
  const runtimeState = useKhalaSyncScopeEntities({
    decode: decodeRuntimeEventEntity,
    entityType: RUNTIME_EVENT_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store
  })
  const turnState = useKhalaSyncScopeEntities({
    decode: decodeRuntimeTurnEntity,
    entityType: RUNTIME_TURN_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store
  })
  // MM-B2 (#8472): the thread entity itself (title, status, repoBinding) is
  // replicated into this same thread-local scope alongside messages/events
  // (see `chatThreadOverlayEffects` in `packages/khala-sync-db-collection`),
  // so reading it here needs no extra owner-scope round trip.
  const threadEntityState = useKhalaSyncScopeEntities({
    decode: decodeChatThreadEntity,
    entityType: CHAT_THREAD_ENTITY_TYPE,
    overlay,
    scope,
    session,
    store
  })
  const boundRepo = threadEntityState.items[0]?.repoBinding
  const activeTurn = useMemo(() => findActiveTurn(turnState.items), [turnState.items])
  const defaultLane = useMemo(() => mostRecentTurnLane(turnState.items), [turnState.items])
  const push = useKhalaSyncPush()
  const [quoteRequest, setQuoteRequest] = useState<QuoteRequest | undefined>(undefined)
  const [handoffPendingTurnId, setHandoffPendingTurnId] = useState<string | undefined>(undefined)
  const [handoffError, setHandoffError] = useState<string | null>(null)
  const [showScrollToLatest, setShowScrollToLatest] = useState(false)

  const requestHandoff = async (input: {
    turnId: string
    sourceLane: KhalaRuntimeLane
    targetLane: KhalaRuntimeLane
  }) => {
    if (handoffPendingTurnId !== undefined) return
    setHandoffPendingTurnId(input.turnId)
    setHandoffError(null)
    try {
      const turnEvents = sortEventsBySequence(
        runtimeState.items.filter(entity => entity.turnId === input.turnId),
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
            turnId: newTurnId,
          }),
          name: "runtime.startTurn",
        },
      ])
    } catch (error) {
      setHandoffError(error instanceof Error ? error.message : String(error))
    } finally {
      setHandoffPendingTurnId(undefined)
    }
  }

  const messages = sortByKeyAsc(
    chatState.items.filter(message => message.deletedAt === null),
    createdAtOf,
  )
  const transcriptParts = useMemo(
    () =>
      reduceRuntimeTranscript(
        sortEventsBySequence(runtimeState.items).map(entity => entity.event),
      ),
    [runtimeState.items],
  )
  const hasRichTranscript = transcriptParts.length > 0

  useEffect(() => {
    if (messages.length === 0 && transcriptParts.length === 0) return
    listRef.current?.scrollToEnd({ animated: true })
    setShowScrollToLatest(false)
  }, [messages.length, transcriptParts.length])

  const status = hasRichTranscript ? runtimeState.status : chatState.status
  const loading = status === "loading" && messages.length === 0 && transcriptParts.length === 0

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <KhalaThreadHeader
        onBack={() => {
          if (navigation.canGoBack()) navigation.goBack()
        }}
        subtitle="work · Khala Mobile"
        title={title ?? "Thread"}
      />
      <Pressable
        accessibilityLabel={
          boundRepo === undefined || boundRepo === null
            ? "No repo bound — tap to pick a repo"
            : `Repo bound: ${boundRepo.owner}/${boundRepo.name}`
        }
        accessibilityRole="button"
        className="mx-4 mb-2 flex-row items-center justify-between rounded-lg border border-borderMuted bg-surfaceRaised px-3 py-2"
        onPress={() => navigation.navigate("RepoPicker", { threadId })}
      >
        <KhalaText numberOfLines={1} variant="muted">
          {boundRepo === undefined || boundRepo === null
            ? "No repo — tap to pick one"
            : `Repo: ${boundRepo.owner}/${boundRepo.name}`}
        </KhalaText>
        <KhalaText variant="faint">›</KhalaText>
      </Pressable>
      <View className="mx-4 mb-2">
        <CreditsBalanceChip />
      </View>
      <KeyboardAvoidingView
        behavior={chatComposerKeyboardVerticalOffset === 0 ? "height" : "padding"}
        className="flex-1"
        keyboardVerticalOffset={chatComposerKeyboardVerticalOffset}
      >
        <View className="flex-1">
          {syncRuntimeStatus === "missing_token" ? (
            <KhalaEmptyState
              className="flex-1"
              detail="Restart the app to sign in again."
              title="Not signed in"
            />
          ) : syncRuntimeStatus === "error" ? (
            <KhalaEmptyState
              className="flex-1"
              detail={syncRuntimeError ?? undefined}
              title="Sync unavailable"
              tone="danger"
            />
          ) : status === "error" ? (
            <KhalaEmptyState
              className="flex-1"
              detail={chatState.error ?? runtimeState.error ?? undefined}
              title="Thread unavailable"
              tone="danger"
            />
          ) : loading ? (
            <KhalaEmptyState className="flex-1" loading title="Loading messages" />
          ) : hasRichTranscript ? (
            <FlatList
              contentContainerClassName="gap-4 px-8 pb-8 pt-1"
              data={transcriptParts}
              keyExtractor={part => part.id}
              onScroll={event => setShowScrollToLatest(!atBottomFromScroll(event))}
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
                const quoteSnippet = buildQuoteSnippet(part)
                if (quoteSnippet === undefined) return row
                const onQuote = () => setQuoteRequest({ id: part.id, snippet: quoteSnippet })
                return (
                  <TouchablePopupHandler options={buildTranscriptPartPopupOptions(part, onQuote)}>
                    <SwipeableItem onSwipeComplete={onQuote}>{row}</SwipeableItem>
                  </TouchablePopupHandler>
                )
              }}
              scrollEventThrottle={120}
            />
          ) : messages.length === 0 ? (
            <KhalaEmptyState className="flex-1" title="No messages yet" />
          ) : (
            <FlatList
              contentContainerClassName="gap-4 px-8 pb-8 pt-1"
              data={messages}
              keyExtractor={message => message.messageId}
              onScroll={event => setShowScrollToLatest(!atBottomFromScroll(event))}
              ref={listRef}
              renderItem={({ item: message }) => (
                <View className="gap-1 px-1 py-1">
                  <KhalaText variant="faint">
                    {formatClockTime(message.createdAt)}
                  </KhalaText>
                  <KhalaText className="text-[22px] leading-8 text-text" variant="body">
                    {message.body}
                  </KhalaText>
                </View>
              )}
              scrollEventThrottle={120}
            />
          )}
          {showScrollToLatest ? (
            <View className="absolute bottom-4 left-0 right-0 items-center">
              <KhalaScrollToLatestButton
                onPress={() => {
                  listRef.current?.scrollToEnd({ animated: true })
                  setShowScrollToLatest(false)
                }}
              />
            </View>
          ) : null}
        </View>
        {handoffError === null ? null : (
          <KhalaText className="px-4 pb-1 text-danger" numberOfLines={2} variant="faint">
            {handoffError}
          </KhalaText>
        )}
        {syncRuntimeStatus === "missing_token" ? null : (
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
