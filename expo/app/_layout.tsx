import "@/utils/gestureHandler"
import { useQuery } from "convex/react"
import * as Haptics from "expo-haptics"
import { Stack, useRouter } from "expo-router"
import { StatusBar } from "expo-status-bar"
import React from "react"
import {
    ActivityIndicator, I18nManager, InteractionManager, Pressable, ScrollView,
    Text, View
} from "react-native"
import { Drawer } from "react-native-drawer-layout"
import { SafeAreaProvider, SafeAreaView } from "react-native-safe-area-context"
import { AppHeader } from "@/components/app-header"
import { Colors, NavigationTheme } from "@/constants/theme"
import {
    applyTypographyGlobals, Typography, useTypographySetup
} from "@/constants/typography"
import { useAutoUpdate } from "@/hooks/use-auto-update"
import { useAppLogStore } from "@/lib/app-log"
import { useOnboarding } from "@/lib/onboarding-store"
import { ensureThreadsRehydrated, useThreads } from "@/lib/threads-store"
import { ConvexProviderLocal } from "@/providers/convex"
import { DrawerProvider, useDrawer } from "@/providers/drawer"
import { ProjectsProvider, useProjects } from "@/providers/projects"
import { SkillsProvider } from "@/providers/skills"
import { BridgeProvider, useBridge } from "@/providers/ws"
import { AntDesign, Ionicons } from "@expo/vector-icons"
import { ThemeProvider } from "@react-navigation/native"

function DrawerContent() {
  const router = useRouter();
  const { projects, setActive } = useProjects();
  const { setOpen } = useDrawer();
  // Convex-only history
  const convexThreads = (useQuery as any)('threads:list', {}) as any[] | undefined | null
  const topThreads = React.useMemo(() => {
    if (!Array.isArray(convexThreads)) return null
    const copy = convexThreads.slice()
    copy.sort((a: any, b: any) => {
      const at = (a?.updatedAt ?? a?.createdAt ?? 0) as number
      const bt = (b?.updatedAt ?? b?.createdAt ?? 0) as number
      return bt - at
    })
    return copy.slice(0, 10)
  }, [convexThreads])
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
            {/** Projects links temporarily disabled in drawer
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
              <Ionicons name="folder-outline" size={14} color={Colors.secondary} />
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Projects</Text>
            </View>
            <Pressable onPress={closeAnd(() => router.push('/projects'))} accessibilityRole="button" style={{ paddingVertical: 8 }}>
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>See projects…</Text>
            </Pressable>
            {projects.slice(0, 5).map((p) => (
              <Pressable key={p.id} onPress={closeAnd(() => { setActive(p.id); router.push('/thread?focus=1&new=1'); })} accessibilityRole="button" style={{ paddingVertical: 8 }}>
                <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{p.name}</Text>
              </Pressable>
            ))}
            */}
            <View style={{ height: 8 }} />
            <View style={{ flexDirection: 'row', alignItems: 'center', gap: 6 }}>
              <Ionicons name="time-outline" size={14} color={Colors.secondary} />
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>History</Text>
              {convexThreads === undefined ? (
                <ActivityIndicator size="small" color={Colors.secondary} />
              ) : null}
            </View>
            {Array.isArray(convexThreads) && (
              (topThreads?.length ?? 0) === 0 ? (
                <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, paddingVertical: 8 }}>No history yet.</Text>
              ) : (
                (topThreads || []).map((row: any) => (
                  <View key={String(row._id || row.id)} style={{ paddingVertical: 6 }}>
                    <Pressable onPress={closeAnd(() => {
                      router.push(`/convex/thread/${encodeURIComponent(row._id || row.id)}`)
                    })} accessibilityRole="button">
                      <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{row.title || '(no title)'}</Text>
                      <Text numberOfLines={1} style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{new Date(((row.updatedAt ?? row.createdAt) ?? Date.now())).toLocaleString()}</Text>
                    </Pressable>
                  </View>
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
          <Pressable
            onPress={closeAnd(() => router.push('/library'))}
            accessibilityRole="button"
            accessibilityLabel="Open component library"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10, paddingVertical: 8 }}
          >
            <Ionicons name="book-outline" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Component Library</Text>
          </Pressable>
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
          <ProjectsProvider>
            <SkillsProvider>
              <ConvexProviderLocal>
                <DrawerProvider>
                  <DrawerWrapper />
                </DrawerProvider>
              </ConvexProviderLocal>
            </SkillsProvider>
          </ProjectsProvider>
        </BridgeProvider>
      </ThemeProvider>
    </SafeAreaProvider>
  );
}

function DrawerWrapper() {
  const { open, setOpen } = useDrawer();
  const isRTL = I18nManager.isRTL;
  const router = useRouter();
  const onboarding = useOnboarding();
  React.useEffect(() => {
    if (!onboarding.rehydrated) return;
    if (!onboarding.completed) {
      try { router.push('/onboarding' as any) } catch {}
    }
  }, [onboarding.rehydrated, onboarding.completed]);

  const ConnectionDot = () => {
    const { connected } = useBridge();
    return (
      <View style={{ marginLeft: 10 }}>
        <View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: connected ? Colors.success : Colors.danger }} />
      </View>
    );
  };

  const NewChatButton = () => {
    const createThread = (require('convex/react') as any).useMutation('threads:create') as (args?: { title?: string; projectId?: string }) => Promise<string>;
    const onPress = async () => {
      try { if (process.env.EXPO_OS === 'ios') { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light); } } catch {}
      try { const id = await createThread({ title: 'New Thread' }); router.push(`/convex/thread/${encodeURIComponent(String(id))}`); }
      catch { router.push('/thread?focus=1&new=1'); }
    };
    return (
      <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel="New chat" hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }} style={{ paddingHorizontal: 6, paddingVertical: 6 }}>
        <Ionicons name="add" size={22} color={Colors.foreground} />
      </Pressable>
    );
  };
  return (
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
        <AppHeader />
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
          <Stack.Screen name="convex/thread/[id]/metadata" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="convex/message/[id]" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="skills/index" />
          <Stack.Screen name="project/[id]" />
          <Stack.Screen name="project/new" />
          <Stack.Screen name="library/index" />
          <Stack.Screen name="library/markdown" />
          <Stack.Screen name="library/reasoning-headline" />
          <Stack.Screen name="library/reasoning-card" />
          <Stack.Screen name="library/exec" />
          <Stack.Screen name="library/file-change" />
          <Stack.Screen name="library/command" />
          <Stack.Screen name="library/search-mcp" />
          <Stack.Screen name="library/todo" />
          <Stack.Screen name="library/turn-error" />
          <Stack.Screen name="library/unused" />
          <Stack.Screen name="settings/index" />
          <Stack.Screen name="logs/index" />
        </Stack>
        {open ? (
          <View pointerEvents="none" style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0, backgroundColor: Colors.white, opacity: 0.04 }} />
        ) : null}
      </View>
    </Drawer>
  );
}

// Native header removed; title management handled by AppHeader via Zustand store
