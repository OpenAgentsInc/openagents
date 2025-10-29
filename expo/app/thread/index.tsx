import React from 'react'
import { View, Text, Pressable } from 'react-native'
import { useHeaderTitle } from '@/lib/header-store'
import { router } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function ThreadEntry() {
  useHeaderTitle('New Thread')
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center', padding: 24 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginBottom: 10 }}>Start a new chat.</Text>
      <Pressable onPress={() => { try { router.replace('/thread/new' as any) } catch {} }} accessibilityRole="button" style={{ backgroundColor: Colors.foreground, paddingHorizontal: 14, paddingVertical: 8 }}>
        <Text style={{ color: Colors.black, fontFamily: Typography.bold }}>New Session</Text>
      </Pressable>
    </View>
  )
}
