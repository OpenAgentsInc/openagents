import type { ReactNode } from "react"
import { View, type AccessibilityState } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { ActivityIndicator } from "./activity-indicator"
import { KhalaText } from "./khala-text"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaButtonVariant = "danger" | "ghost" | "primary" | "secondary"

export type KhalaButtonProps = Readonly<{
  accessibilityState?: AccessibilityState
  children?: ReactNode
  className?: string
  disabled?: boolean
  leftAccessory?: ReactNode
  loading?: boolean
  onPress?: () => void
  rightAccessory?: ReactNode
  testID?: string
  text?: string
  textClassName?: string
  variant?: KhalaButtonVariant
}>

const baseClassName = "min-h-11 flex-row items-center justify-center gap-2 rounded-lg px-4 py-3"

const variantClassName: Record<KhalaButtonVariant, string> = {
  danger: "border border-danger/70 bg-danger/10",
  ghost: "bg-transparent",
  primary: "bg-accent",
  secondary: "border border-border bg-surfaceRaised",
}

const variantTextClassName: Record<KhalaButtonVariant, string> = {
  danger: "text-danger",
  ghost: "text-textFaint",
  primary: "text-bg",
  secondary: "text-text",
}

const loadingColor: Record<KhalaButtonVariant, string> = {
  danger: khalaMobileTheme.danger,
  ghost: khalaMobileTheme.textMuted,
  primary: khalaMobileTheme.background,
  secondary: khalaMobileTheme.accent,
}

const variantHighlightColor: Record<KhalaButtonVariant, string> = {
  danger: "rgba(228, 90, 90, 0.14)",
  ghost: "rgba(255, 255, 255, 0.08)",
  primary: "rgba(2, 6, 13, 0.14)",
  secondary: "rgba(79, 208, 255, 0.1)",
}

/** Uses the ported Arcade `TouchableFeedback` for press state (a UI-thread
 * color cross-fade, not an instant NativeWind class swap) and the ported
 * Skia `ActivityIndicator` for its loading state, rather than the plain
 * `Pressable`/`react-native` `ActivityIndicator` this used before — see
 * docs/khala-code/2026-07-06-khala-mobile-arcade-ignite-fidelity-audit.md §4. */
export const KhalaButton = ({
  accessibilityState,
  children,
  className = "",
  disabled = false,
  leftAccessory,
  loading = false,
  onPress,
  rightAccessory,
  testID,
  text,
  textClassName = "",
  variant = "secondary",
}: KhalaButtonProps) => {
  const unavailable = disabled || loading

  return (
    <TouchableFeedback
      accessibilityRole="button"
      accessibilityState={{ ...accessibilityState, busy: loading, disabled: unavailable }}
      className={`${baseClassName} ${variantClassName[variant]} ${
        unavailable ? "opacity-50" : ""
      } ${className}`.trim()}
      disabled={unavailable}
      highlightColor={variantHighlightColor[variant]}
      onPress={onPress}
      testID={testID}
    >
      {leftAccessory === undefined ? null : <View>{leftAccessory}</View>}
      {loading ? <ActivityIndicator color={loadingColor[variant]} size={52} strokeWidth={5} type="large" /> : null}
      {text === undefined && children === undefined ? null : (
        <KhalaText
          className={`text-center font-semibold ${variantTextClassName[variant]} ${textClassName}`.trim()}
          variant="body"
        >
          {text ?? children}
        </KhalaText>
      )}
      {rightAccessory === undefined ? null : <View>{rightAccessory}</View>}
    </TouchableFeedback>
  )
}
