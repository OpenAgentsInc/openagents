import { Keyboard, Pressable } from "react-native"
import { useKeyboard } from "@/hooks/useKeyboard"

export const KeyboardDismisser = () => {
  const { isOpened } = useKeyboard()
  return (
    <Pressable
      style={{
        position: "absolute",
        top: 0,
        left: 0,
        right: 0,
        bottom: 0,
        zIndex: isOpened ? 2 : -1,
      }}
      onPress={Keyboard.dismiss}
    />
  )
}
