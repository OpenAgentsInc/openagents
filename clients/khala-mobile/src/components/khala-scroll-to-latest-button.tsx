import { KhalaText } from "./khala-text"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaScrollToLatestButtonProps = Readonly<{
  onPress: () => void
}>

export const KhalaScrollToLatestButton = ({ onPress }: KhalaScrollToLatestButtonProps) => (
  <TouchableFeedback
    accessibilityLabel="Scroll to latest"
    accessibilityRole="button"
    className="h-12 w-12 items-center justify-center rounded-full border border-borderMuted bg-surfaceRaised"
    onPress={onPress}
  >
    <KhalaText className="text-[30px] leading-8 text-text" variant="body">
      ↓
    </KhalaText>
  </TouchableFeedback>
)
