import type { ReactNode } from "react"
import { View, type StyleProp, type ViewStyle } from "react-native"

export type KhalaFormRowPreset = "top" | "middle" | "bottom" | "soloRound" | "soloStraight" | "clear"

export type KhalaFormRowProps = Readonly<{
  children?: ReactNode
  className?: string
  preset?: KhalaFormRowPreset
  style?: StyleProp<ViewStyle>
}>

const presetClassName: Record<KhalaFormRowPreset, string> = {
  bottom: "rounded-b-lg border-t-0",
  clear: "border-transparent bg-transparent",
  middle: "border-t-0",
  soloRound: "rounded-lg",
  soloStraight: "",
  top: "rounded-t-lg",
}

export const KhalaFormRow = ({
  children,
  className = "",
  preset = "soloRound",
  style,
}: KhalaFormRowProps) => (
  <View
    className={`border border-border bg-surfaceRaised px-4 py-3 ${presetClassName[preset]} ${className}`.trim()}
    style={style}
  >
    {children}
  </View>
)
