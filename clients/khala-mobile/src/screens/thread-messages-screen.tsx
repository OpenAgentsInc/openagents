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
  Platform,
  Pressable,
  type NativeScrollEvent,
  type NativeSyntheticEvent,
  type TextStyle,
  type ViewStyle,
  View,
} from "react-native"
import type { DrawerNavigationProp } from "@react-navigation/drawer"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { type PopupOptionType, TouchablePopupHandler } from "../components/blurred-popup"
import { ChatComposer, chatComposerKeyboardVerticalOffset } from "../components/chat-composer"
import { KhalaScrollToLatestButton } from "../components/khala-scroll-to-latest-button"
import { KhalaThreadHeader } from "../components/khala-thread-header"
import { SwipeableItem } from "../components/swipeable-item"
import { TranscriptPartRow } from "../components/transcript-part-row"
import { EmptyState, Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import type { AppDrawerParamList, AppStackScreenProps } from "../navigators/navigationTypes"
import { buildCopyMarkdown, buildCopyText } from "../sync/blurred-popup-menu-core"
import { buildHandoffPromptBody, summarizeTurnEventsForHandoff } from "../sync/khala-cross-agent-handoff-core"
import {
  buildChatAppendMessageArgs,
  buildStartTurnIntentArgs,
  chatMessageBodyRef,
  findActiveTurn,
  findRecoverableTurn,
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
import {
  useKhalaMobileSyncPrimitives,
  useKhalaMobileSyncRuntime,
} from "../sync/khala-mobile-sync-runtime-context"
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

/**
 * Thread view, rebuilt on the ported Infinite Red Ignite `Text`/`EmptyState`
 * primitives + theme tokens (`../ignite`). Product behavior — local-first
 * delta sync, turn dispatch, steer/queue/stop, cross-agent handoff, swipe-
 * quote, new-thread escape hatch — is unchanged.
 */
export const ThreadMessagesScreen = ({ navigation, route }: ThreadMessagesScreenProps) => {
  const { createdLocally = false, threadId, title } = route.params
  const { theme, themed } = useAppTheme()
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
  const recoverableTurn = useMemo(() => findRecoverableTurn(turnState.items), [turnState.items])
  const defaultLane = useMemo(() => mostRecentTurnLane(turnState.items), [turnState.items])
  const push = useKhalaSyncPush()
  const syncRuntime = useKhalaMobileSyncRuntime()
  const [quoteRequest, setQuoteRequest] = useState<QuoteRequest | undefined>(undefined)
  const [handoffPendingTurnId, setHandoffPendingTurnId] = useState<string | undefined>(undefined)
  const [handoffError, setHandoffError] = useState<string | null>(null)
  const [showScrollToLatest, setShowScrollToLatest] = useState(false)
  const [creatingThread, setCreatingThread] = useState(false)
  const [newThreadError, setNewThreadError] = useState<string | null>(null)

  // One-tap "new thread" escape hatch (owner report, 2026-07-06: "no way to
  // start a new thread ... cant do anything"). Creates a fresh empty thread
  // and replaces the current one so the back stack doesn't accumulate a pile
  // of half-empty threads. Always reachable — even while a turn is in flight,
  // because the header (which owns this action) renders above the turn-gated
  // composer and never depends on turn state.
  const startNewThread = async () => {
    if (syncRuntime.status !== "ready" || creatingThread) return
    setCreatingThread(true)
    setNewThreadError(null)
    try {
      const newThreadId = makeSafeRef("thread")
      const newTitle = "New chat"
      const created = await syncRuntime.runtime.createThread({ threadId: newThreadId, title: newTitle })
      if (!created.ok) {
        throw new Error(created.error ?? "Could not create a new thread.")
      }
      navigation.replace("ThreadMessages", { createdLocally: true, threadId: newThreadId, title: newTitle })
    } catch (error) {
      setNewThreadError(error instanceof Error ? error.message : String(error))
      setCreatingThread(false)
    }
  }

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
  const scopeDenied =
    chatState.error === "Khala Sync scope access was denied" ||
    runtimeState.error === "Khala Sync scope access was denied" ||
    turnState.error === "Khala Sync scope access was denied" ||
    threadEntityState.error === "Khala Sync scope access was denied"
  const emptyLocalDraft = createdLocally && messages.length === 0 && transcriptParts.length === 0
  const loading = !emptyLocalDraft && status === "loading" && messages.length === 0 && transcriptParts.length === 0
  const threadUnavailable = status === "error" && !(emptyLocalDraft && scopeDenied)

  const repoBound = boundRepo !== undefined && boundRepo !== null

  return (
    <SafeAreaView style={themed($safeArea)} edges={["top", "bottom", "left", "right"]}>
      <KhalaThreadHeader
        onOpenMenu={() => navigation.getParent<DrawerNavigationProp<AppDrawerParamList>>()?.openDrawer()}
        onNewThread={syncRuntime.status === "ready" && !creatingThread ? () => void startNewThread() : undefined}
        subtitle="work · Khala Mobile"
        title={title ?? "Thread"}
      />
      {newThreadError === null ? null : (
        <Text size="xxs" numberOfLines={2} style={themed($errorLineWide)} text={newThreadError} />
      )}
      <Pressable
        accessibilityLabel={
          repoBound ? `Repo bound: ${boundRepo.owner}/${boundRepo.name}` : "No repo bound — tap to pick a repo"
        }
        accessibilityRole="button"
        style={themed($repoChip)}
        onPress={() => navigation.navigate("RepoPicker", { threadId })}
      >
        <Text numberOfLines={1} style={themed($dim)} text={repoBound ? `Repo: ${boundRepo.owner}/${boundRepo.name}` : "No repo — tap to pick one"} />
        <Text style={themed($faint)} text="›" />
      </Pressable>
      <KeyboardAvoidingView
        // iOS: `padding` + a 0 vertical offset makes the composer hug the top
        // of the keyboard exactly. The SafeAreaView's own bottom inset already
        // sits BELOW this view (covered by the keyboard once it's up), so any
        // extra offset here double-counts and opens a dead gap under the input
        // (owner report, 2026-07-06). Android keeps its prior `height` resize
        // (unchanged). See `chatComposerKeyboardVerticalOffset` (now 0).
        behavior={Platform.OS === "ios" ? "padding" : "height"}
        style={$flex1}
        keyboardVerticalOffset={chatComposerKeyboardVerticalOffset}
      >
        <View style={$flex1}>
          {syncRuntimeStatus === "missing_token" ? (
            <EmptyState style={$flex1Center} heading="Not signed in" content="Restart the app to sign in again." />
          ) : syncRuntimeStatus === "error" ? (
            <EmptyState style={$flex1Center} status="error" heading="Sync unavailable" content={syncRuntimeError ?? undefined} />
          ) : threadUnavailable ? (
            <EmptyState
              style={$flex1Center}
              status="error"
              heading="Thread unavailable"
              content={chatState.error ?? runtimeState.error ?? undefined}
            />
          ) : loading ? (
            <EmptyState style={$flex1Center} loading heading="Loading messages" />
          ) : hasRichTranscript ? (
            <FlatList
              contentContainerStyle={themed($transcriptContent)}
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
            <EmptyState style={$flex1Center} heading="No messages yet" />
          ) : (
            <FlatList
              // `grow justify-end` anchors a short conversation to the bottom,
              // just above the composer, instead of stranding a single message
              // at the top with a large empty gap below it (owner report,
              // 2026-07-06: "just the one message ... the big empty gap"). Has
              // no effect once the transcript overflows the viewport.
              contentContainerStyle={themed($messagesContent)}
              data={messages}
              keyExtractor={message => message.messageId}
              onScroll={event => setShowScrollToLatest(!atBottomFromScroll(event))}
              ref={listRef}
              renderItem={({ item: message }) => (
                <View style={$messageRow}>
                  <View style={themed($bubble)}>
                    <Text style={[$bubbleText, { color: theme.colors.text }]}>{message.body}</Text>
                  </View>
                  <Text size="xxs" style={themed($timestamp)} text={formatClockTime(message.createdAt)} />
                </View>
              )}
              scrollEventThrottle={120}
            />
          )}
          {showScrollToLatest ? (
            <View style={$scrollButtonWrap}>
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
          <Text size="xxs" numberOfLines={2} style={themed($errorLine)} text={handoffError} />
        )}
        {syncRuntimeStatus === "missing_token" ? null : (
          <ChatComposer
            activeTurn={activeTurn}
            appendMessage={syncRuntime.status === "ready" ? syncRuntime.runtime.appendMessage : undefined}
            defaultLane={defaultLane}
            onQuoteConsumed={() => setQuoteRequest(undefined)}
            push={push}
            quoteRequest={quoteRequest}
            recoverableTurn={activeTurn === undefined ? recoverableTurn : undefined}
            threadId={threadId}
          />
        )}
      </KeyboardAvoidingView>
    </SafeAreaView>
  )
}

const $flex1: ViewStyle = { flex: 1 }
const $flex1Center: ViewStyle = { flex: 1, justifyContent: "center" }
const $messageRow: ViewStyle = { alignItems: "flex-end" }
const $scrollButtonWrap: ViewStyle = { position: "absolute", bottom: 16, left: 0, right: 0, alignItems: "center" }
const $bubbleText: TextStyle = { fontSize: 17, lineHeight: 24 }

const $safeArea: ThemedStyle<ViewStyle> = ({ colors }) => ({
  flex: 1,
  backgroundColor: colors.background,
})

const $repoChip: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  marginHorizontal: spacing.md,
  marginBottom: spacing.xs,
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
  borderRadius: 8,
  borderWidth: 1,
  borderColor: colors.palette.neutral400,
  backgroundColor: colors.palette.neutral300,
  paddingHorizontal: spacing.sm,
  paddingVertical: spacing.xs,
})

