import {
  CHAT_MESSAGE_ENTITY_TYPE,
  decodeChatMessageEntity,
  decodeRuntimeEventEntity,
  RUNTIME_EVENT_ENTITY_TYPE,
  threadScope,
  type ChatMessageEntity,
  type RuntimeEventEntity
} from "@openagentsinc/khala-sync"
import { useLocalSearchParams } from "expo-router"
import { useEffect, useMemo, useRef } from "react"
import { FlatList, Text, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { AppHeader } from "../../src/components/app-header"
import { TranscriptPartRow } from "../../src/components/transcript-part-row"
import { sortByKeyAsc } from "../../src/sync/khala-sync-entities-core"
import {
  reduceRuntimeTranscript,
  sortEventsBySequence,
  type TranscriptPart
} from "../../src/sync/khala-runtime-transcript-core"
import { useKhalaSyncCollection } from "../../src/sync/use-khala-sync-collection"

const messageIdOf = (message: ChatMessageEntity): string => message.messageId
const createdAtOf = (message: ChatMessageEntity): string => message.createdAt
const runtimeEventIdOf = (event: RuntimeEventEntity): string => event.eventId

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
          renderItem={({ item: part }) => <TranscriptPartRow part={part} />}
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
    </SafeAreaView>
  )
}
