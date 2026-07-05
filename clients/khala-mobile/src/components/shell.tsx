import type { ReactNode } from "react"
import { ScrollView, Text, View } from "react-native"
import { SafeAreaView } from "react-native-safe-area-context"

import { TouchableFeedback } from "./touchable-feedback"

type ScreenShellProps = Readonly<{
  title: string
  subtitle?: string
  children: ReactNode
}>

export const ScreenShell = ({ children, subtitle, title }: ScreenShellProps) => (
  <SafeAreaView className="flex-1 bg-bg">
    <ScrollView
      className="flex-1"
      contentContainerClassName="gap-5 px-4 pb-10 pt-4"
    >
      <View className="gap-2 border-b border-border pb-4">
        <Text className="font-sans text-3xl font-semibold text-text">
          {title}
        </Text>
        {subtitle === undefined ? null : (
          <Text className="font-sans text-base text-textMuted">{subtitle}</Text>
        )}
      </View>
      {children}
    </ScrollView>
  </SafeAreaView>
)

type NavigationTileProps = Readonly<{
  onPress: () => void
  title: string
  detail: string
}>

export const NavigationTile = ({ detail, onPress, title }: NavigationTileProps) => (
  <TouchableFeedback
    accessibilityRole="button"
    className="rounded-xl border border-border bg-surfaceRaised p-4"
    onPress={onPress}
  >
    <Text className="font-sans text-lg font-semibold text-text">{title}</Text>
    <Text className="mt-2 font-sans text-base text-textMuted">{detail}</Text>
  </TouchableFeedback>
)

type StatLineProps = Readonly<{
  label: string
  value: string
}>

export const StatLine = ({ label, value }: StatLineProps) => (
  <View className="flex-row items-center justify-between gap-3 border-b border-borderMuted py-3">
    <Text className="shrink font-sans text-base text-textMuted">{label}</Text>
    <Text className="font-mono text-base tabular-nums text-text">{value}</Text>
  </View>
)

type PillProps = Readonly<{
  tone?: "accent" | "success" | "warning" | "danger"
  children: ReactNode
}>

export const Pill = ({ children, tone = "accent" }: PillProps) => {
  const toneClass = {
    accent: "border-accent/60 bg-accent/10 text-accentText",
    danger: "border-danger/60 bg-danger/10 text-danger",
    success: "border-success/60 bg-success/10 text-success",
    warning: "border-warning/60 bg-warning/10 text-warning"
  }[tone]

  return (
    <Text
      className={`self-start rounded-lg border px-2 py-1 font-mono text-sm ${toneClass}`}
    >
      {children}
    </Text>
  )
}
