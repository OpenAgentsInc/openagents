import { useRouter } from "expo-router"
import { Pressable, Text, View } from "react-native"

import { ConnectivityDot } from "../status/connectivity-dot"

type AppHeaderProps = Readonly<{
  title: string
  showBack?: boolean
}>

/** Fully custom header bar (native headers wrap headerRight/headerLeft in
 * their own circular button chrome on newer iOS, which looked wrong here) —
 * this gives exact control over layout, so the connectivity dot is just a
 * small, properly centered circle, not a native button. */
export const AppHeader = ({ showBack = false, title }: AppHeaderProps) => {
  const router = useRouter()

  return (
    <View className="flex-row items-center border-b border-borderMuted px-4 py-3">
      <View className="w-8">
        {showBack ? (
          <Pressable accessibilityRole="button" hitSlop={12} onPress={() => router.back()}>
            <Text className="font-sans text-2xl text-text">‹</Text>
          </Pressable>
        ) : null}
      </View>
      <Text
        className="flex-1 text-center font-sans text-lg font-semibold text-text"
        numberOfLines={1}
      >
        {title}
      </Text>
      <View className="w-8 items-end">
        <ConnectivityDot />
      </View>
    </View>
  )
}
