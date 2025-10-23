import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export function CommandExecutionCard({ command, status, exitCode, sample, outputLen, showExitCode = false }: { command: string; status?: string; exitCode?: number | null; sample?: string; outputLen?: number; showExitCode?: boolean }) {
  const badgeBg = status === 'failed' || (typeof exitCode === 'number' && exitCode !== 0) ? Colors.statusError : status === 'completed' ? Colors.statusSuccess : Colors.muted
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12, gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <View style={{ backgroundColor: badgeBg, borderRadius: 0, paddingHorizontal: 8, paddingVertical: 2 }}>
          <Text style={{ color: Colors.textOnBright, fontFamily: Typography.bold, fontSize: 11 }}>{(status ?? 'cmd').toUpperCase()}</Text>
        </View>
        <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold }}>{command}</Text>
      </View>
      {showExitCode && typeof exitCode === 'number' ? (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>exit_code: {exitCode}</Text>
      ) : null}
      {typeof outputLen === 'number' && outputLen > 0 ? (
        <View style={{ gap: 4 }}>
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>output ~{outputLen}B</Text>
          {sample ? (
            <Text selectable style={{ color: Colors.textPrimary, fontFamily: Typography.primary, backgroundColor: Colors.codeBg, borderWidth: 1, borderColor: Colors.border, padding: 8 }}>{sample}</Text>
          ) : null}
        </View>
      ) : null}
    </View>
  )
}
