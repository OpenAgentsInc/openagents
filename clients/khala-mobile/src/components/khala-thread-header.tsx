import { View } from "react-native"

import { KhalaText } from "./khala-text"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaThreadHeaderProps = Readonly<{
  onBack: () => void
  onMore?: () => void
  onNewNote?: () => void
  subtitle: string
  title: string
}>

export const KhalaThreadHeader = ({
  onBack,
  onMore,
  onNewNote,
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

      <View className="flex-row items-center gap-2 rounded-full border border-borderMuted bg-surfaceRaised px-3 py-2">
        <TouchableFeedback
          accessibilityLabel="New note"
          accessibilityRole="button"
          className="h-9 w-9 items-center justify-center rounded-full"
          disabled={onNewNote === undefined}
          hitSlop={8}
          onPress={onNewNote}
        >
          <KhalaText className="text-[26px] leading-8 text-text" variant="body">
            ✎
          </KhalaText>
        </TouchableFeedback>
        <TouchableFeedback
          accessibilityLabel="More"
          accessibilityRole="button"
          className="h-9 w-9 items-center justify-center rounded-full"
          disabled={onMore === undefined}
          hitSlop={8}
          onPress={onMore}
        >
          <KhalaText className="text-[24px] leading-7 text-text" variant="body">
            ⋯
          </KhalaText>
        </TouchableFeedback>
      </View>
    </View>
  </View>
)
