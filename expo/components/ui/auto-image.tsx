import React from 'react'
import { Image, type ImageProps, type ImageStyle } from 'react-native'

export interface AutoImageProps extends Omit<ImageProps, 'style'> {
  style?: ImageStyle | ImageStyle[]
}

export function AutoImage({ style, resizeMode = 'contain', ...rest }: AutoImageProps) {
  return <Image {...rest} resizeMode={resizeMode} style={[{ width: '100%' }, style as any]} />
}

