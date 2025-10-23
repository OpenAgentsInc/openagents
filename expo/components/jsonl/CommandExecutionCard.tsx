import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { CodeBlock } from '@/components/code-block'
import { MaterialCommunityIcons } from '@expo/vector-icons'

export function CommandExecutionCard({
  command,
  status,
  exitCode,
  sample,
  outputLen,
  showExitCode = false,
  showOutputLen = false,
  collapsed = false,
  maxBodyHeight = 120,
}: {
  command: string
  status?: string
  exitCode?: number | null
  sample?: string
  outputLen?: number
  showExitCode?: boolean
  showOutputLen?: boolean
  collapsed?: boolean
  maxBodyHeight?: number
}) {
  const isFail = status === 'failed' || (typeof exitCode === 'number' && exitCode !== 0)
  const isDone = status === 'completed' || (typeof exitCode === 'number' && exitCode === 0)
  const iconName = isFail ? 'close' : isDone ? 'check' : 'dots-horizontal'
  const iconColor = isFail ? Colors.danger : isDone ? Colors.success : Colors.secondary
  const clamp = collapsed ? { maxHeight: maxBodyHeight, overflow: 'hidden' as const } : undefined
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12, gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <MaterialCommunityIcons name={iconName as any} size={18} color={iconColor} />
        <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13 }}>{command}</Text>
      </View>
      {showExitCode && typeof exitCode === 'number' ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>exit_code: {exitCode}</Text>
      ) : null}
      {showOutputLen && typeof outputLen === 'number' && outputLen > 0 ? (
        <View style={{ gap: 4 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>output ~{outputLen}B</Text>
          {sample ? (
            <View style={clamp}>
              <CodeBlock code={sample} language="bash" />
            </View>
          ) : null}
        </View>
      ) : sample ? (
        // Show sample snippet but hide size meta in the main feed
        <View style={clamp}>
          <CodeBlock code={sample} language="bash" />
        </View>
      ) : null}
    </View>
  )
}
