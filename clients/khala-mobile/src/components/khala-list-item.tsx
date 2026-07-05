import type { ReactNode } from "react"
import { View, type AccessibilityState } from "react-native"

import { TouchableFeedback } from "./touchable-feedback"
import { KhalaText } from "./khala-text"

export type KhalaListItemTone = "default" | "danger" | "success" | "warning"

export type KhalaListItemProps = Readonly<{
  accessibilityLabel?: string
  accessibilityState?: AccessibilityState
  className?: string
  detail?: ReactNode
  disabled?: boolean
  meta?: ReactNode
  onPress?: () => void
  testID?: string
  title: ReactNode
  titleNumberOfLines?: number
  tone?: KhalaListItemTone
  variant?: "plain" | "surface"
}>

const toneClassName: Record<KhalaListItemTone, string> = {
  danger: "text-danger",
  default: "text-text",
  success: "text-success",
  warning: "text-warning",
}

const renderTextish = (
  value: ReactNode,
  variant: "body" | "faint" | "muted",
  className: string,
  numberOfLines?: number,
) =>
  typeof value === "string" || typeof value === "number" ? (
    <KhalaText className={className} numberOfLines={numberOfLines} variant={variant}>
      {value}
    </KhalaText>
  ) : (
    value
  )

export const KhalaListItem = ({
  accessibilityLabel,
  accessibilityState,
  className = "",
  detail,
  disabled = false,
  meta,
  onPress,
  testID,
  title,
  titleNumberOfLines = 1,
  tone = "default",
  variant = "plain",
}: KhalaListItemProps) => {
  const content = (
    <View
      className={`gap-1.5 px-4 py-3 ${
        variant === "surface" ? "rounded-lg border border-borderMuted bg-surfaceRaised" : ""
      } ${disabled ? "opacity-50" : ""} ${className}`.trim()}
      testID={testID}
    >
      <View className="flex-row items-start justify-between gap-3">
        {renderTextish(
          title,
          "body",
          `min-w-0 shrink text-base font-semibold leading-snug ${toneClassName[tone]}`,
          titleNumberOfLines,
        )}
        {meta === undefined ? null : renderTextish(meta, "faint", "pt-1 tabular-nums", 1)}
      </View>
      {detail === undefined ? null : renderTextish(detail, "muted", "shrink", 1)}
    </View>
  )

  if (onPress === undefined) return content

  return (
    <TouchableFeedback
      accessibilityLabel={accessibilityLabel}
      accessibilityRole="button"
      accessibilityState={{ ...accessibilityState, disabled }}
      disabled={disabled}
      highlightColor="rgba(79, 208, 255, 0.1)"
      onPress={onPress}
    >
      {content}
    </TouchableFeedback>
  )
}
