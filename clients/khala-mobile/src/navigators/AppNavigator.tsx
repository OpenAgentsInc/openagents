import {
  createDrawerNavigator,
  DrawerContentScrollView,
  DrawerItemList,
  type DrawerContentComponentProps,
} from "@react-navigation/drawer"
import {
  DarkTheme,
  NavigationContainer,
  type LinkingOptions,
  type Theme,
} from "@react-navigation/native"
import { createNativeStackNavigator } from "@react-navigation/native-stack"
import * as Linking from "expo-linking"
import { View, type ViewStyle } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { DrawerCreditsBalance } from "../components/drawer-credits-balance"
import { AboutEffectNativeScreen } from "../screens/about-effect-native-screen"
import { CreditsHistoryScreen } from "../screens/credits-history-screen"
import { FleetPeekScreen } from "../screens/fleet-peek-screen"
import { RepoPickerScreen } from "../screens/repo-picker-screen"
import { SettingsScreen } from "../screens/settings-screen"
import { ThreadListScreen } from "../screens/thread-list-screen"
import { ThreadMessagesScreen } from "../screens/thread-messages-screen"
import { tx } from "../i18n/copy"
import { usePushNotificationDeepLink } from "../push/use-push-notification-deep-link"
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

const linking: LinkingOptions<AppDrawerParamList> = {
  config: {
    screens: {
      Main: {
        screens: {
          Threads: "",
          ThreadMessages: "thread/:threadId",
          RepoPicker: "thread/:threadId/repo",
          CreditsHistory: "credits/history",
        },
      },
      FleetPeek: "fleet",
      Settings: "settings",
      AboutEffectNative: "about-effect-native",
    },
  },
  prefixes: [Linking.createURL("/")],
}

/** The threads area — list, thread/chat view, repo picker, and credit history —
 * as a native stack nested inside the root Drawer's "Main" screen. The chat
 * header's hamburger opens the drawer from any of these via
 * `navigation.getParent()?.openDrawer()`. */
const MainStackNavigator = () => (
  <Stack.Navigator
    screenOptions={{
      contentStyle: { backgroundColor: khalaMobileTheme.background },
      headerShown: false,
    }}
  >
    <Stack.Screen name="Threads" component={ThreadListScreen} />
    <Stack.Screen name="ThreadMessages" component={ThreadMessagesScreen} />
    <Stack.Screen name="RepoPicker" component={RepoPickerScreen} options={{ presentation: "modal" }} />
    <Stack.Screen name="CreditsHistory" component={CreditsHistoryScreen} />
  </Stack.Navigator>
)

/** Custom drawer flyout: the standard nav items, then the live credit balance
 * pinned at the very bottom (owner request, 2026-07-07). */
const AppDrawerContent = (props: DrawerContentComponentProps) => (
  <SafeAreaView style={$drawerContent} edges={["top", "bottom"]}>
    <DrawerContentScrollView {...props} contentContainerStyle={$drawerScroll}>
      <DrawerItemList {...props} />
    </DrawerContentScrollView>
    <View style={$drawerFooter}>
      <DrawerCreditsBalance />
    </View>
  </SafeAreaView>
)

export const AppNavigator = () => {
  useBackButtonHandler(routeName => routeName === "Threads")
  // MM-H3 (#8489): a tapped "task finished / needs you" push (MM-G2, #8486)
  // opens the exact thread it's about via the server's own `data.deepLink`,
  // reusing the SAME `khala://thread/:threadId` scheme this navigator's
  // `linking` config already parses.
  usePushNotificationDeepLink()

  return (
    <NavigationContainer linking={linking} ref={navigationRef} theme={navigationTheme}>
      <Drawer.Navigator
        drawerContent={AppDrawerContent}
        screenOptions={{
          drawerActiveTintColor: khalaMobileTheme.text,
          drawerInactiveTintColor: khalaMobileTheme.textMuted,
          drawerStyle: { backgroundColor: khalaMobileTheme.surface },
          headerShown: false,
          sceneStyle: { backgroundColor: khalaMobileTheme.background },
        }}
      >
        <Drawer.Screen name="Main" component={MainStackNavigator} options={{ title: tx("nav.threads") }} />
        <Drawer.Screen name="FleetPeek" component={FleetPeekScreen} options={{ title: "Fleet" }} />
        <Drawer.Screen name="Settings" component={SettingsScreen} options={{ title: tx("nav.settings") }} />
        {/* EN-3 (#8568): renderer adapter #1 proof — a screen whose UI is
         * authored with the Effect Native component set and rendered by
         * @effect-native/render-rn. Registered so it is navigable/deep-linkable
         * (khala://about-effect-native), but hidden from the drawer flyout so
         * shipping UX is unchanged (new-screen-only, per the issue). */}
        <Drawer.Screen
          name="AboutEffectNative"
          component={AboutEffectNativeScreen}
          options={{ drawerItemStyle: { display: "none" }, title: "Effect Native" }}
        />
      </Drawer.Navigator>
    </NavigationContainer>
  )
}

const $drawerContent: ViewStyle = { flex: 1, backgroundColor: khalaMobileTheme.surface }
const $drawerScroll: ViewStyle = { flexGrow: 1 }
const $drawerFooter: ViewStyle = { backgroundColor: khalaMobileTheme.surface }
