import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ContentText } from '@/components/acp/ContentText'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/Content/Text' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ContentText text={'**Markdown** with `inline` code'} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: ContentText</Text>
        <Text style={{ color: Colors.foreground }}>Renders markdown text with our code block renderer.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'text: string — markdown body'}</Text>
      </View>
    </View>
  ),
}

