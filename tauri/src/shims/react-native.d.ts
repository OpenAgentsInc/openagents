declare module 'react-native' {
  import * as React from 'react'
  export interface ViewProps { style?: React.CSSProperties | any; children?: React.ReactNode; testID?: string }
  export interface TextProps { style?: React.CSSProperties | any; children?: React.ReactNode; numberOfLines?: number; testID?: string }
  export interface PressableProps { onPress?: () => void; onLongPress?: () => void; delayLongPress?: number; accessibilityRole?: string; testID?: string; style?: React.CSSProperties | any; children?: React.ReactNode }
  export const View: React.FC<ViewProps>
  export const Text: React.FC<TextProps>
  export const Pressable: React.FC<PressableProps>
}
