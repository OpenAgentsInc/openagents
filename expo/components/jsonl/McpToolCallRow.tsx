import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function McpToolCallRow({ server, tool, status }: { server: string; tool: string; status?: string }) {
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>
        MCP <Text style={{ color: Colors.foreground, fontFamily: Typography.bold }}>{server}</Text> · {tool}
        {status ? <Text> ({status})</Text> : null}
      </Text>
    </View>
  )
}
