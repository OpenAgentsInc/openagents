import {
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadEntity,
  personalScope,
  type ChatThreadEntity
} from "@openagentsinc/khala-sync"
import { useRouter } from "expo-router"
import { FlatList, Text, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../../src/auth/khala-auth-context"
import { AppHeader } from "../../src/components/app-header"
import { TouchableFeedback } from "../../src/components/touchable-feedback"
import { formatRelativeTime } from "../../src/sync/relative-time-core"
import { sortByKeyDesc } from "../../src/sync/khala-sync-entities-core"
import { useKhalaSyncCollection } from "../../src/sync/use-khala-sync-collection"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../../src/theme/motion"

const threadIdOf = (thread: ChatThreadEntity): string => thread.threadId
const recencyOf = (thread: ChatThreadEntity): string =>
  thread.lastMessageAt ?? thread.updatedAt ?? thread.createdAt

export default function ThreadListScreen() {
  const router = useRouter()
  const { ownerUserId } = useKhalaAuth()
  const state = useKhalaSyncCollection(
    ownerUserId === "" ? "" : String(personalScope(ownerUserId)),
    CHAT_THREAD_ENTITY_TYPE,
    decodeChatThreadEntity,
    threadIdOf
  )
  const threads = sortByKeyDesc(state.items, recencyOf)
  const now = Date.now()

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <AppHeader showMenu title="Khala" />
      {state.status === "missing_token" ? (
        <View className="flex-1 items-center justify-center px-8">
          <Text className="text-center font-mono text-sm text-textFaint">
            Not signed in. Restart the app to sign in again.
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
          renderItem={({ index, item: thread }) => (
            <Animated.View entering={FadeIn.delay(MOTION_STAGGER_MS * index).duration(MOTION_MEDIUM)}>
              <TouchableFeedback
                accessibilityRole="button"
                className="border-b border-borderMuted px-4 py-4"
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
              </TouchableFeedback>
            </Animated.View>
          )}
        />
      )}
    </SafeAreaView>
  )
}
