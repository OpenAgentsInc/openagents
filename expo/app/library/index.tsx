import React from 'react'
import { ScrollView, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'

export default function ComponentLibraryScreen() {
  useHeaderTitle('Component Library')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 14 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
        Component samples moved; this screen will be repopulated.
      </Text>
    </ScrollView>
  )
}
