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
    Text, View, ActionSheetIOS, Platform
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
import { usePairingStore } from "@/lib/pairing-store"
import { AntDesign, Ionicons } from "@expo/vector-icons"
import { ThemeProvider } from "@react-navigation/native"
import { ErrorBoundary } from "@/components/error-boundary"
import { useUpdateStore } from "@/lib/update-store"
import { useArchiveStore } from "@/lib/archive-store"

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
  const { isArchived, archive, unarchive } = useArchiveStore()
  const archivedMap = useArchiveStore((s) => s.archived)
  const [menuFor, setMenuFor] = React.useState<string | null>(null)
  const topThreads = React.useMemo(() => {
    if (!Array.isArray(threads)) return null
    const copy = threads
      .slice()
      .filter((r) => {
        const tid = String(r.id || '')
        return tid && !isArchived(tid)
      })
      .sort((a, b) => {
        const at = Number(a.updated_at ?? a.created_at ?? 0)
        const bt = Number(b.updated_at ?? b.created_at ?? 0)
        return bt - at
      })
    return copy.slice(0, 10)
  }, [threads, archivedMap])
  // Drawer deliberately does not warm per-thread messages.
  // TinyvexProvider prefetches a small recent set to avoid connect-time bursts.
  const closeAnd = (fn: () => void) => () => { setOpen(false); fn(); };
  const showActions = (threadId: string) => {
    if (!threadId) return
    if (Platform.OS === 'ios') {
      try {
        const archived = isArchived(threadId)
        ActionSheetIOS.showActionSheetWithOptions(
          {
            options: [archived ? 'Unarchive' : 'Archive', 'Cancel'],
            cancelButtonIndex: 1,
            userInterfaceStyle: 'dark',
          },
          (idx) => {
            if (idx === 0) {
              try { archived ? unarchive(threadId) : archive(threadId) } catch {}
            }
          }
        )
      } catch {}
    } else {
      setMenuFor(threadId)
    }
  }

  const closeMenu = () => setMenuFor(null)

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
                    onLongPress={() => showActions(String(row.id))}
                  />
                ))
              )
            )}
            {/* Archived link moved below History */}
            <View style={{ height: 12 }} />
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
              <Ionicons name="archive-outline" size={14} color={Colors.secondary} />
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Archived</Text>
            </View>
            <Pressable onPress={closeAnd(() => router.push('/thread/archived' as any))} accessibilityRole="button" style={{ paddingVertical: 8 }}>
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>View archived</Text>
            </Pressable>
          </View>
        </ScrollView>
        {/* Inline action menu for Android/Web */}
        {menuFor ? (
          <Pressable onPress={closeMenu} style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0 }}>
            <View pointerEvents="box-none" style={{ flex: 1 }}>
              <View style={{ position: 'absolute', right: 12, top: 84, backgroundColor: Colors.card, borderWidth: 1, borderColor: Colors.border, paddingVertical: 6, paddingHorizontal: 8 }}>
                <Pressable onPress={() => { try { (isArchived(menuFor) ? unarchive(menuFor) : archive(menuFor)) } catch {}; closeMenu() }} accessibilityRole="button" style={{ paddingVertical: 6 }}>
                  <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 14 }}>{isArchived(menuFor) ? 'Unarchive' : 'Archive'}</Text>
                </Pressable>
                <View style={{ height: 6 }} />
                <Pressable onPress={closeMenu} accessibilityRole="button" style={{ paddingVertical: 6 }}>
                  <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14 }}>Cancel</Text>
                </Pressable>
              </View>
            </View>
          </Pressable>
        ) : null}
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
            testID="drawer-settings"
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
              testID="drawer-library"
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
  // Always call hooks unconditionally and outside try/catch to keep order stable
  const updating = useUpdateStore((s) => s.updating)
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
  // Show updating overlay first to avoid hook-order shifts during OTA state
  if (updating) {
    return (
      <SafeAreaProvider>
        <ThemeProvider value={NavigationTheme}>
          <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
            <ActivityIndicator size="large" color={Colors.foreground} />
            <Text style={{ marginTop: 10, color: Colors.secondary, fontFamily: Typography.bold, fontSize: 16 }}>Updating</Text>
          </View>
        </ThemeProvider>
      </SafeAreaProvider>
    )
  }

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
  const setLastRoute = useSettings((s) => s.setLastRoute)
  const lastRoute = useSettings((s) => s.lastRoute)
  // Allow dev-only navigation to the component library when disconnected
  const { useIsDevEnv } = require('@/lib/env') as { useIsDevEnv: () => boolean };
  const isDevEnv = useIsDevEnv();
  // Tinyvex migration: avoid Convex hooks here
  const convexThreads: any[] | undefined | null = []
  const pathname = (require('expo-router') as any).usePathname?.() as string | undefined;
  // Track last visited route so we can restore after OTA reloads
  React.useEffect(() => {
    const path = String(pathname || '')
    if (path) {
      try { setLastRoute(path) } catch {}
    }
  }, [pathname])

  // Do not force navigation on disconnect; keep user on current screen and just update the connection dot.

  // When connected, if we are still on onboarding, immediately move to a new thread.
  React.useEffect(() => {
    const path = String(pathname || '')
    if (connected && path.startsWith('/onboarding')) {
      // Restore last route if we have one; otherwise go to a new thread
      try { router.replace((lastRoute && !lastRoute.startsWith('/onboarding')) ? (lastRoute as any) : ('/thread/new' as any)) } catch {}
      try { usePairingStore.getState().setDeeplinkPairing(false) } catch {}
    }
  }, [connected, pathname, lastRoute])

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
          <Stack.Screen name="onboarding/manual-code" options={{ headerShown: false }} />
          <Stack.Screen name="dashboard/index" options={{ headerShown: false }} />
          <Stack.Screen name="help/index" options={{ headerShown: false }} />
          <Stack.Screen name="thread/[id]" options={{ animation: 'none' }} />
          <Stack.Screen name="thread/archived" options={{ headerShown: false }} />
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
  const setDeeplinkPairing = usePairingStore((s) => s.setDeeplinkPairing)
  React.useEffect(() => {
    let cancelled = false
    const handleUrl = (url: string) => {
      if (!url || cancelled) return
      try {
        const parsed = parseBridgeCode(url)
        if (!parsed) return
        try { setDeeplinkPairing(true) } catch {}
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
