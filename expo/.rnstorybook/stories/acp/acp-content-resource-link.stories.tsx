import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ContentResourceLink } from '@/components/acp/ContentResourceLink'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/Content/ResourceLink' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

export const Basic: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ContentResourceLink name={'ADR 0002'} uri={'docs/adr/0002-rust-to-ts-types.md'} mimeType={'text/markdown'} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: ContentResourceLink</Text>
        <Text style={{ color: Colors.foreground }}>Shows a named link to a resource (local path or URL).</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'name: string — display name'}</Text>
        <Text style={{ color: Colors.foreground }}>{'uri: string — URI or repo path'}</Text>
        <Text style={{ color: Colors.foreground }}>{'mimeType?: string — optional mime type hint'}</Text>
      </View>
    </View>
  ),
}

