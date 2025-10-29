import React from 'react'
import { ScrollView, Text, View } from 'react-native'
// Skills disabled temporarily
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle, useHeaderSubtitle } from '@/lib/header-store'

export default function SkillsIndex() {
  useHeaderTitle('Skills')
  useHeaderSubtitle('')
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 16 }}>Skills are temporarily unavailable.</Text>
    </View>
  )
}
