import React from "react"
import { Image, ImageStyle } from "react-native"
import { images } from "../theme/images"

interface ThinkingAnimationProps {
  size?: number
  style?: ImageStyle
}

export const ThinkingAnimation = ({ size = 30, style }: ThinkingAnimationProps) => {
  return (
    <Image
      source={images.thinking}
      style={[{
        backgroundColor: "black",
        height: size,
        width: size,
        marginVertical: Math.floor(size / 3),
      }, style]}
      resizeMode="contain"
    />
  )
}