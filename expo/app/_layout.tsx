import "@/utils/gestureHandler"
import "@/utils/global-error"
// import { useQuery } from "convex/react"
import * as Haptics from "expo-haptics"
import { Stack, useRouter } from "expo-router"
import * as Linking from 'expo-linking'
import { StatusBar } from "expo-status-bar"
import React from "react"
import {
    ActivityIndicator, I18nManager, InteractionManager, Pressable, ScrollView,
    Text, View
} from "react-native"
import { Drawer } from "react-native-drawer-layout"
import { SafeAreaProvider, SafeAreaView } from "react-native-safe-area-context"
import { AppHeader } from "@/components/app-header"
import { DrawerThreadItem } from "@/components/drawer/ThreadListItem"
import { ToastOverlay } from "@/components/toast-overlay"
import { Colors, NavigationTheme } from "@/constants/theme"
import {
    applyTypographyGlobals, Typography, useTypographySetup
} from "@/constants/typography"
import { useAutoUpdate } from "@/hooks/use-auto-update"
import { useAppLogStore } from "@/lib/app-log"
import { ensureThreadsRehydrated, useThreads } from "@/lib/threads-store"
import { TinyvexProvider, useTinyvex } from "@/providers/tinyvex"
import { AcpProvider } from "@/providers/acp"
import { DrawerProvider, useDrawer } from "@/providers/drawer"
// Projects/Skills providers temporarily disabled
import { BridgeProvider, useBridge } from "@/providers/ws"
import { useSettings } from "@/lib/settings-store"
import { parseBridgeCode, normalizeBridgeCodeInput } from "@/lib/pairing"
import { AntDesign, Ionicons } from "@expo/vector-icons"
import { ThemeProvider } from "@react-navigation/native"
import { ErrorBoundary } from "@/components/error-boundary"

function DrawerContent() {
  const router = useRouter();
  const { setOpen } = useDrawer();
  // Dev environment flag from env.ts
  const { useIsDevEnv } = require('@/lib/env') as { useIsDevEnv: () => boolean };
  const isDevEnv = useIsDevEnv();
  // Tinyvex history
  // Drawer no longer triggers Tinyvex bootstrap. The provider owns:
  // - `threads` subscribe + initial `threads.list` query on WS connect
  // - bounded prefetch for top threads
  // - throttled message tail queries on live updates
  const { threads, subscribeMessages, queryMessages } = useTinyvex()
  const topThreads = React.useMemo(() => {
    if (!Array.isArray(threads)) return null
    const copy = threads.slice()
    copy.sort((a: any, b: any) => {
      const at = (a?.updated_at ?? a?.updatedAt ?? a?.created_at ?? a?.createdAt ?? 0) as number
      const bt = (b?.updated_at ?? b?.updatedAt ?? b?.created_at ?? b?.createdAt ?? 0) as number
      return bt - at
    })
    return copy.slice(0, 10)
  }, [threads])
  // Drawer deliberately does not warm per-thread messages.
  // TinyvexProvider prefetches a small recent set to avoid connect-time bursts.
  const closeAnd = (fn: () => void) => () => { setOpen(false); fn(); };
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: Colors.sidebarBackground }}>
      <View style={{ flex: 1, backgroundColor: Colors.sidebarBackground }}>
        <ScrollView contentContainerStyle={{ paddingBottom: 24 }} style={{ flex: 1, backgroundColor: Colors.sidebarBackground }}>
          <View style={{ height: 56, justifyContent: 'center', paddingHorizontal: 16 }}>
            <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18 }}>OpenAgents</Text>
          </View>
          <View style={{ paddingHorizontal: 16, gap: 4 }}>
            <View style={{ height: 8 }} />
            {/** Projects section temporarily disabled for v1
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
              <Ionicons name="folder-outline" size={14} color={Colors.secondary} />
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Projects</Text>
            </View>
            <Pressable onPress={closeAnd(() => router.push('/projects'))} accessibilityRole="button" style={{ paddingVertical: 8 }}>
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>See projects…</Text>
            </Pressable>
            {projects.slice(0, 5).map((p) => (
              <Pressable
                key={p.id}
                onPress={closeAnd(async () => {
                  try { await setActive(p.id); } catch {}
                  try { const id = await createThread({ title: p.name || 'New Thread', projectId: p.id }); router.push(`/convex/thread/${encodeURIComponent(String(id))}`); }
                  catch {}
                })}
                accessibilityRole="button"
                style={{ paddingVertical: 8 }}
              >
                <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{p.name}</Text>
              </Pressable>
            ))}
            */}
            <View style={{ height: 8 }} />
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
              <Ionicons name="time-outline" size={14} color={Colors.secondary} />
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>History</Text>
              {/* Tinyvex snapshot loads quickly; spinner optional */}
            </View>
            {Array.isArray(threads) && (
              (topThreads?.filter((r: any) => typeof r.messageCount === 'number' ? r.messageCount > 0 : true).length ?? 0) === 0 ? (
                <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, paddingVertical: 8 }}>No history yet.</Text>
              ) : (
                (topThreads || []).map((row: any) => (
                  <DrawerThreadItem
                    key={String(row.id)}
                    row={row}
                    onPress={closeAnd(() => router.push(`/thread/${encodeURIComponent(String(row.id))}` as any))}
                  />
                ))
              )
            )}
          </View>
        </ScrollView>
        <View style={{ borderTopWidth: 1, borderColor: Colors.border, paddingHorizontal: 16, paddingVertical: 12 }}>
          {/* <Pressable
            onPress={closeAnd(() => router.push('/dashboard' as any))}
            accessibilityRole="button"
            accessibilityLabel="Open dashboard"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
          >
            <AntDesign name="dashboard" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Dashboard</Text>
          </Pressable> */}
          {/** Skills link disabled in drawer
          <Pressable
            onPress={closeAnd(() => router.push('/skills'))}
            accessibilityRole="button"
            accessibilityLabel="Open skills"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
          >
            <Ionicons name="flash-outline" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Skills</Text>
          </Pressable>
          */}
          <Pressable
            onPress={closeAnd(() => router.push('/settings'))}
            accessibilityRole="button"
            accessibilityLabel="Open settings"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
          >
            <Ionicons name="settings-outline" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Settings</Text>
          </Pressable>
          <Pressable
            onPress={closeAnd(() => router.push('/help' as any))}
            accessibilityRole="button"
            accessibilityLabel="Open help"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
          >
            <Ionicons name="help-circle-outline" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Help</Text>
          </Pressable>
          {isDevEnv ? (
            <Pressable
              onPress={closeAnd(() => router.push('/library'))}
              accessibilityRole="button"
              accessibilityLabel="Open component library"
              style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
            >
              <Ionicons name="book-outline" size={18} color={Colors.foreground} />
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Component Library</Text>
            </Pressable>
          ) : null}
          {/** Logs link disabled in drawer
          <Pressable
            onPress={closeAnd(() => router.push('/logs'))}
            accessibilityRole="button"
            accessibilityLabel="Open logs"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
          >
            <Ionicons name="bug-outline" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Logs</Text>
          </Pressable>
          */}
          {/** Convex link disabled in drawer
          <Pressable
            onPress={closeAnd(() => router.push('/convex'))}
            accessibilityRole="button"
            accessibilityLabel="Open Convex status"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
          >
            <Ionicons name="cloud-outline" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Convex</Text>
          </Pressable>
          */}
        </View>
      </View>
    </SafeAreaView>
  );
}

