import { Pressable } from "react-native"

import { KhalaText } from "./khala-text"

export type KhalaScrollToLatestButtonProps = Readonly<{
  onPress: () => void
}>

export const KhalaScrollToLatestButton = ({ onPress }: KhalaScrollToLatestButtonProps) => (
  <Pressable
    accessibilityLabel="Scroll to latest"
    accessibilityRole="button"
    className="h-12 w-12 items-center justify-center rounded-full border border-borderMuted bg-surfaceRaised"
    onPress={onPress}
  >
    <KhalaText className="text-[30px] leading-8 text-text" variant="body">
      ↓
    </KhalaText>
  </Pressable>
)
