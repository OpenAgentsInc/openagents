import { View } from "react-native"

import { KhalaText } from "./khala-text"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaThreadHeaderProps = Readonly<{
  onBack: () => void
  /** One-tap "start a fresh thread" action. Always rendered so it stays a
   * reachable escape hatch even while a turn is in flight (owner report,
   * 2026-07-06: "no way to start a new thread ... cant do anything"). When
   * `undefined` (the sync runtime hasn't opened yet) the button is shown
   * disabled rather than hidden, so the affordance never disappears. */
  onNewThread?: () => void
  subtitle: string
  title: string
}>

export const KhalaThreadHeader = ({
  onBack,
  onNewThread,
  subtitle,
  title,
}: KhalaThreadHeaderProps) => (
  <View className="px-4 pb-3 pt-2">
    <View className="flex-row items-center gap-3">
      <TouchableFeedback
        accessibilityLabel="Back"
        accessibilityRole="button"
        className="h-14 w-14 items-center justify-center rounded-full border border-borderMuted bg-surfaceRaised"
        hitSlop={10}
        onPress={onBack}
      >
        <KhalaText className="text-[36px] leading-10 text-text" variant="body">
          ‹
        </KhalaText>
      </TouchableFeedback>

      <View className="min-w-0 flex-1">
        <KhalaText className="text-[19px] font-semibold leading-6" numberOfLines={1} variant="body">
          {title}
        </KhalaText>
        <KhalaText className="text-[15px] leading-5" numberOfLines={1} variant="muted">
          {subtitle}
        </KhalaText>
      </View>

      <TouchableFeedback
        accessibilityLabel="New thread"
        accessibilityRole="button"
        className={`h-14 flex-row items-center gap-2 rounded-full border px-4 ${
          onNewThread === undefined
            ? "border-borderMuted bg-surface"
            : "border-accent/60 bg-surfaceRaised"
        }`}
        disabled={onNewThread === undefined}
        hitSlop={10}
        onPress={onNewThread}
      >
        <KhalaText
          className={`text-[22px] leading-6 ${onNewThread === undefined ? "text-textFaint" : "text-accent"}`}
          variant="body"
        >
          ✎
        </KhalaText>
        <KhalaText
          className={`text-[12px] font-semibold uppercase tracking-wide ${
            onNewThread === undefined ? "text-textFaint" : "text-accent"
          }`}
          variant="faint"
        >
          New
        </KhalaText>
      </TouchableFeedback>
    </View>
  </View>
)
