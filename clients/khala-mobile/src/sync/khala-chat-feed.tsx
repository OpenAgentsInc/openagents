import { useEffect, useRef, useState } from "react"
import { ScrollView, Text, View } from "react-native"

import {
  KHALA_SYNC_DEMO_BASE_URL,
  KHALA_SYNC_DEMO_CLIENT_GROUP_ID,
  KHALA_SYNC_DEMO_OWNER_USER_ID,
  KHALA_SYNC_DEMO_THREAD_ID
} from "../config/khala-sync-demo"
import {
  openKhalaMobileSyncRuntime,
  type KhalaMobileChatMessagesState,
  type KhalaMobileChatThreadsState,
  type KhalaMobileSyncRuntime
} from "./khala-mobile-sync-runtime"

const REFRESH_INTERVAL_MS = 2_000

type FeedState =
  | Readonly<{ status: "connecting" }>
  | Readonly<{ status: "missing_config"; error: string }>
  | Readonly<{ status: "missing_auth"; error: string }>
  | Readonly<{ status: "error"; error: string }>
  | Readonly<{
      status: "ready"
      messages: KhalaMobileChatMessagesState
      threads: KhalaMobileChatThreadsState
      updatedAt: string
    }>

export const KhalaChatFeed = () => {
  const [state, setState] = useState<FeedState>({ status: "connecting" })
  const runtimeRef = useRef<KhalaMobileSyncRuntime | null>(null)
  const refreshingRef = useRef(false)

  useEffect(() => {
    if (KHALA_SYNC_DEMO_OWNER_USER_ID === "") {
      setState({
        error: "missing_owner_user_id",
        status: "missing_config"
      })
      return
    }

    let cancelled = false
    let timer: ReturnType<typeof setInterval> | null = null

    const refresh = async (runtime: KhalaMobileSyncRuntime) => {
      if (refreshingRef.current) return
      refreshingRef.current = true
      try {
        const [threads, messages] = await Promise.all([
          runtime.chatThreads(),
          runtime.chatMessages({
            limit: 100,
            threadId: KHALA_SYNC_DEMO_THREAD_ID
          })
        ])
        if (!cancelled) {
          setState({
            messages,
            status: "ready",
            threads,
            updatedAt: new Date().toISOString()
          })
        }
      } catch (error) {
        if (!cancelled) {
          setState({
            error: error instanceof Error ? error.message : String(error),
            status: "error"
          })
        }
      } finally {
        refreshingRef.current = false
      }
    }

    const run = async () => {
      setState({ status: "connecting" })
      try {
        const opened = await openKhalaMobileSyncRuntime({
          clientGroupId: KHALA_SYNC_DEMO_CLIENT_GROUP_ID,
          ownerUserId: KHALA_SYNC_DEMO_OWNER_USER_ID,
          syncBaseUrl: KHALA_SYNC_DEMO_BASE_URL
        })
        if (cancelled) return
        if (!opened.ok) {
          setState({
            error: opened.error,
            status: opened.authState === "missing" ? "missing_auth" : "error"
          })
          return
        }
        runtimeRef.current = opened.runtime
        await refresh(opened.runtime)
        timer = setInterval(() => void refresh(opened.runtime), REFRESH_INTERVAL_MS)
      } catch (error) {
        if (!cancelled) {
          setState({
            error: error instanceof Error ? error.message : String(error),
            status: "error"
          })
        }
      }
    }

    void run()

    return () => {
      cancelled = true
      if (timer !== null) clearInterval(timer)
      const runtime = runtimeRef.current
      runtimeRef.current = null
      void runtime?.close()
    }
  }, [])

  const statusLine =
    state.status === "ready"
      ? `${state.messages.phase} - ${state.messages.messages.length} messages - ${state.messages.pendingMutations} pending`
      : state.status === "connecting"
        ? "connecting to Khala Sync"
        : state.status === "missing_auth"
          ? "auth required"
          : state.error

  return (
    <View className="mt-8 w-full flex-1 gap-2 px-4">
      <Text className="font-sans text-sm text-textMuted">
        chat sync - {KHALA_SYNC_DEMO_THREAD_ID}
      </Text>
      <Text className="font-mono text-xs text-textFaint">{statusLine}</Text>
      {state.status === "ready" ? (
        <View className="flex-row gap-3">
          <Text className="font-mono text-xs text-textFaint">
            threads {state.threads.threads.length}
          </Text>
          <Text className="font-mono text-xs text-textFaint">
            updated {state.updatedAt}
          </Text>
        </View>
      ) : null}
      <ScrollView className="flex-1 rounded-lg border border-border bg-surface">
        {state.status !== "ready" ? (
          <Text className="p-3 font-mono text-xs text-textFaint">
            {statusLine}
          </Text>
        ) : state.messages.rejections.length > 0 ? (
          state.messages.rejections.map(rejection => (
            <View
              key={`${rejection.mutationId}-${rejection.observedAt}`}
              className="border-b border-borderMuted p-3"
            >
              <Text className="font-mono text-xs text-textFaint">
                rejected {rejection.errorCode}
              </Text>
              <Text className="mt-1 font-mono text-xs text-text">
                {rejection.messageSafe}
              </Text>
            </View>
          ))
        ) : state.messages.messages.length === 0 ? (
          <Text className="p-3 font-mono text-xs text-textFaint">
            no synced messages
          </Text>
        ) : (
          state.messages.messages.map(message => (
            <View key={message.messageId} className="border-b border-borderMuted p-3">
              <Text className="font-mono text-xs text-textFaint">
                {message.authorUserId} - {message.createdAt}
              </Text>
              <Text className="mt-1 font-sans text-sm text-text">{message.body}</Text>
            </View>
          ))
        )}
      </ScrollView>
    </View>
  )
}
