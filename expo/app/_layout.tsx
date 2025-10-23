import React from 'react'
import '@/utils/gestureHandler'
import { Stack, useRouter } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { ThemeProvider } from '@react-navigation/native';
import { StatusBar } from 'expo-status-bar';
import { SafeAreaProvider, SafeAreaView } from 'react-native-safe-area-context';
import { Drawer } from 'react-native-drawer-layout';
import { I18nManager, Pressable, ScrollView, Text, View } from 'react-native';
import { useTypographySetup, applyTypographyGlobals, Typography } from '@/constants/typography';
import { Colors, NavigationTheme } from '@/constants/theme';
import { WsProvider, useWs } from '@/providers/ws';
import { ProjectsProvider, useProjects } from '@/providers/projects';
import { useAutoUpdate } from '@/hooks/use-auto-update';
import { AppHeader } from '@/components/app-header'
import { DrawerProvider, useDrawer } from '@/providers/drawer';
import * as Haptics from 'expo-haptics';
import { useSessions } from '@/lib/sessions-store';

function DrawerContent() {
  const router = useRouter();
  const { projects, setActive } = useProjects();
  const { setOpen } = useDrawer();
  const { wsUrl } = useWs();
  const history = useSessions((s) => s.history);
  const loading = useSessions((s) => s.loadingHistory);
  const loadHistory = useSessions((s) => s.loadHistory);
  React.useEffect(() => { loadHistory(wsUrl).catch(() => {}); }, [loadHistory, wsUrl]);
  const closeAnd = (fn: () => void) => () => { setOpen(false); fn(); };
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: Colors.black }}>
      <View style={{ flex: 1 }}>
        <ScrollView contentContainerStyle={{ paddingBottom: 24 }} style={{ flex: 1 }}>
          <View style={{ height: 56, justifyContent: 'center', paddingHorizontal: 16 }}>
            <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 18 }}>OpenAgents</Text>
          </View>
          <View style={{ paddingHorizontal: 16, gap: 8 }}>
            <Pressable onPress={closeAnd(() => router.push('/library'))} accessibilityRole="button" style={{ paddingVertical: 10 }}>
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Component Library</Text>
            </Pressable>
            <View style={{ height: 12 }} />
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Projects</Text>
            <Pressable onPress={closeAnd(() => router.push('/projects'))} accessibilityRole="button" style={{ paddingVertical: 10 }}>
              <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>See projects…</Text>
            </Pressable>
            {projects.slice(0, 5).map((p) => (
              <Pressable key={p.id} onPress={closeAnd(() => { setActive(p.id); router.push('/session'); })} accessibilityRole="button" style={{ paddingVertical: 8 }}>
                <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{p.name}</Text>
              </Pressable>
            ))}
            <View style={{ height: 16 }} />
            <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>History</Text>
            {loading ? (
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, paddingVertical: 8 }}>Loading…</Text>
            ) : history.length === 0 ? (
              <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14, paddingVertical: 8 }}>No history yet.</Text>
            ) : history.slice(0, 5).map((h) => (
              <Pressable key={h.id} onPress={closeAnd(() => router.push(`/session/${encodeURIComponent(h.id)}?path=${encodeURIComponent(h.path)}`))} accessibilityRole="button" style={{ paddingVertical: 8 }}>
                <Text numberOfLines={1} style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>{h.title || '(no title)'}</Text>
                <Text numberOfLines={1} style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>{new Date(h.mtime * 1000).toLocaleString()}</Text>
              </Pressable>
            ))}
          </View>
        </ScrollView>
        <View style={{ borderTopWidth: 1, borderColor: Colors.border, paddingHorizontal: 16, paddingVertical: 12 }}>
          <Pressable
            onPress={closeAnd(() => router.push('/settings'))}
            accessibilityRole="button"
            accessibilityLabel="Open settings"
            style={{ flexDirection: 'row', alignItems: 'center', gap: 10 }}
          >
            <Ionicons name="settings-outline" size={18} color={Colors.foreground} />
            <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Settings</Text>
          </Pressable>
        </View>
      </View>
    </SafeAreaView>
  );
}

export default function RootLayout() {
  const fontsLoaded = useTypographySetup();
  useAutoUpdate();
  if (!fontsLoaded) return null;
  applyTypographyGlobals();

  return (
    <SafeAreaProvider>
      <ThemeProvider value={NavigationTheme}>
        <WsProvider>
          <ProjectsProvider>
            <DrawerProvider>
              <DrawerWrapper />
            </DrawerProvider>
          </ProjectsProvider>
        </WsProvider>
      </ThemeProvider>
    </SafeAreaProvider>
  );
}

function DrawerWrapper() {
  const { open, setOpen } = useDrawer();
  const isRTL = I18nManager.isRTL;
  const router = useRouter();

  const ConnectionDot = () => {
    const { connected } = useWs();
    return (
      <View style={{ marginLeft: 10 }}>
        <View style={{ width: 8, height: 8, borderRadius: 4, backgroundColor: connected ? Colors.success : Colors.danger }} />
      </View>
    );
  };

  const NewChatButton = () => {
    const { clearLog, setResumeNextId } = useWs();
    const onPress = async () => {
      try { if (process.env.EXPO_OS === 'ios') { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light); } } catch {}
      // Force the next send to start a fresh session (no resume)
      try { setResumeNextId('new') } catch {}
      clearLog();
      router.push('/session');
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
      renderDrawerContent={() => <DrawerContent />}
    >
      <StatusBar style="light" />
      <View
        style={{
          flex: 1,
          borderLeftWidth: isRTL ? 0 : 1,
          borderRightWidth: isRTL ? 1 : 0,
          borderColor: Colors.border,
          // Hide the divider when closed by nudging it off-screen
          ...(isRTL ? { marginRight: open ? 0 : -1 } : { marginLeft: open ? 0 : -1 }),
        }}
      >
        <AppHeader />
        <Stack
          screenOptions={{
            contentStyle: { backgroundColor: Colors.background },
            headerShown: false,
            animation: 'none',
          }}
        >
          {/* Removed tabs; declare screens individually */}
          <Stack.Screen name="message/[id]" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="session/index" options={{ headerShown: false }} />
          <Stack.Screen name="session/[id]" options={{ animation: 'slide_from_right' }} />
          <Stack.Screen name="projects/index" />
          <Stack.Screen name="project/[id]" />
          <Stack.Screen name="project/new" />
          <Stack.Screen name="library/index" />
          <Stack.Screen name="settings/index" />
          <Stack.Screen name="session/[id]" options={{ animation: 'slide_from_right' }} />
        </Stack>
        {open ? (
          <View pointerEvents="none" style={{ position: 'absolute', left: 0, right: 0, top: 0, bottom: 0, backgroundColor: Colors.white, opacity: 0.04 }} />
        ) : null}
      </View>
    </Drawer>
  );
}

// Native header removed; title management handled by AppHeader via Zustand store
