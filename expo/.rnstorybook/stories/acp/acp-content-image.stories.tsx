import React from 'react'
import type { Meta, StoryObj } from '@storybook/react-native'
import { View, Text } from 'react-native'
import { ContentImage } from '@/components/acp/ContentImage'
import { Colors } from '@/constants/theme'

const meta = { title: 'ACP/Content/Image' } satisfies Meta
export default meta
type Story = StoryObj<typeof meta>

const onePxPngBase64 = 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMB/aqG4f8AAAAASUVORK5CYII='

export const WithDataUrl: Story = {
  render: () => (
    <View style={{ flex: 1, backgroundColor: Colors.background, padding: 16 }}>
      <ContentImage data={onePxPngBase64} mimeType={'image/png'} />
      <View style={{ marginTop: 12 }}>
        <Text style={{ color: Colors.secondary }}>Component: ContentImage</Text>
        <Text style={{ color: Colors.foreground }}>Renders an inline image from base64 or external URI.</Text>
        <Text style={{ color: Colors.secondary, marginTop: 6 }}>Props</Text>
        <Text style={{ color: Colors.foreground }}>{'data: string — base64 data (used if no uri)'} </Text>
        <Text style={{ color: Colors.foreground }}>{'mimeType: string — e.g., image/png'}</Text>
        <Text style={{ color: Colors.foreground }}>{'uri?: string — optional external image URI'}</Text>
        <Text style={{ color: Colors.foreground }}>{'maxHeight?: number — constrain height (default 240)'}</Text>
      </View>
    </View>
  ),
}

