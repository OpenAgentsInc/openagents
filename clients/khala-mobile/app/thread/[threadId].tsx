import {
  CHAT_MESSAGE_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeRuntimeEventEntity,
  decodeRuntimeTurnEntity,
  RUNTIME_EVENT_ENTITY_TYPE,
  RUNTIME_TURN_ENTITY_TYPE,
  threadScope,
  type ChatMessageEntity,
  type RuntimeEventEntity,
  type RuntimeTurnEntity
} from "@openagentsinc/khala-sync"
import { useLocalSearchParams } from "expo-router"
import { useEffect, useMemo, useRef } from "react"
import { FlatList, KeyboardAvoidingView, Text, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { AppHeader } from "../../src/components/app-header"
import { ChatComposer, chatComposerKeyboardVerticalOffset } from "../../src/components/chat-composer"
import { TranscriptPartRow } from "../../src/components/transcript-part-row"
import { findActiveTurn } from "../../src/sync/khala-runtime-compose-core"
import { sortByKeyAsc } from "../../src/sync/khala-sync-entities-core"
import {
  reduceRuntimeTranscript,
  sortEventsBySequence,
  type TranscriptPart
} from "../../src/sync/khala-runtime-transcript-core"
import { useKhalaSyncCollection } from "../../src/sync/use-khala-sync-collection"
import { useKhalaSyncPush } from "../../src/sync/use-khala-sync-push"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../../src/theme/motion"

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
  const push = useKhalaSyncPush()

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
                Set EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN before starting the app.
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
              renderItem={({ index, item: part }) => (
                <Animated.View entering={FadeIn.delay(transcriptEntranceDelay(index)).duration(MOTION_MEDIUM)}>
                  <TranscriptPartRow part={part} />
                </Animated.View>
              )}
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
        {threadId === undefined || status === "missing_token" ? null : (
          <ChatComposer activeTurn={activeTurn} push={push} threadId={threadId} />
        )}
      </KeyboardAvoidingView>
    </SafeAreaView>
  )
}
