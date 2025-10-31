import React from 'react'
import { Image, View, Text, type ImageSourcePropType } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function ContentImage({ data, mimeType, uri, maxHeight = 240 }: { data: string; mimeType: string; uri?: string | null; maxHeight?: number }) {
  const src: ImageSourcePropType = uri && uri.length > 0 ? { uri } : { uri: `data:${mimeType};base64,${data}` }
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, padding: 8 }}>
      <Image source={src} resizeMode="contain" style={{ width: '100%', height: maxHeight }} />
      {uri ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginTop: 4 }}>{uri}</Text>
      ) : null}
    </View>
  )
}
