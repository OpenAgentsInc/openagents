import React from 'react'
import '@/utils/gestureHandler'
import { Stack, useRouter } from 'expo-router';
import { ThemeProvider } from '@react-navigation/native';
import { StatusBar } from 'expo-status-bar';
import { SafeAreaProvider, SafeAreaView } from 'react-native-safe-area-context';
import { Drawer } from 'react-native-drawer-layout';
import { I18nManager, Pressable, ScrollView, Text, View } from 'react-native';
import { useTypographySetup, applyTypographyGlobals, Typography } from '@/constants/typography';
import { Colors, NavigationTheme } from '@/constants/theme';
import { WsProvider } from '@/providers/ws';
import { ProjectsProvider, useProjects } from '@/providers/projects';
import { useAutoUpdate } from '@/hooks/use-auto-update';
import { DrawerProvider, useDrawer } from '@/providers/drawer';
import { getAllLogs, loadLogs } from '@/lib/log-store';

function DrawerContent() {
  const router = useRouter();
  const { projects, setActive } = useProjects();
  const { setOpen } = useDrawer();
  const isRTL = I18nManager.isRTL;
  const [_, __] = React.useState(0);
  React.useEffect(() => { (async ()=>{ await loadLogs(); __((n)=>n+1) })(); }, []);
  const userMsgs = getAllLogs().filter((l) => typeof l.text === 'string' && /^\s*>/.test(l.text)).slice(-10).reverse();
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
          {userMsgs.length === 0 ? (
            <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 14, paddingVertical: 8 }}>No history yet.</Text>
          ) : userMsgs.map((m) => {
            const clean = String(m.text).replace(/^\s*>\s?/, '');
            return (
              <Pressable key={m.id} onPress={closeAnd(() => router.push(`/message/${m.detailId ?? m.id}`))} accessibilityRole="button" style={{ paddingVertical: 8 }}>
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
        <Stack screenOptions={{ contentStyle: { backgroundColor: Colors.background }, headerShown: false }}>
          <Stack.Screen name="(tabs)" options={{ headerShown: false }} />
        </Stack>
      </View>
    </Drawer>
  );
}
