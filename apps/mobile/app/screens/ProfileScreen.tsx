import { FC } from "react"
import { TextStyle, View, ViewStyle } from "react-native"

import { Button } from "@/components/Button"
import { Screen } from "@/components/Screen"
import { Text } from "@/components/Text"
import { useAuth } from "@/context/AuthContext"
import { DemoTabScreenProps } from "@/navigators/navigationTypes"
import { useAppTheme } from "@/theme/context"
import type { ThemedStyle } from "@/theme/types"

export const ProfileScreen: FC<DemoTabScreenProps<"Profile">> = function ProfileScreen() {
  const { authUser, authEmail, authUserId, logout } = useAuth()
  const { themed } = useAppTheme()

  const displayName = authUser
    ? [authUser.firstName, authUser.lastName].filter(Boolean).join(" ") || null
    : null
  const email = authUser?.email ?? authEmail ?? null

  return (
    <Screen preset="scroll" safeAreaEdges={["top"]} contentContainerStyle={themed($container)}>
      <Text tx="profileScreen:title" preset="heading" style={themed($title)} />
      <Text tx="profileScreen:subtitle" size="md" style={themed($subtitle)} />

      <View style={themed($card)}>
        {displayName ? (
          <>
            <Text tx="profileScreen:nameLabel" size="xs" style={themed($labelFirst)} />
            <Text text={displayName} preset="bold" style={themed($value)} />
          </>
        ) : null}
        <Text
          tx="profileScreen:emailLabel"
          size="xs"
          style={themed(displayName ? $label : $labelFirst)}
        />
        <Text text={email ?? "—"} preset="bold" style={themed($value)} />
        {authUserId ? (
          <>
            <Text tx="profileScreen:userIdLabel" size="xs" style={themed($label)} />
            <Text text={authUserId} size="sm" style={themed($valueMuted)} numberOfLines={1} />
          </>
        ) : null}
      </View>

      <Button tx="common:logOut" preset="default" style={themed($logoutButton)} onPress={logout} />
    </Screen>
  )
}

const $container: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.lg,
  paddingTop: spacing.lg,
  paddingBottom: spacing.xxl,
})

const $title: ThemedStyle<TextStyle> = ({ spacing }) => ({
  marginBottom: spacing.xs,
})

const $subtitle: ThemedStyle<TextStyle> = ({ spacing }) => ({
  marginBottom: spacing.xl,
})

const $card: ThemedStyle<ViewStyle> = ({ colors, spacing }) => ({
  backgroundColor: colors.palette.neutral100,
  borderRadius: 12,
  padding: spacing.lg,
  marginBottom: spacing.xl,
})

const $label: ThemedStyle<TextStyle> = ({ colors, spacing }) => ({
  color: colors.textDim,
  marginTop: spacing.sm,
  marginBottom: spacing.xs,
})
const $labelFirst: ThemedStyle<TextStyle> = ({ colors, spacing }) => ({
  color: colors.textDim,
  marginBottom: spacing.xs,
})
const $value: ThemedStyle<TextStyle> = ({ spacing }) => ({
  marginBottom: spacing.xs,
})
const $valueMuted: ThemedStyle<TextStyle> = ({ colors }) => ({
  color: colors.textDim,
})

const $logoutButton: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginTop: spacing.md,
})
