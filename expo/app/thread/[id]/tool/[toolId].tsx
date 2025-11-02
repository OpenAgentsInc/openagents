import React from 'react'
import { ScrollView, Text, View } from 'react-native'
import { useLocalSearchParams } from 'expo-router'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import { useTinyvex, type ToolCallRow } from '@/providers/tinyvex'
import type { ToolCallContent as ToolCallContentType, ContentBlock } from '@/types/acp'
import { ContentText } from '@/components/acp/ContentText'
import { ContentImage } from '@/components/acp/ContentImage'
import { ContentAudio } from '@/components/acp/ContentAudio'
import { ContentResource } from '@/components/acp/ContentResource'
import { ContentResourceLink } from '@/components/acp/ContentResourceLink'
import { ToolCallContentDiff } from '@/components/acp/ToolCallContentDiff'
import { ToolCallContentTerminal } from '@/components/acp/ToolCallContentTerminal'
import { useHeaderTitle } from '@/lib/header-store'

export default function ToolCallDetailScreen() {
  const { id: threadId, toolId } = useLocalSearchParams<{ id: string; toolId: string }>()
  useHeaderTitle('Tool Call')
  const { toolCallsByThread, queryToolCalls } = useTinyvex()
  const rows: ToolCallRow[] = React.useMemo(() => toolCallsByThread[String(threadId)] ?? [], [toolCallsByThread, threadId])
  const row = React.useMemo(() => rows.find((r) => String(r.tool_call_id) === String(toolId)), [rows, toolId])
  React.useEffect(() => { try { if (threadId) queryToolCalls(String(threadId), 100) } catch {} }, [threadId, queryToolCalls])
  const content: ToolCallContentType[] = React.useMemo(() => {
    try {
      const j = (row?.content_json ? JSON.parse(String(row?.content_json)) : [])
      return Array.isArray(j) ? j : []
    } catch { return [] }
  }, [row])
  const locations: { path: string; line?: number }[] = React.useMemo(() => {
    try {
      const j = (row?.locations_json ? JSON.parse(String(row?.locations_json)) : [])
      return Array.isArray(j) ? j.map((x: any) => ({ path: String(x?.path || ''), line: (typeof x?.line === 'number' ? x.line : undefined) })).filter((x) => !!x.path) : []
    } catch { return [] }
  }, [row])
  return (
    <ScrollView style={{ flex: 1, backgroundColor: Colors.background }} contentContainerStyle={{ padding: 16, gap: 12 }}>
      {!row ? (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Loading…</Text>
      ) : (
        <View>
          <Text style={{ color: Colors.foreground, fontFamily: Typography.bold, fontSize: 16 }}>{row.title || 'Tool Call'}</Text>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary, marginTop: 4 }}>{String(row.kind || 'tool')} · {String(row.status || 'pending')}</Text>
        </View>
      )}
      {locations.length > 0 ? (
        <View style={{ gap: 2 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Locations</Text>
          {locations.map((loc, i) => (
            <Text key={i} selectable style={{ color: Colors.foreground, fontFamily: Typography.primary }}>{loc.path}{typeof loc.line === 'number' ? `:${loc.line}` : ''}</Text>
          ))}
        </View>
      ) : null}
      {content.length > 0 ? (
        <View style={{ gap: 10 }}>
          {content.map((c, i) => (
            <ToolCallContent key={i} content={c} />
          ))}
        </View>
      ) : (
        <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>No output.</Text>
      )}
    </ScrollView>
  )
}

function ToolCallContent({ content }: { content: ToolCallContentType }) {
  if (content.type === 'content') return <InlineContent block={content.content} />
  if (content.type === 'diff') return <ToolCallContentDiff path={content.path} oldText={(content as Extract<ToolCallContentType, { type: 'diff' }>).oldText} newText={(content as Extract<ToolCallContentType, { type: 'diff' }>).newText} />
  if (content.type === 'terminal') return <ToolCallContentTerminal terminalId={content.terminalId} />
  return null
}

function InlineContent({ block }: { block: ContentBlock }) {
  switch (block.type) {
    case 'text':
      return <ContentText text={block.text} />
    case 'image':
      return <ContentImage data={block.data} mimeType={block.mimeType} uri={block.uri} />
    case 'audio':
      return <ContentAudio mimeType={block.mimeType} />
    case 'resource':
      return <ContentResource resource={block.resource} />
    case 'resource_link':
      return <ContentResourceLink name={block.name} uri={block.uri} mimeType={block.mimeType} />
    default:
      return null
  }
}