const $transcriptContent: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  gap: spacing.md,
  paddingHorizontal: spacing.xl,
  paddingBottom: spacing.xl,
  paddingTop: spacing.xxxs,
})

const $messagesContent: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexGrow: 1,
  justifyContent: "flex-end",
  gap: spacing.sm,
  paddingHorizontal: spacing.md,
  paddingBottom: spacing.md,
  paddingTop: spacing.lg,
})

const $bubble: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  maxWidth: "86%",
  borderRadius: 16,
  borderBottomRightRadius: 4,
  borderWidth: 1,
  borderColor: colors.tint,
  backgroundColor: colors.palette.neutral300,
  paddingHorizontal: spacing.md,
  paddingVertical: spacing.sm,
})

const $timestamp: ThemedStyle<TextStyle> = ({ colors, spacing }) => ({
  color: colors.textDim,
  marginTop: spacing.xxs,
  paddingHorizontal: spacing.xxs,
})

const $errorLine: ThemedStyle<TextStyle> = ({ colors, spacing }) => ({
  color: colors.error,
  paddingHorizontal: spacing.md,
  paddingBottom: spacing.xxs,
})

const $errorLineWide: ThemedStyle<TextStyle> = ({ colors, spacing }) => ({
  color: colors.error,
  marginHorizontal: spacing.md,
  marginBottom: spacing.xxs,
})

const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $faint: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.palette.neutral500 })
