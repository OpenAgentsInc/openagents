import { Tabs } from 'expo-router';
import { Ionicons } from '@expo/vector-icons';
import { Colors } from '@/constants/theme';
import { View } from 'react-native';
import { useWs } from '@/providers/ws';
import React from 'react';
import { Typography } from '@/constants/typography';
import { HapticTab } from '@/components/haptic-tab';

export default function TabLayout() {
  const ConnectionDot = () => {
    const { connected } = useWs();
    return (
      <View style={{ marginRight: 12 }}>
        <View style={{ width: 10, height: 10, borderRadius: 5, backgroundColor: connected ? '#22C55E' : '#EF4444' }} />
      </View>
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
        headerRight: () => <ConnectionDot />,
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
