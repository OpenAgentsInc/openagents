import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
import { SessionUpdateCurrentModeUpdate } from '@/components/acp'
import { Colors } from '@/constants/theme'

const meta = {
  title: 'ACP/CurrentMode',
} satisfies Meta

export default meta

type Story = StoryObj<typeof meta>

export const Review: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <SessionUpdateCurrentModeUpdate currentModeId={'review'} />
    </View>
  ),
}

