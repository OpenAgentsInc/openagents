import React from 'react'
import { ScrollView, View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useHeaderTitle } from '@/lib/header-store'
import { ThreadStartedRow } from '@/components/jsonl/ThreadStartedRow'
import { TurnEventRow } from '@/components/jsonl/TurnEventRow'

export default function UnusedLibraryScreen() {
  useHeaderTitle('Unused Samples')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 40 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, fontSize: 12 }}>
        These are not shown in the main feed; kept here for reference.
      </Text>
      <View>
        <ThreadStartedRow threadId="abcd1234" />
        <TurnEventRow phase="started" />
      </View>
    </ScrollView>
  )
}

