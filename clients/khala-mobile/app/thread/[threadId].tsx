import {
  CHAT_MESSAGE_ENTITY_TYPE,
  decodeChatMessageEntity,
  threadScope,
  type ChatMessageEntity
} from "@openagentsinc/khala-sync"
import { useLocalSearchParams } from "expo-router"
import { useEffect, useRef } from "react"
import { FlatList, Text, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { AppHeader } from "../../src/components/app-header"
import { sortByKeyAsc } from "../../src/sync/khala-sync-entities-core"
import { useKhalaSyncCollection } from "../../src/sync/use-khala-sync-collection"

const messageIdOf = (message: ChatMessageEntity): string => message.messageId
const createdAtOf = (message: ChatMessageEntity): string => message.createdAt

const formatClockTime = (iso: string): string => {
  const parsed = new Date(iso)
  if (Number.isNaN(parsed.getTime())) return iso
  return parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
}

export default function ThreadMessagesScreen() {
  const { threadId, title } = useLocalSearchParams<{ threadId: string; title?: string }>()
  const listRef = useRef<FlatList<ChatMessageEntity>>(null)
  const state = useKhalaSyncCollection(
    threadId === undefined ? "" : String(threadScope(threadId)),
    CHAT_MESSAGE_ENTITY_TYPE,
    decodeChatMessageEntity,
    messageIdOf
  )
  const messages = sortByKeyAsc(
    state.items.filter(message => message.deletedAt === null),
    createdAtOf
  )

  useEffect(() => {
    if (messages.length === 0) return
    listRef.current?.scrollToEnd({ animated: true })
  }, [messages.length])

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <AppHeader showBack title={title ?? "Thread"} />
      {state.status === "missing_token" ? (
        <View className="flex-1 items-center justify-center px-8">
          <Text className="text-center font-mono text-sm text-textFaint">
            Set EXPO_PUBLIC_KHALA_SYNC_DEMO_TOKEN before starting the app.
          </Text>
        </View>
      ) : state.status === "error" ? (
        <View className="flex-1 items-center justify-center px-8">
          <Text className="text-center font-sans text-base text-danger">
            {state.error}
          </Text>
        </View>
      ) : state.status === "loading" && messages.length === 0 ? (
        <View className="flex-1 items-center justify-center">
          <Text className="font-sans text-base text-textMuted">loading messages…</Text>
        </View>
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
