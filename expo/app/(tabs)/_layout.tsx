import { Stack, useRouter } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { Colors } from '@/constants/theme';
import { Pressable, View, Text } from 'react-native';
import { useWs } from '@/providers/ws';
import React from 'react';
import { Typography } from '@/constants/typography';
import * as Haptics from 'expo-haptics';
import { useDrawer } from '@/providers/drawer';

export default function TabLayout() {
  const router = useRouter();
  const drawer = useDrawer();
  const ConnectionDot = () => {
    const { connected } = useWs();
    return (
      <View style={{ marginLeft: 10 }}>
        <View style={{ width: 10, height: 10, borderRadius: 5, backgroundColor: connected ? '#22C55E' : '#EF4444' }} />
      </View>
    );
  };
  
  const NewChatButton = () => {
    const { clearLog } = useWs();
    const onPress = async () => {
      try {
        if (process.env.EXPO_OS === 'ios') {
          await Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
        }
      } catch {}
      // Start a fresh chat: clear current log and focus Session tab
      clearLog();
      router.push('/(tabs)/session');
    };
    return (
      <Pressable
        onPress={onPress}
        accessibilityRole="button"
        accessibilityLabel="New chat"
        hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
        style={{ paddingHorizontal: 6, paddingVertical: 6 }}
      >
        <Ionicons name="add" size={22} color={Colors.textPrimary} />
      </Pressable>
    );
  };
  return (
    <Stack
      initialRouteName="session"
      screenOptions={{
        headerShown: true,
        animation: 'none',
        headerTitleStyle: { fontFamily: Typography.bold },
        headerStyle: { backgroundColor: Colors.background },
        headerBackground: () => (
          <View style={{ flex: 1, backgroundColor: Colors.background, borderBottomWidth: 1, borderBottomColor: Colors.border }} />
        ),
        headerShadowVisible: false,
        headerTintColor: Colors.textPrimary,
        headerLeft: () => (
          <Pressable
            onPress={drawer.toggle}
            accessibilityRole="button"
            hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
            style={{ paddingHorizontal: 6, paddingVertical: 6 }}
          >
            <Ionicons name="menu" size={22} color={Colors.textPrimary} />
          </Pressable>
        ),
        headerRight: () => (
          <View style={{ flexDirection: 'row', alignItems: 'center', marginRight: 12 }}>
            <NewChatButton />
            <ConnectionDot />
          </View>
        ),
      }}
    >
      <Stack.Screen name="dashboard" options={{ title: 'Dashboard' }} />
      <Stack.Screen name="projects" options={{ title: 'Projects' }} />
      <Stack.Screen
        name="session"
        options={{
          headerTitle: () => null,
          headerLeft: () => (
            <View style={{ flexDirection: 'row', alignItems: 'center' }}>
              <Pressable
                onPress={drawer.toggle}
                accessibilityRole="button"
                hitSlop={{ top: 8, bottom: 8, left: 8, right: 8 }}
                style={{ paddingHorizontal: 6, paddingVertical: 6 }}
              >
                <Ionicons name="menu" size={22} color={Colors.textPrimary} />
              </Pressable>
              <Text style={{ color: Colors.textPrimary, fontFamily: Typography.primary, fontSize: 16, marginLeft: 6 }}>
                New session
              </Text>
            </View>
          ),
        }}
      />
      <Stack.Screen name="history" options={{ title: 'History' }} />
      <Stack.Screen name="library" options={{ title: 'Component Library' }} />
      <Stack.Screen name="settings" options={{ title: 'Settings' }} />
    </Stack>
  );
}
