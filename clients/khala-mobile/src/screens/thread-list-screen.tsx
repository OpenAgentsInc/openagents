import {
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadEntity,
  personalScope,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import type { NativeStackNavigationProp } from "@react-navigation/native-stack"
import { FlatList, StyleSheet, View } from "react-native"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { ActivityIndicator } from "../components/activity-indicator"
import { AppHeader } from "../components/app-header"
import { BackgroundGradient } from "../components/background-gradient"
import { KhalaEmptyState } from "../components/khala-empty-state"
import { KhalaListItem } from "../components/khala-list-item"
import { KhalaScreen } from "../components/khala-screen"
import { KhalaText } from "../components/khala-text"
import type { AppDrawerScreenProps, AppStackParamList } from "../navigators/navigationTypes"
import { formatRelativeTime } from "../sync/relative-time-core"
import { sortByKeyDesc } from "../sync/khala-sync-entities-core"
import { useKhalaMobileSyncPrimitives } from "../sync/khala-mobile-sync-runtime-context"
import { useKhalaSyncScopeEntities } from "../sync/use-khala-sync-scope-entities"
import { khalaMobileTheme } from "../theme/tokens"

const recencyOf = (thread: ChatThreadEntity): string =>
  thread.lastMessageAt ?? thread.updatedAt ?? thread.createdAt

type ThreadListScreenProps = AppDrawerScreenProps<"Threads">

type ThreadListNoticeProps = Readonly<{
  detail?: string
  loading?: boolean
  title: string
  tone?: "accent" | "danger" | "muted"
}>

const totalMessageCount = (threads: ReadonlyArray<ChatThreadEntity>): number =>
  threads.reduce((sum, thread) => sum + thread.messageCount, 0)

const messageCountLabel = (count: number): string =>
  count === 0 ? "No messages yet" : count === 1 ? "1 message" : `${count} messages`

const syncLabel = (input: {
  collectionStatus: "loading" | "ready" | "error"
  runtimeStatus: "loading" | "missing_token" | "error" | "ready"
}): string => {
  if (input.runtimeStatus === "loading" || input.collectionStatus === "loading") return "syncing"
  if (input.runtimeStatus === "ready" && input.collectionStatus === "ready") return "live"
  return "attention"
}

const ThreadListNotice = ({
  detail,
  loading = false,
  title,
  tone = "muted",
}: ThreadListNoticeProps) => {
  return (
    <View className="flex-1 justify-center px-4">
      <KhalaEmptyState detail={detail} loading={loading} title={title} tone={tone} />
    </View>
  )
}

type ThreadListOverviewProps = Readonly<{
  collectionStatus: "loading" | "ready" | "error"
  latestRecency: string | undefined
  now: number
  runtimeStatus: "loading" | "missing_token" | "error" | "ready"
  threads: ReadonlyArray<ChatThreadEntity>
}>

const ThreadListOverview = ({
  collectionStatus,
  latestRecency,
  now,
  runtimeStatus,
  threads,
}: ThreadListOverviewProps) => {
  const busy = runtimeStatus === "loading" || collectionStatus === "loading"
  const label = syncLabel({ collectionStatus, runtimeStatus })
  const latest = latestRecency === undefined ? "none" : formatRelativeTime(latestRecency, now)
  const summary = `${threads.length} threads | ${messageCountLabel(totalMessageCount(threads))} | latest ${latest}`

  return (
    <View className="px-4 pb-3 pt-2">
      <BackgroundGradient
        colors={[
          "rgba(79,208,255,0.16)",
          "rgba(58,123,255,0.08)",
          "rgba(10,17,29,0.04)",
          "rgba(79,208,255,0.12)",
        ]}
        cornerRadius={8}
        maxBlur={busy ? 8 : 2}
        style={styles.overviewGradient}
      >
        <View className="border border-borderMuted bg-surface/95 px-4 py-3">
          <View className="flex-row items-center justify-between gap-4">
            <View className="min-w-0 flex-1 gap-1">
              <KhalaText className="text-2xl" variant="heading">
                Threads
              </KhalaText>
              <KhalaText className="shrink" numberOfLines={1} variant="muted">
                {summary}
              </KhalaText>
            </View>
            <View className="flex-row items-center gap-2">
              {busy ? <ActivityIndicator size={22} /> : <View className="h-2 w-2 bg-accent" />}
              <KhalaText className={label === "attention" ? "text-danger" : "text-accent"} variant="faint">
                {label}
              </KhalaText>
            </View>
          </View>
        </View>
      </BackgroundGradient>
    </View>
  )
}

type ThreadRowProps = Readonly<{
  now: number
  onPress: () => void
  thread: ChatThreadEntity
}>

const ThreadRow = ({ now, onPress, thread }: ThreadRowProps) => {
  const messageLabel = messageCountLabel(thread.messageCount)
  const title = thread.title.trim() || "Untitled chat"

  return (
    <KhalaListItem
      accessibilityLabel={title}
      detail={messageLabel}
      meta={formatRelativeTime(recencyOf(thread), now)}
      onPress={onPress}
      title={title}
      titleNumberOfLines={2}
    />
  )
}

const ThreadListSeparator = () => <View className="mx-4 h-px bg-borderMuted" />

const ThreadListFooter = () => <View className="h-8" />

const ThreadListEmpty = () => (
  <View className="px-4 py-16">
    <KhalaEmptyState title="No threads yet" />
  </View>
)

const renderThreadListSeparator = () => <ThreadListSeparator />

const renderThreadListFooter = () => <ThreadListFooter />

const renderThreadListEmpty = () => <ThreadListEmpty />

export const ThreadListScreen = ({ navigation }: ThreadListScreenProps) => {
  const { ownerUserId } = useKhalaAuth()
  // Local-first, delta-synced: same fix as the thread message view. Reads
  // whatever thread rows are already on-device immediately, then catches up
  // via the shared session's durable cursor.
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
  const firstThread = threads[0]
  const latestRecency = firstThread === undefined ? undefined : recencyOf(firstThread)

  return (
    <KhalaScreen preset="fixed">
      <AppHeader showMenu title="Khala" />
      {syncRuntimeStatus === "missing_token" ? (
        <ThreadListNotice
          detail="Restart the app to sign in again."
          title="Not signed in"
        />
      ) : syncRuntimeStatus === "error" ? (
        <ThreadListNotice detail={syncRuntimeError ?? "Khala Sync could not open."} title="Sync unavailable" tone="danger" />
      ) : state.status === "error" ? (
        <ThreadListNotice detail={state.error ?? "Could not read local threads."} title="Threads unavailable" tone="danger" />
      ) : state.status === "loading" && threads.length === 0 ? (
        <ThreadListNotice loading title="Loading threads" tone="accent" />
      ) : (
        <FlatList
          ItemSeparatorComponent={renderThreadListSeparator}
          ListEmptyComponent={renderThreadListEmpty}
          ListFooterComponent={renderThreadListFooter}
          ListHeaderComponent={
            <ThreadListOverview
              collectionStatus={state.status}
              latestRecency={latestRecency}
              now={now}
              runtimeStatus={syncRuntimeStatus}
              threads={threads}
            />
          }
          data={threads}
          keyExtractor={thread => thread.threadId}
          renderItem={({ item: thread }) => (
            <ThreadRow
              now={now}
              onPress={() =>
                stackNavigation?.navigate("ThreadMessages", {
                  threadId: thread.threadId,
                  title: thread.title,
                })
              }
              thread={thread}
            />
          )}
        />
      )}
    </KhalaScreen>
  )
}

const styles = StyleSheet.create({
  overviewGradient: {
    backgroundColor: khalaMobileTheme.surface,
    borderRadius: 8,
    overflow: "hidden",
  },
})
