import React from 'react'
import { ScrollView, Text, View, Pressable } from 'react-native'
import { router } from 'expo-router'
import { typedRouter } from '@/lib/typed-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { useIsDevEnv } from '@/lib/env'

export default function DeveloperMenu() {
  useHeaderTitle('Developer')
  const isDev = useIsDevEnv()
  if (!isDev) {
    return (
      <View style={{ flex: 1, backgroundColor: Colors.background, alignItems: 'center', justifyContent: 'center' }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 14 }}>Developer tools are only available in dev mode.</Text>
      </View>
    )
  }
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16 }}>
      <View style={{ gap: 12 }}>
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Diagnostics</Text>
        <Pressable
          onPress={() => { try { typedRouter.push('/developer/logs') } catch {} }}
          accessibilityRole="button"
          testID="developer-open-logs"
          style={{ paddingVertical: 10 }}
        >
          <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Logs</Text>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>View live app and sidecar logs with ANSI colors</Text>
        </Pressable>

        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Thread Tools</Text>
        <Pressable
          onPress={() => { try { typedRouter.push('/developer/recent-thread') } catch {} }}
          accessibilityRole="button"
          style={{ paddingVertical: 10 }}
        >
          <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 16 }}>Most Recent Codex Thread</Text>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>Render the latest Codex chat using ACP components</Text>
        </Pressable>
      </View>
    </ScrollView>
  )
}
