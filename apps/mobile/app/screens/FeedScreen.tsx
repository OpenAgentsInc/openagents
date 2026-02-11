import { useConvexAuth, useMutation, useQuery } from "convex/react"
import { FC, useEffect, useRef, useState } from "react"
import { FlatList, View } from "react-native"
import { api } from "../../../web/convex/_generated/api"
import Config from "@/config"
import { Screen } from "@/components/Screen"
import { Text } from "@/components/Text"
import { useAuth } from "@/context/AuthContext"
import { DemoTabScreenProps } from "@/navigators/navigationTypes"
import { $styles } from "@/theme/styles"

/**
 * Feed tab: user's thread messages from Convex (same as /autopilot on web).
 * Requires auth; ensures owned thread then subscribes to getThreadSnapshot.
 */
export const FeedScreen: FC<DemoTabScreenProps<"Feed">> = function FeedScreen() {
  const { isAuthenticated: convexAuthenticated, isLoading: convexAuthLoading } = useConvexAuth()
  const { isAuthenticated: isSignedIn, authToken } = useAuth()
  const ensureOwnedThread = useMutation(api.autopilot.threads.ensureOwnedThread)
  const [threadId, setThreadId] = useState<string | null>(null)
  const [ensureError, setEnsureError] = useState<string | null>(null)
  const ensuredOnce = useRef(false)
  const lastAuthTokenRef = useRef<string | undefined>(undefined)
  if (lastAuthTokenRef.current !== authToken) {
    lastAuthTokenRef.current = authToken
    if (!authToken) {
      ensuredOnce.current = false
      setEnsureError(null)
    }
  }

  const convexUrl = Config.convexUrl ?? "(none)"
  const envLabel = convexUrl.includes("quaint-leopard") ? "dev" : convexUrl.includes("aware-caterpillar") ? "prod" : "unknown"

  // Convex has finished auth and rejected our token (we have token but Convex says not authenticated)
  const tokenRejectedByConvex = !!authToken && !convexAuthLoading && !convexAuthenticated && !threadId && !ensureError

  console.log("[FeedScreen] render", {
    convexUrl,
    env: envLabel,
    isSignedIn,
    convexAuthLoading,
    convexAuthenticated,
    hasAuthToken: !!authToken,
    threadId,
    ensuredOnce: ensuredOnce.current,
    tokenRejectedByConvex,
  })

  // Only call ensureOwnedThread after Convex has accepted the token (avoids sending mutation before auth is set).
  useEffect(() => {
    if (!isSignedIn || !authToken || !convexAuthenticated || ensuredOnce.current) return
    setEnsureError(null)
    ensuredOnce.current = true
    console.log("[FeedScreen] ensureOwnedThread start")
    ensureOwnedThread({})
      .then((result) => {
        const id = (result as { threadId?: string })?.threadId ?? null
        console.log("[FeedScreen] ensureOwnedThread ok", { threadId: id })
        setThreadId(id)
        setEnsureError(null)
      })
      .catch((err) => {
        console.warn("[FeedScreen] ensureOwnedThread failed", err)
        const msg = String(err?.message ?? err)
        setEnsureError(msg.includes("unauthorized") ? "auth" : "unknown")
      })
  }, [isSignedIn, authToken, convexAuthenticated, ensureOwnedThread])

  const snapshot = useQuery(
    api.autopilot.messages.getThreadSnapshot,
    threadId ? { threadId, maxMessages: 100, maxParts: 2000 } : "skip",
  )

  const messages = Array.isArray(snapshot?.messages) ? snapshot.messages : []
  const status = snapshot === undefined ? "loading" : snapshot === null ? "error" : "loaded"

  console.log("[FeedScreen] query", {
    threadId,
    status,
    messageCount: messages.length,
    snapshotOk: snapshot && typeof snapshot === "object" && (snapshot as any).ok,
  })

  // Use AuthContext as source of truth for signed-in (Profile uses the same; Convex auth can lag).
  if (!isSignedIn) {
    return (
      <Screen preset="fixed" contentContainerStyle={$styles.container} safeAreaEdges={["top"]}>
        <Text text="Sign in to see your feed." />
      </Screen>
    )
  }

  // Signed in but no token (e.g. session from before Worker returned token) — need to re-login for Convex.
  if (!authToken && !threadId) {
    return (
      <Screen preset="fixed" contentContainerStyle={$styles.container} safeAreaEdges={["top"]}>
        <Text text="Log out and sign in again to load your feed." />
      </Screen>
    )
  }

  // Convex rejected the token (invalid or wrong issuer); or mutation failed with unauthorized.
  if (ensureError === "auth" || tokenRejectedByConvex) {
    return (
      <Screen preset="fixed" contentContainerStyle={$styles.container} safeAreaEdges={["top"]}>
        <Text text="Log out and sign in again to load your feed." />
      </Screen>
    )
  }

  if (convexAuthLoading || (!threadId && !ensureError)) {
    return (
      <Screen preset="fixed" contentContainerStyle={$styles.container} safeAreaEdges={["top"]}>
        <Text text="Loading your thread…" />
      </Screen>
    )
  }

  if (!threadId) {
    return (
      <Screen preset="fixed" contentContainerStyle={$styles.container} safeAreaEdges={["top"]}>
        <Text text="Could not load thread. Check logs." />
      </Screen>
    )
  }

  return (
    <Screen preset="fixed" contentContainerStyle={$styles.container} safeAreaEdges={["top"]}>
      <View style={{ padding: 8 }}>
        <Text size="xs" text={`Convex: ${envLabel} | ${convexUrl}`} />
        <Text size="xs" text={`Thread: ${threadId} | messages: ${messages.length}`} />
      </View>
      {status === "loading" && <Text text="Loading messages…" />}
      {status === "error" && <Text text="Failed to load messages." />}
      {status === "loaded" && (
        <FlatList
          data={messages}
          keyExtractor={(item: any) => item?.messageId ?? String(item)}
          renderItem={({ item }: { item: any }) => (
            <View style={{ padding: 8, borderBottomWidth: 1, borderColor: "#eee" }}>
              <Text size="xs" text={`${item?.role ?? "?"}: ${(item?.text ?? "").slice(0, 80)}`} />
            </View>
          )}
        />
      )}
    </Screen>
  )
}
