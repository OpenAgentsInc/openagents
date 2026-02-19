import { FC } from "react"
import { TextStyle, ViewStyle } from "react-native"

import { Button } from "@/components/Button"
import { Screen } from "@/components/Screen"
import { Text } from "@/components/Text"
import { useAuth } from "@/context/AuthContext"
import { DemoTabScreenProps } from "@/navigators/navigationTypes"
import { useAppTheme } from "@/theme/context"
import type { ThemedStyle } from "@/theme/types"

export const FeedScreen: FC<DemoTabScreenProps<"Feed">> = function FeedScreen({ navigation }) {
  const { isAuthenticated } = useAuth()
  const { themed } = useAppTheme()

  if (!isAuthenticated) {
    return (
      <Screen preset="fixed" contentContainerStyle={themed($container)} safeAreaEdges={["top"]}>
        <Text text="Sign in to access runtime-backed mobile surfaces." />
      </Screen>
    )
  }

  return (
    <Screen preset="fixed" contentContainerStyle={themed($container)} safeAreaEdges={["top"]}>
      <Text text="Mobile feed is being migrated to runtime parity." preset="heading" />
      <Text
        text="Use the Codex tab for runtime worker read/admin controls and live stream updates."
        size="sm"
        style={themed($muted)}
      />
      <Button
        text="Open Codex workers"
        onPress={() => navigation.navigate("Codex")}
        style={themed($button)}
      />
    </Screen>
  )
}

const $container: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.lg,
  paddingTop: spacing.lg,
  gap: spacing.md,
})

const $muted: ThemedStyle<TextStyle> = ({ colors }) => ({
  color: colors.textDim,
})

const $button: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginTop: spacing.sm,
})
