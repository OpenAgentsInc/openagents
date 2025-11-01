import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View } from 'react-native'
import { ThreadListItemBase } from '@/components/drawer/ThreadListItem'
import { Colors } from '@/constants/theme'

const meta = { title: 'Drawer/ThreadListItem' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ThreadListItemBase title={'Example thread'} timestamp={Date.now() - 1000 * 60 * 3} count={12} />
      <ThreadListItemBase title={'Another thread with a very long name that will truncate'} timestamp={Date.now() - 1000 * 60 * 60 * 2} />
    </View>
  ),
}

