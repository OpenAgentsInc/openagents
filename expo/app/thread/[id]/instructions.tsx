import React from 'react'
import { useLocalSearchParams, Stack } from 'expo-router'
import { ScrollView, Text, View } from 'react-native'
import { useThreads } from '@/lib/threads-store'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export default function ThreadInstructions() {
  const { id, path } = useLocalSearchParams<{ id: string; path?: string }>()
  // In Convex-only flow, instructions are not loaded via bridge history.
  // Show a placeholder until we add a dedicated Convex field.
  const body = '(no instructions)'
  return (
    <View style={{ flex: 1, backgroundColor: Colors.background }}>
      <Stack.Screen options={{ title: 'Instructions', headerBackTitle: '' }} />
      <ScrollView contentContainerStyle={{ padding: 16 }}>
        <Text selectable style={{ color: Colors.foreground, fontFamily: Typography.primary, lineHeight: 18 }}>{body}</Text>
      </ScrollView>
    </View>
  )
}