export default function RootLayout() {
  const fontsLoaded = useTypographySetup();
  useAutoUpdate();
  React.useEffect(() => {
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const run = () => {
      if (!cancelled) ensureThreadsRehydrated();
    };
    const handle = InteractionManager?.runAfterInteractions?.(run);
    if (!handle) {
      timer = setTimeout(run, 0);
    }
    return () => {
      cancelled = true;
      if (timer !== null) clearTimeout(timer);
      if (handle && typeof handle.cancel === "function") handle.cancel();
    };
  }, []);
  if (!fontsLoaded) return null;
  applyTypographyGlobals();

  return (
    <SafeAreaProvider>
      <ThemeProvider value={NavigationTheme}>
        <BridgeProvider>
          <AcpProvider>
          <TinyvexProvider>
            <DrawerProvider>
              <LinkingBootstrap />
              <DrawerWrapper />
            </DrawerProvider>
          </TinyvexProvider>
          </AcpProvider>
        </BridgeProvider>
      </ThemeProvider>
    </SafeAreaProvider>
  );
}

function DrawerWrapper() {
  const { open, setOpen } = useDrawer();
  const isRTL = I18nManager.isRTL;
  const router = useRouter();
  const { connected, connecting } = useBridge();
  // Allow dev-only navigation to the component library when disconnected
  const { useIsDevEnv } = require('@/lib/env') as { useIsDevEnv: () => boolean };
  const isDevEnv = useIsDevEnv();
  // Tinyvex migration: avoid Convex hooks here
  const convexThreads: any[] | undefined | null = []
  const pathname = (require('expo-router') as any).usePathname?.() as string | undefined;
  // Connection-gated onboarding: require bridge and convex
  React.useEffect(() => {
    const path = String(pathname || '')
    if (!connected) {
      const allowWhileDisconnected = path.startsWith('/onboarding') || (isDevEnv && path.startsWith('/library')) || (connecting && path.startsWith('/thread'))
      if (!allowWhileDisconnected) {
        try { router.push('/onboarding' as any) } catch {}
      }
    }
  }, [connected, connecting, pathname]);

  const ConnectionDot = () => {
    const { connected } = useBridge();
    return (
      <View style={{ marginLeft: 10 }}>
        <View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: connected ? Colors.success : Colors.danger }} />
      </View>
    );
  };

  const NewChatButton = () => {
    const onPress = async () => {
      try { if (process.env.EXPO_OS === 'ios') { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light); } } catch {}
      try { router.push('/thread/new'); } catch {}
    };
    return (
      <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel="New chat" hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }} style={{ paddingHorizontal: 6, paddingVertical: 6 }}>
        <Ionicons name="add" size={22} color={Colors.foreground} />
      </Pressable>
    );
  };
  return (
    <ErrorBoundary catchErrors="always">
    <Drawer
      open={open}
      onOpen={() => setOpen(true)}
      onClose={() => setOpen(false)}
      drawerType="back"
      drawerPosition={isRTL ? 'right' : 'left'}
      drawerStyle={{ backgroundColor: Colors.sidebarBackground }}
      renderDrawerContent={() => <DrawerContent />}
    >
      <StatusBar style="light" />
      <View style={{ flex: 1 }}>
        {(() => {
          const path = String(pathname || '')
          const hideHeader = path.startsWith('/onboarding')
          return hideHeader ? null : <AppHeader />
        })()}
        <Stack
          screenOptions={{
            contentStyle: { backgroundColor: Colors.background },
            headerShown: false,
            animation: 'none',
          }}
        >
          {/* Removed legacy Message detail; Convex message detail below */}
          <Stack.Screen name="onboarding/index" options={{ headerShown: false }} />
          <Stack.Screen name="dashboard/index" options={{ headerShown: false }} />
          <Stack.Screen name="help/index" options={{ headerShown: false }} />
          <Stack.Screen name="thread/index" options={{ headerShown: false }} />
          <Stack.Screen name="thread/[id]" options={{ animation: 'none' }} />
          <Stack.Screen name="projects/index" />
          {/** Convex screens removed in Tinyvex build */}
          <Stack.Screen name="skills/index" />
          <Stack.Screen name="project/[id]" />
          <Stack.Screen name="project/new" />
          <Stack.Screen name="library/index" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/markdown" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/user-message" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/reasoning-headline" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/reasoning-card" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/exec" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/file-change" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/command" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/search-mcp" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/todo" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/turn-error" options={{ animation: 'slide_from_right' }} />
          {/* ACP-specific library demos */}
          <Stack.Screen name="library/acp-message" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/acp-thought" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/acp-tool-call" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/acp-plan" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/acp-available-commands" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/acp-current-mode" options={{ animation: 'slide_from_right' }} />
          {/* Example conversation + details */}
          <Stack.Screen name="library/acp-example-conversation" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/acp-example-conversation/[id]" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/drawer" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="library/unused" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="settings/index" />
          <Stack.Screen name="logs/index" />
        </Stack>
        {open ? (
          <View pointerEvents="none" style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0, backgroundColor: Colors.white, opacity: 0.04 }} />
        ) : null}
        {/* Toasts overlay (bottom-right corner) */}
        <ToastOverlay />
      </View>
    </Drawer>
    </ErrorBoundary>
  );
}

