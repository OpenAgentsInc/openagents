import React from 'react'
import { ScrollView, View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { ThreadListItemBase } from '@/components/drawer/ThreadListItem'

export default function DrawerLibraryScreen() {
  useHeaderTitle('Drawer Components')
  const now = Date.now()
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12, paddingBottom: 40 }}>
      <Text style={{ color: Colors.secondary }}>ThreadListItem</Text>
      <View>
        <ThreadListItemBase title="New Thread" timestamp={now} count={12} onPress={() => {}} />
        <ThreadListItemBase title="Bug bash" timestamp={now - 1000 * 60 * 60} count={3} onPress={() => {}} />
      </View>
    </ScrollView>
  )
}

