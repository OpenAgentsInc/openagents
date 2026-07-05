import type { ReactNode } from "react"
import { ActivityIndicator, Pressable, View, type PressableProps } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { KhalaText } from "./khala-text"

export type KhalaButtonVariant = "danger" | "ghost" | "primary" | "secondary"

export type KhalaButtonProps = Omit<PressableProps, "children"> &
  Readonly<{
    children?: ReactNode
    className?: string
    disabled?: boolean
    leftAccessory?: ReactNode
    loading?: boolean
    rightAccessory?: ReactNode
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

export const KhalaButton = ({
  accessibilityState,
  children,
  className = "",
  disabled = false,
  leftAccessory,
  loading = false,
  rightAccessory,
  text,
  textClassName = "",
  variant = "secondary",
  ...props
}: KhalaButtonProps) => {
  const unavailable = disabled || loading

  return (
    <Pressable
      {...props}
      accessibilityRole="button"
      accessibilityState={{ ...accessibilityState, busy: loading, disabled: unavailable }}
      className={`${baseClassName} ${variantClassName[variant]} ${
        unavailable ? "opacity-50" : ""
      } ${className}`.trim()}
      disabled={unavailable}
    >
      {leftAccessory === undefined ? null : <View>{leftAccessory}</View>}
      {loading ? <ActivityIndicator color={loadingColor[variant]} /> : null}
      {text === undefined && children === undefined ? null : (
        <KhalaText
          className={`text-center font-semibold ${variantTextClassName[variant]} ${textClassName}`.trim()}
          variant="body"
        >
          {text ?? children}
        </KhalaText>
      )}
      {rightAccessory === undefined ? null : <View>{rightAccessory}</View>}
    </Pressable>
  )
}
