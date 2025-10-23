import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { CodeBlock } from '@/components/code-block'

export function CommandExecutionCard({ command, status, exitCode, sample, outputLen, showExitCode = false, showOutputLen = false }: { command: string; status?: string; exitCode?: number | null; sample?: string; outputLen?: number; showExitCode?: boolean; showOutputLen?: boolean }) {
  const badgeBg = status === 'failed' || (typeof exitCode === 'number' && exitCode !== 0) ? Colors.danger : status === 'completed' ? Colors.success : Colors.gray
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12, gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ backgroundColor: badgeBg, borderRadius: 0, paddingHorizontal: 8, paddingVertical: 2 }}>
          <Text style={{ color: Colors.primaryForeground, fontFamily: Typography.bold, fontSize: 11 }}>{(status ?? 'cmd').toUpperCase()}</Text>
        </View>
        <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13 }}>{command}</Text>
      </View>
      {showExitCode && typeof exitCode === 'number' ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>exit_code: {exitCode}</Text>
      ) : null}
      {showOutputLen && typeof outputLen === 'number' && outputLen > 0 ? (
        <View style={{ gap: 4 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>output ~{outputLen}B</Text>
          {sample ? (
            <CodeBlock code={sample} language="bash" />
          ) : null}
        </View>
      ) : sample ? (
        // Show sample snippet but hide size meta in the main feed
        <CodeBlock code={sample} language="bash" />
      ) : null}
    </View>
  )
}
