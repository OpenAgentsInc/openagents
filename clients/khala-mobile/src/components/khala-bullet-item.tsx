import { View, type StyleProp, type TextStyle, type ViewStyle } from "react-native"

import { KhalaText } from "./khala-text"

export type KhalaBulletItemProps = Readonly<{
  className?: string
  style?: StyleProp<ViewStyle>
  text: string
  textStyle?: StyleProp<TextStyle>
}>

export const KhalaBulletItem = ({ className = "", style, text, textStyle }: KhalaBulletItemProps) => (
  <View
    className={`flex-row border-b border-borderMuted py-4 ${className}`.trim()}
    style={style}
  >
    <View className="mr-4 mt-2 h-2 w-2 rounded-sm bg-accent" />
    <KhalaText className="min-w-0 flex-1 text-textSoft" style={textStyle} variant="body">
      {text}
    </KhalaText>
  </View>
)