function LinkingBootstrap() {
  const router = useRouter();
  const { setBridgeHost, connect } = useBridge();
  // Avoid writing Bridge Code into the input field on deep links to prevent UIKit text churn
  const setBridgeToken = useSettings((s) => s.setBridgeToken)
  React.useEffect(() => {
    let cancelled = false
    const handleUrl = (url: string) => {
      if (!url || cancelled) return
      try {
        const parsed = parseBridgeCode(url)
        if (!parsed) return
        // Apply host/token from deep link and auto-connect, skipping onboarding.
        // We intentionally do not set the raw bridgeCode field to avoid TextInput churn.
        try { if (parsed.bridgeHost) setBridgeHost(parsed.bridgeHost) } catch {}
        try { if (parsed.token) setBridgeToken(parsed.token || '') } catch {}
        try { connect() } catch {}
        // While connecting, Drawer gating allows /thread paths; land user in a new thread.
        try { router.replace('/thread/new' as any) } catch {}
      } catch {}
    }
    try { Linking.getInitialURL().then((u) => { if (u) handleUrl(u) }).catch(() => {}) } catch {}
    const sub = Linking.addEventListener('url', (evt) => { try { handleUrl(evt.url) } catch {} }) as any
    return () => { try { cancelled = true; if (sub && typeof sub.remove === 'function') sub.remove() } catch {} }
  }, [])
  return null
}

// Native header removed; title management handled by AppHeader via Zustand store
