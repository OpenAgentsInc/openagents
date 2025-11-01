import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { Composer } from '@/components/Composer'
import { Colors } from '@/constants/theme'

const meta = { title: 'App/Composer' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, justifyContent: 'flex-end' }}>
      <Composer onSend={() => {}} connected={true} placeholder="Ask Codex" />
      <Text style={{ color: Colors.secondary, marginTop: 12 }}>Component: Composer — send box with queue/interrupt logic.</Text>
    </View>
  ),
}

export const Running: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16, justifyContent: 'flex-end' }}>
      <Composer onSend={() => {}} onQueue={() => {}} onInterrupt={() => {}} connected={true} isRunning={true} />
    </View>
  ),
}
