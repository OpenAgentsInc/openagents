import { DrawerActions } from "@react-navigation/native"
import { useNavigation } from "@react-navigation/native"
import { useDrawerStatus } from "@react-navigation/drawer"
import { Pressable, Text, View } from "react-native"

import { ConnectivityDot } from "../status/connectivity-dot"
import { DrawerIconButton } from "./drawer-icon-button"

type AppHeaderProps = Readonly<{
  title: string
  showBack?: boolean
  showMenu?: boolean
}>

/** Hamburger↔X drawer toggle. Split out so `useDrawerStatus` (which throws
 * outside a drawer navigator's screen tree) is only ever called while this
 * subcomponent is mounted, i.e. only when `showMenu` is true. */
const DrawerMenuButton = () => {
  const navigation = useNavigation()
  const drawerStatus = useDrawerStatus()
  const open = drawerStatus === "open"

  return (
    <DrawerIconButton
      accessibilityLabel={open ? "Close menu" : "Open menu"}
      accessibilityRole="button"
      hitSlop={12}
      onPress={() =>
        navigation.dispatch(open ? DrawerActions.closeDrawer() : DrawerActions.openDrawer())
      }
      open={open}
    />
  )
}

/** Fully custom header bar (native headers wrap headerRight/headerLeft in
 * their own circular button chrome on newer iOS, which looked wrong here) -
 * this gives exact control over layout, so the connectivity dot is just a
 * small, properly centered circle, not a native button. */
export const AppHeader = ({ showBack = false, showMenu = false, title }: AppHeaderProps) => {
  const navigation = useNavigation()

  return (
    <View className="flex-row items-center border-b border-borderMuted px-4 py-3">
      <View className="w-14">
        {showBack ? (
          <Pressable
            accessibilityRole="button"
            hitSlop={12}
            onPress={() => {
              if (navigation.canGoBack()) navigation.goBack()
            }}
          >
            <Text className="font-sans text-2xl text-text">‹</Text>
          </Pressable>
        ) : showMenu ? (
          <DrawerMenuButton />
        ) : null}
      </View>
      <Text
        className="flex-1 text-center font-sans text-lg font-semibold text-text"
        numberOfLines={1}
      >
        {title}
      </Text>
      <View className="w-14 items-end">
        <ConnectivityDot />
      </View>
    </View>
  )
}
