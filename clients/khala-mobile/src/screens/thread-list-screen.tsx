import {
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadEntity,
  personalScope,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import type { NativeStackNavigationProp } from "@react-navigation/native-stack"
import { FlatList, StyleSheet, View } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { ActivityIndicator } from "../components/activity-indicator"
import { AppHeader } from "../components/app-header"
import { BackgroundGradient } from "../components/background-gradient"
import { Frame, usePowerOnVisible } from "../components/frame"
import { KhalaScreen } from "../components/khala-screen"
import { KhalaText } from "../components/khala-text"
import { TouchableFeedback } from "../components/touchable-feedback"
import type { AppDrawerScreenProps, AppStackParamList } from "../navigators/navigationTypes"
import { formatRelativeTime } from "../sync/relative-time-core"
import { sortByKeyDesc } from "../sync/khala-sync-entities-core"
import { useKhalaMobileSyncPrimitives } from "../sync/khala-mobile-sync-runtime-context"
import { useKhalaSyncScopeEntities } from "../sync/use-khala-sync-scope-entities"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"
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
  const visible = usePowerOnVisible()
  const borderColor =
    tone === "danger"
      ? khalaMobileTheme.danger
      : tone === "accent"
        ? khalaMobileTheme.accent
        : khalaMobileTheme.borderStrong

  return (
    <View className="flex-1 justify-center px-4">
      <Frame
        alwaysShowBorder
        borderColor={borderColor}
        color={borderColor}
        style={styles.noticeFrame}
        visible={visible}
      >
        <View className="items-center px-5 py-7">
          {loading ? <ActivityIndicator size={34} type="large" /> : null}
          <KhalaText className={loading ? "mt-4 text-center" : "text-center"} variant="body">
            {title}
          </KhalaText>
          {detail === undefined ? null : (
            <KhalaText className="mt-2 text-center" variant={tone === "danger" ? "danger" : "muted"}>
              {detail}
            </KhalaText>
          )}
        </View>
      </Frame>
    </View>
  )
}

type MetricPillProps = Readonly<{
  label: string
  value: string
}>

const MetricPill = ({ label, value }: MetricPillProps) => (
  <View className="min-w-[86px] border border-borderMuted bg-surface/80 px-2.5 py-2">
    <KhalaText className="text-[10px]" variant="label">
      {label}
    </KhalaText>
    <KhalaText className="mt-1 text-lg font-semibold" variant="mono">
      {value}
    </KhalaText>
  </View>
)

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
  const visible = usePowerOnVisible()
  const busy = runtimeStatus === "loading" || collectionStatus === "loading"
  const label = syncLabel({ collectionStatus, runtimeStatus })
  const latest = latestRecency === undefined ? "none" : formatRelativeTime(latestRecency, now)

  return (
    <Frame alwaysShowBorder style={styles.overviewFrame} visible={visible}>
      <BackgroundGradient
        colors={[
          "rgba(79,208,255,0.24)",
          "rgba(58,123,255,0.14)",
          "rgba(10,17,29,0.06)",
          "rgba(79,208,255,0.20)",
        ]}
        cornerRadius={8}
        maxBlur={busy ? 12 : 4}
        style={styles.overviewGradient}
      >
        <View className="gap-4 px-4 py-4">
          <View className="flex-row items-start justify-between gap-4">
            <View className="min-w-0 flex-1">
              <KhalaText variant="label">Khala relay</KhalaText>
              <KhalaText className="mt-1 text-3xl font-semibold" variant="heading">
                Threads
              </KhalaText>
            </View>
            <View className="items-end gap-2">
              {busy ? (
                <ActivityIndicator size={30} type="large" />
              ) : (
                <View className="h-3 w-3 border border-accent bg-accent" />
              )}
              <KhalaText className={label === "attention" ? "text-danger" : "text-accent"} variant="faint">
                {label}
              </KhalaText>
            </View>
          </View>
          <View className="flex-row flex-wrap gap-2">
            <MetricPill label="threads" value={String(threads.length)} />
            <MetricPill label="messages" value={String(totalMessageCount(threads))} />
            <MetricPill label="latest" value={latest} />
          </View>
        </View>
      </BackgroundGradient>
    </Frame>
  )
}

type ThreadRowProps = Readonly<{
  index: number
  now: number
  onPress: () => void
  thread: ChatThreadEntity
}>

const threadRowDelay = (index: number): number => MOTION_STAGGER_MS * Math.min(index, 8)

const ThreadRow = ({ index, now, onPress, thread }: ThreadRowProps) => {
  const visible = usePowerOnVisible(threadRowDelay(index))
  const isFirst = index === 0
  const messageLabel =
    thread.messageCount === 0
      ? "No messages yet"
      : thread.messageCount === 1
        ? "1 message"
        : `${thread.messageCount} messages`
  const title = thread.title.trim() || "Untitled chat"
  const borderColor = isFirst ? khalaMobileTheme.accent : khalaMobileTheme.borderStrong

  return (
    <Animated.View entering={FadeIn.delay(threadRowDelay(index)).duration(MOTION_MEDIUM)}>
      <Frame
        alwaysShowBorder
        borderColor={borderColor}
        color={borderColor}
        style={styles.rowFrame}
        visible={visible}
      >
        <TouchableFeedback
          accessibilityRole="button"
          className="px-3.5 py-3"
          defaultColor="rgba(5, 8, 14, 0.72)"
          highlightColor="rgba(79, 208, 255, 0.14)"
          onPress={onPress}
        >
          <View className="flex-row items-start gap-3">
            <View className="mt-0.5 items-center gap-1">
              <View className={isFirst ? "h-2.5 w-2.5 bg-accent" : "h-2.5 w-2.5 border border-borderStrong"} />
              <View className="h-8 w-px bg-borderMuted" />
            </View>
            <View className="min-w-0 flex-1 gap-1">
              <View className="flex-row items-start justify-between gap-3">
                <KhalaText className="shrink text-lg font-semibold leading-snug" numberOfLines={2} variant="body">
                  {title}
                </KhalaText>
                <KhalaText className="pt-1 tabular-nums" variant="faint">
                  {formatRelativeTime(recencyOf(thread), now)}
                </KhalaText>
              </View>
              <View className="flex-row items-center justify-between gap-3">
                <KhalaText className="shrink" numberOfLines={1} variant="muted">
                  {messageLabel}
                </KhalaText>
                <KhalaText className="text-accent" variant="faint">
                  open
                </KhalaText>
              </View>
            </View>
          </View>
        </TouchableFeedback>
      </Frame>
    </Animated.View>
  )
}

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
          ListEmptyComponent={<ThreadListNotice title="No threads yet" />}
          ListHeaderComponent={
            <ThreadListOverview
              collectionStatus={state.status}
              latestRecency={latestRecency}
              now={now}
              runtimeStatus={syncRuntimeStatus}
              threads={threads}
            />
          }
          contentContainerClassName="gap-3 px-4 pb-8 pt-3"
          data={threads}
          keyExtractor={thread => thread.threadId}
          renderItem={({ index, item: thread }) => (
            <ThreadRow
              index={index}
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
  noticeFrame: {
    backgroundColor: khalaMobileTheme.surface,
    minHeight: 168,
  },
  overviewFrame: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    overflow: "hidden",
  },
  overviewGradient: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderRadius: 8,
    overflow: "hidden",
  },
  rowFrame: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    overflow: "hidden",
  },
})
