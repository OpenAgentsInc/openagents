import React from 'react'
import { useAcp, type SessionNotificationWithTs } from '@/providers/acp'
import { useTinyvex, type TinyvexContextValue, type MessageRow, type ToolCallRow } from '@/providers/tinyvex'
import type { ToolCallLike } from '@/types/acp'
import { SessionUpdateAgentMessageChunk } from '@/components/acp/SessionUpdateAgentMessageChunk'
import { SessionUpdateUserMessageChunk } from '@/components/acp/SessionUpdateUserMessageChunk'
import { SessionUpdatePlan } from '@/components/acp/SessionUpdatePlan'
import { SessionUpdateToolCall } from '@/components/acp/SessionUpdateToolCall'

export type TimelineItem = { key: string; ts: number; node: React.ReactNode }

export function useThreadTimeline(threadId: string): TimelineItem[] {
  const { eventsForThread } = useAcp()
  const { messagesByThread, toolCallsByThread } = useTinyvex() as TinyvexContextValue
  const acpUpdates = React.useMemo(() => eventsForThread(threadId), [eventsForThread, threadId])
  const msgRows: MessageRow[] = React.useMemo(() => messagesByThread[threadId] ?? [], [messagesByThread, threadId])
  const toolRows: ToolCallRow[] = React.useMemo(() => toolCallsByThread[threadId] ?? [], [toolCallsByThread, threadId])

  const items: TimelineItem[] = []
  // Tinyvex messages: last N already ascending
  for (const r of msgRows) {
    const ts = Number(r.ts || r.updated_at || r.created_at || Date.now())
    const kind = String(r.kind || '').toLowerCase()
    if (kind === 'reason') continue // omit inline; shown in detail
    if (kind === 'assistant' || (kind === 'message' && (r.role || '').toLowerCase() === 'assistant')) {
      const firstLine = String(r.text || '').split('\n')[0]
      const content: { type: 'text'; text: string } = { type: 'text', text: firstLine }
      items.push({ key: `tvx-a-${r.id}`, ts, node: <SessionUpdateAgentMessageChunk content={content} /> })
    } else {
      const content: { type: 'text'; text: string } = { type: 'text', text: String(r.text || '') }
      items.push({ key: `tvx-u-${r.id}`, ts, node: <SessionUpdateUserMessageChunk content={content} /> })
    }
  }

  // ACP updates
  for (let i = 0; i < acpUpdates.length; i++) {
    const n = acpUpdates[i] as SessionNotificationWithTs
    const u: any = (n as any).update
    if (!u) continue
    const ts = Number((n as any).addedAt || Date.now())
    if (u.sessionUpdate === 'user_message_chunk') {
      items.push({ key: `acp-u-${ts}-${i}`, ts, node: <SessionUpdateUserMessageChunk content={u.content} /> })
      continue
    }
    if (u.sessionUpdate === 'agent_message_chunk') {
      const fullText = (u.content && (u.content as any).text) || ''
      const firstLine = String(fullText).split('\n')[0]
      const content: { type: 'text'; text: string } = { type: 'text', text: firstLine }
      items.push({ key: `acp-a-${ts}-${i}`, ts, node: <SessionUpdateAgentMessageChunk content={content} /> })
      continue
    }
    if (u.sessionUpdate === 'plan') {
      items.push({ key: `acp-plan-${ts}-${i}`, ts, node: <SessionUpdatePlan entries={u.entries} /> })
      continue
    }
    if (u.sessionUpdate === 'tool_call' || u.sessionUpdate === 'tool_call_update') {
      const props: any = u.sessionUpdate === 'tool_call' ? {
        title: u.title,
        status: u.status,
        kind: u.kind,
        content: u.content,
        locations: u.locations,
      } : {
        title: u.fields?.title,
        status: u.fields?.status,
        kind: u.fields?.kind,
        content: u.fields?.content,
        locations: u.fields?.locations,
      }
      items.push({ key: `acp-tool-${ts}-${i}`, ts, node: <SessionUpdateToolCall {...props} /> })
      continue
    }
  }

  // Helpers: normalizers for Tinyvex tool calls
  function normalizeKind(raw?: string): 'execute'|'edit'|'search'|'fetch'|'read'|'delete'|'move'|'think'|'switch_mode'|'other' {
    const s = (raw || '').toLowerCase()
    if (s.includes('execute')) return 'execute'
    if (s.includes('edit')) return 'edit'
    if (s.includes('search')) return 'search'
    if (s.includes('fetch')) return 'fetch'
    if (s.includes('read')) return 'read'
    if (s.includes('delete')) return 'delete'
    if (s.includes('move')) return 'move'
    if (s.includes('think')) return 'think'
    if (s.includes('switch')) return 'switch_mode'
    return 'other'
  }
  function normalizeStatus(raw?: string): 'completed'|'failed'|'in_progress'|'pending' {
    const s = (raw || '').toLowerCase()
    if (s.includes('complete')) return 'completed'
    if (s.includes('fail')) return 'failed'
    if (s.includes('progress')) return 'in_progress'
    return 'pending'
  }
  function parseLocations(json?: string): { path: string; line?: number }[] {
    if (!json) return []
    try {
      const v = JSON.parse(json)
      return Array.isArray(v) ? v.slice(0, 8).map((x: any) => ({ path: String(x?.path || ''), line: (typeof x?.line === 'number' ? x.line : undefined) })).filter((x: any) => !!x.path) : []
    } catch { return [] }
  }

  // Hydrated tool calls (Tinyvex) — typed, minimal mapping
  for (const r of toolRows) {
    const ts = r.updated_at
    const key = `tvx-tool-${r.tool_call_id}`
    const title = r.title || 'Tool'
    const status = normalizeStatus(r.status)
    const kind = normalizeKind(r.kind)
    const locations = parseLocations(r.locations_json)
    const props: ToolCallLike = { title, status, kind, content: [], locations }
    items.push({ key, ts, node: <SessionUpdateToolCall {...props} /> })
  }

  items.sort((a, b) => a.ts - b.ts)
  return items
}
