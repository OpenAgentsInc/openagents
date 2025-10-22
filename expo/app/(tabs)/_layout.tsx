import { Tabs, useRouter } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { Colors } from '@/constants/theme';
import { Pressable, View } from 'react-native';
import { useWs } from '@/providers/ws';
import React from 'react';
import { Typography } from '@/constants/typography';
import { HapticTab } from '@/components/haptic-tab';
import * as Haptics from 'expo-haptics';

export default function TabLayout() {
  const router = useRouter();
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
    <Tabs
      initialRouteName="session"
      screenOptions={{
        headerShown: true,
        headerTitleStyle: { fontFamily: Typography.bold },
        headerStyle: { backgroundColor: Colors.background },
        headerTintColor: Colors.textPrimary,
        headerRight: () => (
          <View style={{ flexDirection: 'row', alignItems: 'center', marginRight: 12 }}>
            <NewChatButton />
            <ConnectionDot />
          </View>
        ),
        tabBarShowLabel: false,
        tabBarActiveTintColor: Colors.tabBarActive,
        tabBarInactiveTintColor: Colors.tabBarInactive,
        tabBarButton: HapticTab,
        tabBarStyle: {
          backgroundColor: Colors.tabBarBackground,
          borderTopColor: Colors.border,
        },
      }}
    >
      <Tabs.Screen name="session" options={{ title: 'Session', tabBarIcon: ({ color, size }) => (<Ionicons name="grid" size={size} color={color} />) }} />
      <Tabs.Screen name="index" options={{ href: null }} />
      <Tabs.Screen
        name="history"
        options={{
          title: 'History',
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="time" size={size} color={color} />
          ),
        }}
      />
      <Tabs.Screen
        name="library"
        options={{
          title: 'Library',
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="book" size={size} color={color} />
          ),
        }}
      />
      <Tabs.Screen
        name="settings"
        options={{
          title: 'Settings',
          tabBarIcon: ({ color, size }) => (
            <Ionicons name="settings" size={size} color={color} />
          ),
        }}
      />
    </Tabs>
  );
}
