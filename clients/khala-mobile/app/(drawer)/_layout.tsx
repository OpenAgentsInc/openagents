import { Drawer } from "expo-router/drawer"

import { khalaMobileTheme } from "../../src/theme/tokens"

export default function DrawerLayout() {
  return (
    <Drawer
      screenOptions={{
        drawerActiveTintColor: khalaMobileTheme.text,
        drawerInactiveTintColor: khalaMobileTheme.textMuted,
        drawerStyle: { backgroundColor: khalaMobileTheme.surface },
        headerShown: false,
        sceneStyle: { backgroundColor: khalaMobileTheme.background }
      }}
    >
      <Drawer.Screen name="index" options={{ title: "Khala" }} />
      <Drawer.Screen name="settings" options={{ title: "Settings" }} />
    </Drawer>
  )
}
