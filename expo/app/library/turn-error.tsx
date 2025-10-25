import React from 'react'
import { ScrollView } from 'react-native'
import { Colors } from '@/constants/theme'
import { useHeaderTitle } from '@/lib/header-store'
import { TurnEventRow } from '@/components/jsonl/TurnEventRow'
import { ErrorRow } from '@/components/jsonl/ErrorRow'

export default function TurnErrorLibraryScreen() {
  useHeaderTitle('Turn & Error Rows')
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 16, paddingBottom: 40 }}>
      <TurnEventRow phase="completed" usage={{ input_tokens: 1200, cached_input_tokens: 300, output_tokens: 420 }} />
      <ErrorRow message="Something went wrong while fetching." />
    </ScrollView>
  )
}

