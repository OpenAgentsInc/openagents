import * as Haptics from "expo-haptics"
import type { ReactNode } from "react"
import { useCallback, useContext } from "react"
import type { StyleProp, ViewStyle } from "react-native"
import { Gesture, GestureDetector } from "react-native-gesture-handler"
import Animated, { measure, runOnJS, useAnimatedRef, type MeasuredDimensions } from "react-native-reanimated"

import { BlurredPopupContext, type PopupOptionType } from "./blurred-context"

export type TouchablePopupHandlerProps = Readonly<{
  children?: ReactNode
  /** Alternative content to show "popped forward" in the popup instead of
   * `children` — defaults to `children` itself. */
  highlightedChildren?: ReactNode
  style?: StyleProp<ViewStyle>
  /** Optional plain single-tap handler for the wrapped node, independent of
   * the long-press popup. */
  onPress?: () => void
  options: ReadonlyArray<PopupOptionType>
}>

/** Ported from Arcade's `TouchablePopupHandler`
 * (`app/components/BlurredPopup/TouchablePopupHandler.tsx`, see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.5 and issue #8395).
 * Wraps a node with a `Gesture.LongPress().minDuration(500)` that measures
 * the node on the UI thread (`useAnimatedRef` + `measure`, both still
 * synchronous/UI-thread APIs on the pinned Reanimated version) and hands the
 * measured layout to `BlurredPopupContext.showPopup`, firing a light haptic
 * (`expo-haptics`'s `impactAsync(ImpactFeedbackStyle.Light)`) at the same
 * moment — the Expo-native equivalent of Arcade's `react-native-haptic-feedback`
 * `impactLight`, since neither haptics package was a dependency yet and this
 * app already ships the Expo SDK haptics module needs no extra native
 * dependency beyond the one Expo-first package.
 *
 * `Gesture.LongPress`/`Gesture.Exclusive` are marked `@deprecated` in favor of
 * this version's newer `useLongPressGesture`/`useExclusiveGestures` hooks,
 * but still function identically at runtime (no on-device warning, unlike
 * the `SkPath.addArc` case #8402 hit) — kept for consistency with the
 * builder-style `Gesture.Pan()`/`Gesture.Tap()` API `../swipeable-item`
 * already established in this codebase, rather than mixing two gesture
 * definition styles in the same app. */
export const TouchablePopupHandler = ({ children, highlightedChildren, onPress, options, style }: TouchablePopupHandlerProps) => {
  const viewRef = useAnimatedRef<Animated.View>()
  const { showPopup } = useContext(BlurredPopupContext)

  const wrappedJsShowPopup = useCallback(
    (layout: MeasuredDimensions) => {
      showPopup({ layout, node: highlightedChildren ?? children, options })
    },
    [children, highlightedChildren, options, showPopup]
  )

  const runLightFeedback = useCallback(() => {
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {
      // Haptics can fail on devices/simulators with no haptic engine — the
      // popup itself should still open regardless.
    })
  }, [])

  const longPressGesture = Gesture.LongPress()
    .minDuration(500)
    .onStart(() => {
      const dimensions = measure(viewRef)
      if (dimensions === null) return
      runOnJS(wrappedJsShowPopup)(dimensions)
      runOnJS(runLightFeedback)()
    })

  const gesture =
    onPress === undefined
      ? longPressGesture
      : Gesture.Exclusive(
          longPressGesture,
          Gesture.Tap().onTouchesUp(() => {
            runOnJS(onPress)()
          })
        )

  return (
    <GestureDetector gesture={gesture}>
      <Animated.View collapsable={false} ref={viewRef} style={style}>
        {children}
      </Animated.View>
    </GestureDetector>
  )
}
