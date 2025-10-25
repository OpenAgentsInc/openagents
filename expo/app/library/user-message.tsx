import React from 'react'
import { ScrollView, View } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { UserMessageRow } from '@/components/jsonl/UserMessageRow'

export default function UserMessageLibraryScreen() {
  useHeaderTitle('UserMessageRow')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <View>
        <UserMessageRow text={'Find all TODOs in the repo and suggest a plan to address them. Then propose a migration strategy.'} />
      </View>
    </ScrollView>
  )
}

