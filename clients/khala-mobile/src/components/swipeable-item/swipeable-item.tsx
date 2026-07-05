import { Gesture, GestureDetector } from "react-native-gesture-handler"
import type { ReactNode } from "react"
import { useMemo } from "react"
import { StyleSheet, Text, View } from "react-native"
import Animated, {
  runOnJS,
  useAnimatedReaction,
  useAnimatedStyle,
  useDerivedValue,
  useSharedValue,
  withSpring,
  withTiming
} from "react-native-reanimated"

import { khalaMobileTheme } from "../../theme/tokens"
import { SwipeQuoteDonut } from "./swipe-quote-donut"

/** Clamps a value between min and max. Must be a worklet — it runs inside
 * the Pan gesture's UI-thread handlers, and a non-worklet function there
 * crashes the gesture (ported verbatim from Arcade's own comment on this
 * exact helper). */
const clamp = (value: number, min: number, max: number): number => {
  "worklet"
  return Math.max(min, Math.min(value, max))
}

const donutSpringConfig = { mass: 0.5, overshootClamping: true } as const

export type SwipeableItemProps = Readonly<{
  children?: ReactNode
  /** Which way the row is dragged to trigger the action. `"right"` (the
   * default) reveals the action badge on the left as the row translates
   * right — the common "swipe right to reply/quote" chat convention. */
  swipeDirection?: "left" | "right"
  onSwipeComplete?: () => void
  maxScrollableAmount?: number
  /** Glyph rendered in the small badge under the donut ring once the swipe
   * has progressed past 30%. */
  actionGlyph?: string
}>

/** Ported from Arcade's `SwipeableItem` + `AnimatedDonut`
 * (`app/components/SwipeableItem/index.tsx`,
 * `app/components/AnimatedDonut/index.tsx`; see
 * `docs/design/2026-07-05-arcade-ui-harvest-audit.md` §2.3 and issue #8393).
 *
 * A `Gesture.Pan().activeOffsetX([-10, 10])` drags the row via `translateX`
 * (the 10px threshold keeps this from fighting the containing `FlatList`'s
 * vertical scroll — same rationale as Arcade's own source comment).
 * `progress` is `|translateX| / maxScrollableAmount` clamped to `[0, 1]`,
 * then **squared** before driving the Skia donut ring's stroke `end` — an
 * eased, accelerating fill rather than a linear one. Past 30% progress a
 * reveal-badge container fades in with parallax (`translateX / 10`).
 * Completing a full swipe springs the donut from scale 1 -> 2 while fading
 * out (`withSpring({ overshootClamping: true, mass: 0.5 })`), revealing the
 * action glyph underneath it; the row always snaps back via `withTiming(0)`
 * on release regardless of outcome — the caller decides what "complete"
 * means via `onSwipeComplete`.
 *
 * Per #8390/#8392/#8402's finding, the Skia version pinned in this repo
 * (2.6.2) reads Reanimated `SharedValue`/`DerivedValue` props directly, so
 * there's no `useSharedValueEffect`-style bridge like Arcade's original
 * needed. */
export const SwipeableItem = ({
  actionGlyph = "❝",
  children,
  maxScrollableAmount: scrollableAmount = 88,
  onSwipeComplete,
  swipeDirection = "right"
}: SwipeableItemProps) => {
  const translateX = useSharedValue(0)
  const contextX = useSharedValue(0)
  const hasBeenFullySwiped = useSharedValue(false)

  const maxTranslateX = swipeDirection === "right" ? scrollableAmount : 0
  const minTranslateX = swipeDirection === "left" ? -scrollableAmount : 0

  const progress = useDerivedValue(
    () => clamp(Math.abs(translateX.value) / scrollableAmount, 0, 1),
    [scrollableAmount]
  )
  // Eased-not-linear: the donut ring fills on progress**2, so it starts slow
  // and accelerates toward completion instead of tracking the finger 1:1.
  const donutProgress = useDerivedValue(() => progress.value ** 2)

  const panGesture = Gesture.Pan()
    // Needed so the Pan gesture doesn't fight the containing FlatList's
    // vertical scroll gesture.
    .activeOffsetX([-10, 10])
    .onBegin(() => {
      contextX.value = translateX.value
    })
    .onUpdate(event => {
      translateX.value = clamp(event.translationX + contextX.value, minTranslateX, maxTranslateX)
    })
    .onEnd(() => {
      if (progress.value === 1 && onSwipeComplete) {
        // `onSwipeComplete` is a plain JS function, not a worklet — must hop
        // off the UI thread to call it.
        runOnJS(onSwipeComplete)()
      }
      translateX.value = withTiming(0)
    })

  useAnimatedReaction(
    () => progress.value,
    value => {
      if (value === 0) hasBeenFullySwiped.value = false
      else if (value === 1) hasBeenFullySwiped.value = true
    }
  )

  const rowStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: translateX.value }]
  }))

  const revealStyle = useAnimatedStyle(() => ({
    opacity: withTiming(progress.value > 0.3 ? 1 : 0),
    transform: [
      {
        // Parallax: the reveal badge moves at a fraction of the row's own
        // translation, so it feels anchored while still tracking the drag.
        translateX: translateX.value / 10
      }
    ]
  }))

  const donutStyle = useAnimatedStyle(() => ({
    opacity: withSpring(hasBeenFullySwiped.value ? 0 : 1, donutSpringConfig),
    transform: [{ scale: withSpring(hasBeenFullySwiped.value ? 2 : 1, donutSpringConfig) }]
  }))

  const revealContainerStyle = useMemo(
    () => [styles.revealContainer, swipeDirection === "right" ? { left: -18 } : { right: -18 }],
    [swipeDirection]
  )

  return (
    <GestureDetector gesture={panGesture}>
      <Animated.View>
        <Animated.View style={[revealContainerStyle, revealStyle]}>
          <View style={styles.absoluteCenter}>
            <View style={styles.glyphBadge}>
              <Text style={styles.glyphText}>{actionGlyph}</Text>
            </View>
          </View>
          <Animated.View style={[StyleSheet.absoluteFill, donutStyle]}>
            <SwipeQuoteDonut progress={donutProgress} size={36} strokeWidth={3} />
          </Animated.View>
        </Animated.View>
        <Animated.View style={rowStyle}>{children}</Animated.View>
      </Animated.View>
    </GestureDetector>
  )
}

const styles = StyleSheet.create({
  absoluteCenter: { ...StyleSheet.absoluteFill, alignItems: "center", justifyContent: "center" },
  glyphBadge: {
    alignItems: "center",
    aspectRatio: 1,
    backgroundColor: khalaMobileTheme.accent,
    borderRadius: 18,
    height: 32,
    justifyContent: "center"
  },
  glyphText: { color: khalaMobileTheme.background, fontSize: 15, fontWeight: "600" },
  revealContainer: { aspectRatio: 1, height: "80%", position: "absolute", top: "10%" }
})
