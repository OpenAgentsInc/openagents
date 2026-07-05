import {
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadEntity,
  personalScope,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import type { NativeStackNavigationProp } from "@react-navigation/native-stack"
import { FlatList, Text, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"
import { SafeAreaView } from "react-native-safe-area-context"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { AppHeader } from "../components/app-header"
import { TouchableFeedback } from "../components/touchable-feedback"
import type { AppDrawerScreenProps, AppStackParamList } from "../navigators/navigationTypes"
import { formatRelativeTime } from "../sync/relative-time-core"
import { sortByKeyDesc } from "../sync/khala-sync-entities-core"
import { useKhalaMobileSyncPrimitives } from "../sync/khala-mobile-sync-runtime-context"
import { useKhalaSyncScopeEntities } from "../sync/use-khala-sync-scope-entities"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"

const recencyOf = (thread: ChatThreadEntity): string =>
  thread.lastMessageAt ?? thread.updatedAt ?? thread.createdAt

type ThreadListScreenProps = AppDrawerScreenProps<"Threads">

export const ThreadListScreen = ({ navigation }: ThreadListScreenProps) => {
  const { ownerUserId } = useKhalaAuth()
  // Local-first, delta-synced: same fix as the thread message view — reads
  // whatever thread rows are already on-device immediately instead of
  // re-bootstrapping the whole thread list from the server on every app
  // launch, then catches up on only what changed via the shared session's
  // durable cursor.
  const { error: syncRuntimeError, overlay, session, status: syncRuntimeStatus, store } =
    useKhalaMobileSyncPrimitives()
  const state = useKhalaSyncScopeEntities({
    decode: decodeChatThreadEntity,
    entityType: CHAT_THREAD_ENTITY_TYPE,
    overlay,
    scope: ownerUserId === "" ? "" : String(personalScope(ownerUserId)),
    session,
    store
  })
  const threads = sortByKeyDesc(state.items, recencyOf)
  const now = Date.now()
  const stackNavigation = navigation.getParent<NativeStackNavigationProp<AppStackParamList>>()

  return (
    <SafeAreaView className="flex-1 bg-bg" edges={["top", "bottom", "left", "right"]}>
      <AppHeader showMenu title="Khala" />
      {syncRuntimeStatus === "missing_token" ? (
        <View className="flex-1 items-center justify-center px-8">
          <Text className="text-center font-mono text-sm text-textFaint">
            Not signed in. Restart the app to sign in again.
          </Text>
        </View>
      ) : syncRuntimeStatus === "error" ? (
        <View className="flex-1 items-center justify-center px-8">
          <Text className="text-center font-sans text-base text-danger">
            {syncRuntimeError}
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
                  stackNavigation?.navigate("ThreadMessages", {
                    threadId: thread.threadId,
                    title: thread.title,
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
