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
  type RuntimeTurnEntity,
} from "@openagentsinc/khala-sync"
import * as Clipboard from "expo-clipboard"
import { useEffect, useMemo, useRef, useState } from "react"
import { FlatList, KeyboardAvoidingView, Text, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { AppHeader } from "../components/app-header"
import { type PopupOptionType, TouchablePopupHandler } from "../components/blurred-popup"
import { ChatComposer, chatComposerKeyboardVerticalOffset } from "../components/chat-composer"
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
import { useKhalaSyncCollection } from "../sync/use-khala-sync-collection"
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

const messageIdOf = (message: ChatMessageEntity): string => message.messageId
const createdAtOf = (message: ChatMessageEntity): string => message.createdAt
const runtimeEventIdOf = (event: RuntimeEventEntity): string => event.eventId
const runtimeTurnIdOf = (turn: RuntimeTurnEntity): string => turn.turnId

const formatClockTime = (iso: string): string => {
  const parsed = new Date(iso)
  if (Number.isNaN(parsed.getTime())) return iso
  return parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
}

type ThreadMessagesScreenProps = AppStackScreenProps<"ThreadMessages">

export const ThreadMessagesScreen = ({ route }: ThreadMessagesScreenProps) => {
  const { threadId, title } = route.params
  // eslint-disable-next-line @typescript-eslint/no-explicit-any -- shared scroll ref across two independently-typed FlatLists (chat vs. transcript)
  const listRef = useRef<FlatList<any>>(null)
  const scope = String(threadScope(threadId))

  const chatState = useKhalaSyncCollection(
    scope,
    CHAT_MESSAGE_ENTITY_TYPE,
    decodeChatMessageEntity,
    messageIdOf,
  )
  const runtimeState = useKhalaSyncCollection(
    scope,
    RUNTIME_EVENT_ENTITY_TYPE,
    decodeRuntimeEventEntity,
    runtimeEventIdOf,
  )
  const turnState = useKhalaSyncCollection(
    scope,
    RUNTIME_TURN_ENTITY_TYPE,
    decodeRuntimeTurnEntity,
    runtimeTurnIdOf,
  )
  const activeTurn = useMemo(() => findActiveTurn(turnState.items), [turnState.items])
  const defaultLane = useMemo(() => mostRecentTurnLane(turnState.items), [turnState.items])
  const push = useKhalaSyncPush()
  const [quoteRequest, setQuoteRequest] = useState<QuoteRequest | undefined>(undefined)
  const [handoffPendingTurnId, setHandoffPendingTurnId] = useState<string | undefined>(undefined)
  const [handoffError, setHandoffError] = useState<string | null>(null)

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
        {status === "missing_token" ? null : (
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
