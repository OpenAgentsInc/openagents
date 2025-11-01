import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ContentAudio } from '@/components/acp/ContentAudio'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/Content/Audio' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ContentAudio mimeType={'audio/mpeg'} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: ContentAudio</Text>
        <Text style={{ color: Colors.foreground }}>Placeholder renderer for audio content.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'mimeType: string — e.g., audio/mpeg'}</Text>
      </View>
    </View>
  ),
}

