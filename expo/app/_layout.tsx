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
import { DrawerProvider, useDrawer } from '@/providers/drawer';
import * as Haptics from 'expo-haptics';
import { getAllLogs, loadLogs, subscribe } from '@/lib/log-store';

function DrawerContent() {
  const router = useRouter();
  const { projects, setActive } = useProjects();
  const { setOpen } = useDrawer();
  const logs = React.useSyncExternalStore(subscribe, getAllLogs, getAllLogs);
  const [hydrating, setHydrating] = React.useState(true);
  React.useEffect(() => {
    let alive = true;
    loadLogs().catch(() => {}).finally(() => { if (alive) setHydrating(false); });
    return () => { alive = false; };
  }, []);
  const userMsgs = React.useMemo(
    () =>
      logs
        .filter((l) => typeof l.text === 'string' && /^\s*>/.test(l.text))
        .slice(-10)
        .reverse(),
    [logs],
  );
  const closeAnd = (fn: () => void) => () => { setOpen(false); fn(); };
  return (
    <SafeAreaView style={{ flex: 1, backgroundColor: Colors.background }}>
      <ScrollView contentContainerStyle={{ paddingBottom: 24 }} style={{ flex: 1 }}>
        <View style={{ height: 56, justifyContent: 'center', paddingHorizontal: 16 }}>
          <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 18 }}>OpenAgents</Text>
        </View>
        <View style={{ paddingHorizontal: 16, gap: 8 }}>
          <Pressable onPress={closeAnd(() => router.push('/(tabs)/library'))} accessibilityRole="button" style={{ paddingVertical: 10 }}>
            <Text style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 16 }}>Component Library</Text>
          </Pressable>
          <View style={{ height: 12 }} />
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 }}>Projects</Text>
          <Pressable onPress={closeAnd(() => router.push('/(tabs)/projects'))} accessibilityRole="button" style={{ paddingVertical: 10 }}>
            <Text style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 16 }}>See projects…</Text>
          </Pressable>
          {projects.slice(0, 5).map((p) => (
            <Pressable key={p.id} onPress={closeAnd(() => { setActive(p.id); router.push('/(tabs)/session'); })} accessibilityRole="button" style={{ paddingVertical: 8 }}>
              <Text style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 16 }}>{p.name}</Text>
            </Pressable>
          ))}
          <View style={{ height: 16 }} />
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 }}>History</Text>
          {hydrating ? (
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 14, paddingVertical: 8 }}>Loading…</Text>
          ) : userMsgs.length === 0 ? (
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 14, paddingVertical: 8 }}>No history yet.</Text>
          ) : userMsgs.map((m) => {
            const clean = String(m.text).replace(/^\s*>\s?/, '');
            return (
              <Pressable key={m.id} onPress={closeAnd(() => router.push('/(tabs)/session'))} accessibilityRole="button" style={{ paddingVertical: 8 }}>
                <Text numberOfLines={1} style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 16 }}>{clean}</Text>
              </Pressable>
            );
          })}
        </View>
      </ScrollView>
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
        <View style={{ width: 10, height: 10, borderRadius: 5, backgroundColor: connected ? Colors.statusSuccess : Colors.statusError }} />
      </View>
    );
  };

  const NewChatButton = () => {
    const { clearLog } = useWs();
    const onPress = async () => {
      try { if (process.env.EXPO_OS === 'ios') { await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light); } } catch {}
      clearLog();
      router.push('/(tabs)/session');
    };
    return (
      <Pressable onPress={onPress} accessibilityRole="button" accessibilityLabel="New chat" hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }} style={{ paddingHorizontal: 6, paddingVertical: 6 }}>
        <Ionicons name="add" size={22} color={Colors.textPrimary} />
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
        <Stack
          screenOptions={({ route, navigation }) => ({
            contentStyle: { backgroundColor: Colors.background },
            headerShown: true,
            animation: 'none',
            // No hairline here; (tabs) header draws the visible divider
            headerTitle: '',
            headerTitleStyle: { fontFamily: Typography.bold },
            headerStyle: { backgroundColor: Colors.background },
            headerTitleAlign: 'left',
            headerLeftContainerStyle: { marginLeft: 0, paddingLeft: 0 },
            headerLeft: () => {
              const canGoBack = navigation.canGoBack();
              const routeName = String(route.name);
              const title = routeTitle(routeName);
              return (
                <View style={{ flexDirection: 'row', alignItems: 'center' }}>
                  <Pressable
                    onPress={() => { if (canGoBack) navigation.goBack(); else setOpen(true); }}
                    accessibilityRole="button"
                    hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
                    style={{ paddingHorizontal: 6, paddingVertical: 6 }}
                  >
                    <Ionicons name={canGoBack ? 'chevron-back' : 'menu'} size={22} color={Colors.textPrimary} />
                  </Pressable>
                  <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold, fontSize: 16, marginLeft: 6 }}>{title}</Text>
                </View>
              );
            },
            headerRight: () => (
              <View style={{ flexDirection: 'row', alignItems: 'center', marginRight: 12 }}>
                <NewChatButton />
                <ConnectionDot />
              </View>
            ),
          })}
        >
          <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
          <Stack.Screen name="message/[id]" options={{ animation: 'slide_from_right' }} />
        </Stack>
      </View>
    </Drawer>
  );
}

function routeTitle(name: string): string {
  if (name === '(tabs)') return '';
  if (name === 'message/[id]') return 'Message';
  if (name === 'project/[id]') return 'Project';
  if (name === 'project/new') return 'New Project';
  return name.replace(/\[(.*?)\]/g, '$1').replace(/\b\w/g, (c) => c.toUpperCase());
}
