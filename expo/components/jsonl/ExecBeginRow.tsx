import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export type ExecBeginPayload = { command: string[] | ReadonlyArray<string> | string; cwd?: string }

function joinCmd(cmd: string[] | ReadonlyArray<string> | string): string {
  if (Array.isArray(cmd)) return (cmd as ReadonlyArray<string>).join(' ')
  return String(cmd)
}

export function ExecBeginRow({ payload }: { payload: ExecBeginPayload }) {
  const cmd = joinCmd(payload.command)
  const cwd = payload.cwd
  return (
    <View style={{ paddingVertical: 2 }}>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>
        [exec] <Text style={{ color: Colors.textPrimary, fontFamily: Typography.bold }}>{cmd}</Text>
      </Text>
      {cwd ? (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary, fontSize: 12 }}>
          in <Text style={{ color: Colors.textPrimary }}>{cwd}</Text>
        </Text>
      ) : null}
    </View>
  )
}
