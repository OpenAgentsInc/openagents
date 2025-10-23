import { Stack, useRouter } from 'expo-router';
import React from 'react';

export default function TabLayout() {
  useRouter();
  return (
    <Stack initialRouteName="session" screenOptions={{ headerShown: false, animation: 'none' }}>
      <Stack.Screen name="dashboard" />
      <Stack.Screen name="projects" />
      <Stack.Screen name="session" />
      <Stack.Screen name="history" />
      <Stack.Screen name="library" />
      <Stack.Screen name="settings" />
    </Stack>
  );
}
