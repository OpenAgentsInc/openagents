import type { ReactNode } from "react"
import { StyleSheet, View, type StyleProp, type ViewStyle } from "react-native"

import { KhalaWallpaper } from "./khala-wallpaper"
import { KhalaSpotlight } from "./khala-spotlight"

export type KhalaCityBackgroundProps = Readonly<{
  children?: ReactNode
  overlayOpacity?: number
  style?: StyleProp<ViewStyle>
}>

export const KhalaCityBackground = ({
  children,
  overlayOpacity = 0.72,
  style,
}: KhalaCityBackgroundProps) => (
  <View style={[styles.container, style]}>
    <KhalaWallpaper overlayOpacity={overlayOpacity} style={StyleSheet.absoluteFill} />
    <KhalaSpotlight />
    {children}
  </View>
)

const styles = StyleSheet.create({
  container: { overflow: "hidden" },
})
