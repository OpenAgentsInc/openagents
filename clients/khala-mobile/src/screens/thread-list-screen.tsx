import {
  CHAT_THREAD_ENTITY_TYPE,
  decodeChatThreadEntity,
  personalScope,
  type ChatThreadEntity,
} from "@openagentsinc/khala-sync"
import type { DrawerNavigationProp } from "@react-navigation/drawer"
import { ActivityIndicator, FlatList, View, type TextStyle, type ViewStyle } from "react-native"
import Animated, { FadeIn } from "react-native-reanimated"

import { useKhalaAuth } from "../auth/khala-auth-context"
import { Card, EmptyState, Header, ListItem, Screen, Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import type { AppDrawerParamList, AppStackScreenProps } from "../navigators/navigationTypes"
import { OnboardingFlow } from "./onboarding-flow"
import { formatRelativeTime } from "../sync/relative-time-core"
import { sortByKeyDesc } from "../sync/khala-sync-entities-core"
import { useKhalaMobileSyncPrimitives } from "../sync/khala-mobile-sync-runtime-context"
import { useKhalaSyncScopeEntities } from "../sync/use-khala-sync-scope-entities"
import { MOTION_MEDIUM, MOTION_STAGGER_MS } from "../theme/motion"

// Matches thread-messages-screen.tsx's transcript stagger — arcade-fidelity
// audit (2026-07-06) §4: the app's staggered-entrance technique was wired
// into the transcript list but not this one, an inconsistency the owner
// asked to close.
const THREAD_LIST_STAGGER_CAP = 8
const threadEntranceDelay = (index: number): number =>
  MOTION_STAGGER_MS * Math.min(index, THREAD_LIST_STAGGER_CAP)

const recencyOf = (thread: ChatThreadEntity): string =>
  thread.lastMessageAt ?? thread.updatedAt ?? thread.createdAt

type ThreadListScreenProps = AppStackScreenProps<"Threads">

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
  const { themed } = useAppTheme()
  return (
    <View style={themed($noticeContainer)}>
      <EmptyState
        loading={loading}
        status={tone === "danger" ? "error" : undefined}
        heading={title}
        content={detail}
      />
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
  const { theme, themed } = useAppTheme()
  const busy = runtimeStatus === "loading" || collectionStatus === "loading"
  const label = syncLabel({ collectionStatus, runtimeStatus })
  const latest = latestRecency === undefined ? "none" : formatRelativeTime(latestRecency, now)
  const summary = `${threads.length} threads | ${messageCountLabel(totalMessageCount(threads))} | latest ${latest}`

  return (
    <View style={themed($overviewWrap)}>
      <Card
        style={themed($overviewCard)}
        verticalAlignment="center"
        ContentComponent={
          <View style={themed($overviewRow)}>
            <View style={themed($overviewText)}>
              <Text preset="subheading" text="Threads" />
              <Text size="xs" numberOfLines={1} style={themed($dim)} text={summary} />
            </View>
            <View style={themed($statusRow)}>
              {busy ? (
                <ActivityIndicator color={theme.colors.tint} size="small" />
              ) : (
                <View style={themed($statusDot)} />
              )}
              <Text
                size="xxs"
                style={label === "attention" ? themed($danger) : themed($accent)}
                text={label}
              />
            </View>
          </View>
        }
      />
    </View>
  )
}

type ThreadRowProps = Readonly<{
  now: number
  onPress: () => void
  thread: ChatThreadEntity
}>

const ThreadRow = ({ now, onPress, thread }: ThreadRowProps) => {
  const { themed } = useAppTheme()
  const messageLabel = messageCountLabel(thread.messageCount)
  const title = thread.title.trim() || "Untitled chat"

  return (
    <ListItem
      accessibilityLabel={title}
      onPress={onPress}
      TextProps={{ weight: "medium", size: "sm", numberOfLines: 3 }}
      RightComponent={<Text size="xxs" style={themed($meta)} text={formatRelativeTime(recencyOf(thread), now)} />}
    >
      {title}
      {"\n"}
      <Text size="xs" style={themed($dim)} text={messageLabel} />
    </ListItem>
  )
}

const ThreadListSeparator = () => {
  const { themed } = useAppTheme()
  return <View style={themed($separator)} />
}

const ThreadListFooter = () => {
  const { themed } = useAppTheme()
  return <View style={themed($footerSpacer)} />
}

const ThreadListEmpty = () => {
  const { themed } = useAppTheme()
  return (
    <View style={themed($emptyPad)}>
      <EmptyState heading="No threads yet" />
    </View>
  )
}

const renderThreadListSeparator = () => <ThreadListSeparator />

const renderThreadListFooter = () => <ThreadListFooter />

const renderThreadListEmpty = () => <ThreadListEmpty />

export const ThreadListScreen = ({ navigation }: ThreadListScreenProps) => {
  const { ownerUserId } = useKhalaAuth()
  const { themed } = useAppTheme()
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
  const firstThread = threads[0]
  const latestRecency = firstThread === undefined ? undefined : recencyOf(firstThread)

  return (
    <Screen preset="fixed" contentContainerStyle={themed($fill)}>
      <Header title="Khala" leftIcon="☰" onLeftPress={() => navigation.getParent<DrawerNavigationProp<AppDrawerParamList>>()?.openDrawer()} />
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
      ) : state.status === "ready" && threads.length === 0 ? (
        // MM-H2 (#8488): a confirmed-empty thread list (never "still
        // loading") IS the onboarding entry point — no separate route to get
        // stuck on, and it naturally never shows again once the user has any
        // thread at all.
        <OnboardingFlow
          onThreadCreated={({ threadId, title }) => navigation.replace("ThreadMessages", { threadId, title })}
        />
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
          renderItem={({ index, item: thread }) => (
            <Animated.View entering={FadeIn.delay(threadEntranceDelay(index)).duration(MOTION_MEDIUM)}>
              <ThreadRow
                now={now}
                onPress={() =>
                  navigation.navigate("ThreadMessages", {
                    threadId: thread.threadId,
                    title: thread.title,
                  })
                }
                thread={thread}
              />
            </Animated.View>
          )}
        />
      )}
    </Screen>
  )
}

const $fill: ThemedStyle<ViewStyle> = () => ({ flex: 1 })

const $noticeContainer: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  justifyContent: "center",
  paddingHorizontal: spacing.md,
})

const $overviewWrap: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.md,
  paddingTop: spacing.xs,
  paddingBottom: spacing.sm,
})

const $overviewCard: ThemedStyle<ViewStyle> = ({ colors }) => ({
  backgroundColor: colors.palette.neutral200,
  borderColor: colors.palette.neutral400,
})

const $overviewRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "space-between",
  gap: spacing.md,
})

const $overviewText: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flex: 1,
  gap: spacing.xxxs,
})

const $statusRow: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.xs,
})

const $statusDot: ThemedStyle<ViewStyle> = ({ colors }) => ({
  height: 8,
  width: 8,
  borderRadius: 4,
  backgroundColor: colors.tint,
})

const $separator: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  height: 1,
  marginHorizontal: spacing.md,
  backgroundColor: colors.separator,
})

const $footerSpacer: ThemedStyle<ViewStyle> = ({ spacing }) => ({ height: spacing.xl })

const $emptyPad: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.md,
  paddingVertical: spacing.xxxl,
})

const $dim: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })
const $meta: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim, paddingTop: 2 })
const $accent: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.tint })
const $danger: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.error })
