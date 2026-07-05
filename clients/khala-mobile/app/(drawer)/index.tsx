import {
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadEntity,
  personalScope,
  type ChatThreadEntity
} from "@openagentsinc/khala-sync"
import { useRouter } from "expo-router"
import { FlatList, Pressable, Text, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { AppHeader } from "../../src/components/app-header"
import { KHALA_SYNC_DEMO_OWNER_USER_ID } from "../../src/config/khala-sync-demo"
import { formatRelativeTime } from "../../src/sync/relative-time-core"
import { sortByKeyDesc } from "../../src/sync/khala-sync-entities-core"
import { useKhalaSyncCollection } from "../../src/sync/use-khala-sync-collection"

const threadIdOf = (thread: ChatThreadEntity): string => thread.threadId
const recencyOf = (thread: ChatThreadEntity): string =>
  thread.lastMessageAt ?? thread.updatedAt ?? thread.createdAt

export default function ThreadListScreen() {
  const router = useRouter()
  const state = useKhalaSyncCollection(
    KHALA_SYNC_DEMO_OWNER_USER_ID === "" ? "" : String(personalScope(KHALA_SYNC_DEMO_OWNER_USER_ID)),
    CHAT_THREAD_ENTITY_TYPE,
    decodeChatThreadEntity,
    threadIdOf
  )
  const threads = sortByKeyDesc(state.items, recencyOf)
  const now = Date.now()

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <AppHeader showMenu title="Khala" />
      {KHALA_SYNC_DEMO_OWNER_USER_ID === "" ? (
        <View className="flex-1 items-center justify-center px-8">
          <Text className="text-center font-mono text-sm text-textFaint">
            Set EXPO_PUBLIC_KHALA_SYNC_DEMO_OWNER_USER_ID before starting the app.
          </Text>
        </View>
      ) : state.status === "missing_token" ? (
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
      ) : state.status === "loading" && threads.length === 0 ? (
        <View className="flex-1 items-center justify-center">
          <Text className="font-sans text-base text-textMuted">loading threads…</Text>
        </View>
      ) : threads.length === 0 ? (
        <View className="flex-1 items-center justify-center">
          <Text className="font-sans text-base text-textMuted">No threads yet</Text>
        </View>
      ) : (
        <FlatList
          data={threads}
          keyExtractor={thread => thread.threadId}
          renderItem={({ item: thread }) => (
            <Pressable
              accessibilityRole="button"
              className="border-b border-borderMuted px-4 py-4 active:bg-surfaceActive"
              onPress={() =>
                router.push({
                  params: { threadId: thread.threadId, title: thread.title },
                  pathname: "/thread/[threadId]"
                })
              }
            >
              <View className="flex-row items-center justify-between gap-3">
                <Text className="shrink font-sans text-base font-semibold text-text" numberOfLines={1}>
                  {thread.title.trim() || "Untitled chat"}
                </Text>
                <Text className="font-mono text-xs tabular-nums text-textFaint">
                  {formatRelativeTime(recencyOf(thread), now)}
                </Text>
              </View>
              <Text className="mt-1 font-sans text-sm text-textMuted" numberOfLines={1}>
                {thread.messageCount === 0
                  ? "No messages yet"
                  : thread.messageCount === 1
                    ? "1 message"
                    : `${thread.messageCount} messages`}
              </Text>
            </Pressable>
          )}
        />
      )}
    </SafeAreaView>
  )
}
