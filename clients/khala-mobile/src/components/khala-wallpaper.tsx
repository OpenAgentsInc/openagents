import type { ReactNode } from "react"
import { ImageBackground, StyleSheet, View, type ImageSourcePropType, type StyleProp, type ViewStyle } from "react-native"

export type KhalaWallpaperProps = Readonly<{
  children?: ReactNode
  imageSource?: ImageSourcePropType
  overlayOpacity?: number
  style?: StyleProp<ViewStyle>
}>

export const KhalaWallpaper = ({
  children,
  imageSource = require("../../assets/images/home-hero.jpg"),
  overlayOpacity = 0.78,
  style,
}: KhalaWallpaperProps) => (
  <ImageBackground resizeMode="cover" source={imageSource} style={[styles.wallpaper, style]}>
    <View pointerEvents="none" style={[StyleSheet.absoluteFill, { backgroundColor: `rgba(2, 10, 22, ${overlayOpacity})` }]} />
    {children}
  </ImageBackground>
)

const styles = StyleSheet.create({
  wallpaper: { overflow: "hidden" },
})
