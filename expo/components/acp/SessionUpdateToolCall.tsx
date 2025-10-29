import React from 'react'
import { View, Text } from 'react-native'
import { Colors } from '@/constants/theme'
import { Typography } from '@/constants/typography'
import type { ToolCall, ToolCallContent as ToolCallContentType, ContentBlock } from '@/types/acp'
import { MaterialCommunityIcons } from '@expo/vector-icons'
import { ContentText } from './ContentText'
import { ContentImage } from './ContentImage'
import { ContentAudio } from './ContentAudio'
import { ContentResource } from './ContentResource'
import { ContentResourceLink } from './ContentResourceLink'
import { ToolCallContentDiff } from './ToolCallContentDiff'
import { ToolCallContentTerminal } from './ToolCallContentTerminal'

export function SessionUpdateToolCall(props: ToolCall) {
  const icon = iconForKind(props.kind)
  const statusColor = colorForStatus(props.status)
  return (
    <View style={{ borderWidth: 1, borderColor: Colors.border, backgroundColor: Colors.card, borderRadius: 0, padding: 12, gap: 8 }}>
      <View style={{ flexDirection: 'row', alignItems: 'center', gap: 8 }}>
        <MaterialCommunityIcons name={icon as any} size={18} color={statusColor} />
        <Text style={{ color: Colors.foreground, fontFamily: Typography.primary, fontSize: 13 }}>{props.title}</Text>
        <View style={{ marginLeft: 'auto' }} />
        <Text style={{ color: statusColor, fontFamily: Typography.primary, fontSize: 12 }}>{props.status}</Text>
      </View>
      {Array.isArray(props.content) && props.content.length > 0 ? (
        <View style={{ gap: 8 }}>
          {props.content.map((c, i) => (
            <ToolCallContent key={i} content={c} />
          ))}
        </View>
      ) : null}
      {Array.isArray(props.locations) && props.locations.length > 0 ? (
        <View style={{ gap: 2 }}>
          <Text style={{ color: Colors.secondary, fontFamily: Typography.primary }}>Locations:</Text>
          {props.locations!.map((loc, i) => (
            <Text key={i} selectable style={{ color: Colors.foreground, fontFamily: Typography.primary }}>
              {loc.path}{typeof loc.line === 'number' ? `:${loc.line}` : ''}
            </Text>
          ))}
        </View>
      ) : null}
    </View>
  )
}

function ToolCallContent({ content }: { content: ToolCallContentType }) {
  if (content.type === 'content') return <InlineContent block={content.content} />
  if (content.type === 'diff') return <ToolCallContentDiff path={content.path} oldText={(content as any).oldText} newText={(content as any).newText} />
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
      return <ContentResource resource={block.resource as any} />
    case 'resource_link':
      return <ContentResourceLink name={block.name} uri={block.uri} mimeType={block.mimeType} />
    default:
      return null
  }
}

function colorForStatus(status: ToolCall['status']): string {
  switch (status) {
    case 'completed':
      return Colors.success
    case 'failed':
      return Colors.danger
    case 'in_progress':
      return Colors.secondary
    case 'pending':
    default:
      return Colors.tertiary
  }
}

function iconForKind(kind: ToolCall['kind']): any {
  switch (kind) {
    case 'execute':
      return 'console'
    case 'search':
      return 'magnify'
    case 'edit':
      return 'pencil'
    case 'read':
      return 'file-eye-outline'
    case 'delete':
      return 'delete'
    case 'move':
      return 'file-move'
    case 'fetch':
      return 'cloud-download-outline'
    case 'think':
      return 'lightbulb-on-outline'
    case 'switch_mode':
      return 'swap-horizontal'
    case 'other':
    default:
      return 'dots-horizontal'
  }
}
