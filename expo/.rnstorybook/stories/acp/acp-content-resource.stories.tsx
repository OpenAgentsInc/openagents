import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ContentResource } from '@/components/acp/ContentResource'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/Content/Resource' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const TextResource: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ContentResource resource={{ uri: 'README.md', mimeType: 'text/markdown', text: '# Hello' }} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: ContentResource</Text>
        <Text style={{ color: Colors.foreground }}>Renders an inlined resource. Text is shown via CodeBlock; binary shows a placeholder.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'resource: { uri: string; mimeType?: string | null; text?: string; blob?: string }'}</Text>
      </View>
    </View>
  ),
}

