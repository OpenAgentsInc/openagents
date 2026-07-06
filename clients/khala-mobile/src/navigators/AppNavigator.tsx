import { createDrawerNavigator } from "@react-navigation/drawer"
import {
  DarkTheme,
  NavigationContainer,
  type LinkingOptions,
  type Theme,
} from "@react-navigation/native"
import { createNativeStackNavigator } from "@react-navigation/native-stack"
import * as Linking from "expo-linking"

import { RepoPickerScreen } from "../screens/repo-picker-screen"
import { SettingsScreen } from "../screens/settings-screen"
import { ThreadListScreen } from "../screens/thread-list-screen"
import { ThreadMessagesScreen } from "../screens/thread-messages-screen"
import { tx } from "../i18n/copy"
import { khalaMobileTheme } from "../theme/tokens"
import { navigationRef, useBackButtonHandler } from "./navigationUtilities"
import type { AppDrawerParamList, AppStackParamList } from "./navigationTypes"

const Drawer = createDrawerNavigator<AppDrawerParamList>()
const Stack = createNativeStackNavigator<AppStackParamList>()

const navigationTheme: Theme = {
  ...DarkTheme,
  colors: {
    ...DarkTheme.colors,
    background: khalaMobileTheme.background,
    border: khalaMobileTheme.border,
    card: khalaMobileTheme.surface,
    notification: khalaMobileTheme.accent,
    primary: khalaMobileTheme.accent,
    text: khalaMobileTheme.text,
  },
}

const linking: LinkingOptions<AppStackParamList> = {
  config: {
    screens: {
      Home: {
        screens: {
          Settings: "settings",
          Threads: "",
        },
      },
      ThreadMessages: "thread/:threadId",
      RepoPicker: "thread/:threadId/repo",
    },
  },
  prefixes: [Linking.createURL("/")],
}

const AppDrawerNavigator = () => (
  <Drawer.Navigator
    screenOptions={{
      drawerActiveTintColor: khalaMobileTheme.text,
      drawerInactiveTintColor: khalaMobileTheme.textMuted,
      drawerStyle: { backgroundColor: khalaMobileTheme.surface },
      headerShown: false,
      sceneStyle: { backgroundColor: khalaMobileTheme.background },
    }}
  >
    <Drawer.Screen name="Threads" component={ThreadListScreen} options={{ title: tx("nav.threads") }} />
    <Drawer.Screen name="Settings" component={SettingsScreen} options={{ title: tx("nav.settings") }} />
  </Drawer.Navigator>
)

export const AppNavigator = () => {
  useBackButtonHandler(routeName => routeName === "Threads")

  return (
    <NavigationContainer linking={linking} ref={navigationRef} theme={navigationTheme}>
      <Stack.Navigator
        screenOptions={{
          contentStyle: { backgroundColor: khalaMobileTheme.background },
          headerShown: false,
        }}
      >
        <Stack.Screen name="Home" component={AppDrawerNavigator} />
        <Stack.Screen name="ThreadMessages" component={ThreadMessagesScreen} />
        <Stack.Screen name="RepoPicker" component={RepoPickerScreen} options={{ presentation: "modal" }} />
      </Stack.Navigator>
    </NavigationContainer>
  )
}
