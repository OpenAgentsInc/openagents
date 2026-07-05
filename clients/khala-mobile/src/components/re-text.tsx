import type { StyleProp, TextInputProps, TextStyle } from "react-native"
import { TextInput } from "react-native"
import type { SharedValue } from "react-native-reanimated"
import Animated, { useAnimatedProps } from "react-native-reanimated"

// `text` is whitelisted once at module load so `useAnimatedProps` can push
// updates straight into the native TextInput prop without ever touching
// React's render cycle (no setState, no re-render) — see useAnimatedProps
// below, which reads `text.value` on the UI thread.
Animated.addWhitelistedNativeProps({ text: true })

export type ReTextProps = Readonly<
  Omit<TextInputProps, "value" | "style"> & {
    text: SharedValue<string>
    style?: StyleProp<TextStyle>
  }
>

const AnimatedTextInput = Animated.createAnimatedComponent(TextInput)

/**
 * Renders a Reanimated `SharedValue<string>` as text with zero JS-thread
 * re-renders on update — every change to `text.value` is pushed directly
 * into the native `TextInput` `text` prop on the UI thread.
 *
 * Deeply inspired by wcandillon's Redash `ReText` component:
 * https://github.com/wcandillon/react-native-redash
 */
export const ReText = (props: ReTextProps) => {
  const { style, text, ...rest } = props
  const animatedProps = useAnimatedProps(() => {
    return {
      text: text.value
      // `text` is not part of TextInput's public prop types — it's the
      // native prop whitelisted above for direct UI-thread updates.
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
    } as any
  })

  return (
    <AnimatedTextInput
      underlineColorAndroid="transparent"
      editable={false}
      value={text.value}
      style={style}
      {...rest}
      animatedProps={animatedProps}
    />
  )
}
