import type { ReactNode } from "react"
import { createContext } from "react"
import type { MeasuredDimensions } from "react-native-reanimated"

/** Ported from Arcade's `BlurredContext`
 * (`app/components/BlurredPopup/BlurredContext.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.5 and issue #8395).
 * Shared type/context surface for the long-press blurred context menu — kept
 * in its own module (no component logic) so `touchable-popup-handler.tsx` and
 * `blurred-popup-provider.tsx` can both depend on it without importing each
 * other. */

/** Which corner of the pressed node the popup menu grows from, computed from
 * available screen space at `showPopup` time. */
export type PopupAlignment = "top-left" | "top-right" | "bottom-left" | "bottom-right"

/** One row in the popup menu. */
export type PopupOptionType = Readonly<{
  label: string
  onPress?: () => void
  leading?: ReactNode
  trailing?: ReactNode
}>

export type BlurredContextType = Readonly<{
  /** Shows the popup menu: freezes a screenshot of the current screen behind
   * a two-layer blur, re-renders `node` at `layout`'s exact original screen
   * position on top (non-blurred), and lists `options` in an
   * auto-positioned menu. */
  showPopup: (params: { layout: MeasuredDimensions; node: ReactNode; options: ReadonlyArray<PopupOptionType> }) => void
}>

export const BlurredPopupContext = createContext<BlurredContextType>({
  showPopup: () => {
    // Default no-op — only meaningful once wrapped in `BlurredPopupProvider`.
  }
})
