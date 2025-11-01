import React from 'react'
import { Image, type ImageProps, type ImageStyle, type StyleProp } from 'react-native'

export type AutoImageProps = Omit<ImageProps, 'style'> & {
  style?: StyleProp<ImageStyle>
}

// Simple AutoImage: respects given style; defaults to contain and maxWidth: '100%'.
export function AutoImage({ style, resizeMode = 'contain', ...rest }: AutoImageProps) {
  return <Image {...rest} resizeMode={resizeMode} style={[{ alignSelf: 'stretch' }, style]} />
}

export default AutoImage

