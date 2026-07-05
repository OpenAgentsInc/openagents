import { cssInterop } from "nativewind"
import Animated from "react-native-reanimated"

/** NativeWind v4 only auto-styles core React Native host components (View,
 * Text, ...); a Reanimated `Animated.View` needs one explicit `cssInterop`
 * registration so `className` maps to `style` everywhere it's used across
 * the app (the thread-list entrance/press-feedback row, settings/fleet
 * cards, transcript-part entrance). Import this module once for its side
 * effect from the root layout, before any screen mounts, so registration
 * order never depends on which screen happens to import
 * `touchable-feedback.tsx` first. */
cssInterop(Animated.View, { className: "style" })
