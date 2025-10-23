import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'

export type FileChange = { path: string; kind: 'add'|'delete'|'update'|string }

export function FileChangeCard({ changes, status }: { changes: ReadonlyArray<FileChange>; status?: string }) {
  const counts = changes.reduce((acc, c) => { acc[c.kind] = (acc[c.kind] ?? 0) + 1; return acc }, {} as Record<string, number>)
  const summary = [
    counts['add'] ? `+${counts['add']}` : null,
    counts['update'] ? `~${counts['update']}` : null,
    counts['delete'] ? `-${counts['delete']}` : null,
  ].filter(Boolean).join(' ')
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12, gap: 8 }}>
      <Text style={{ color: Colors.textSecondary, fontFamily: Typography.bold }}>file_change {status ? `(${status})` : ''}</Text>
      {!!summary && (
        <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>Summary: {summary}</Text>
      )}
      <View style={{ gap: 4 }}>
        {changes.slice(0, 8).map((c, i) => (
          <Text key={i} selectable style={{ color: Colors.textPrimary, fontFamily: Typography.primary }}>
            {c.kind === 'add' ? '+' : c.kind === 'delete' ? '-' : '~'} {c.path}
          </Text>
        ))}
        {changes.length > 8 && (
          <Text style={{ color: Colors.textSecondary, fontFamily: Typography.primary }}>… {changes.length - 8} more</Text>
        )}
      </View>
    </View>
  )}
