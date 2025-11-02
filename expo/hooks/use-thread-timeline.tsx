import React from 'react'
import { useAcp, type SessionNotificationWithTs } from '@/providers/acp'
import type {
  SessionUpdate,
  AgentMessageChunk,
  UserMessageChunk,
  PlanUpdate,
  ToolCallCreate,
  ToolCallUpdate,
  ToolCallLike,
} from '@/types/acp'
import { useTinyvex, type MessageRow, type ToolCallRow } from '@/providers/tinyvex'
import { renderMessages } from '@/lib/timeline'
import { SessionUpdateAgentMessageChunk } from '@/components/acp/SessionUpdateAgentMessageChunk'
import { SessionUpdateUserMessageChunk } from '@/components/acp/SessionUpdateUserMessageChunk'
import { SessionUpdatePlan } from '@/components/acp/SessionUpdatePlan'
import { SessionUpdateToolCall } from '@/components/acp/SessionUpdateToolCall'
import { Pressable } from 'react-native'
import { router } from 'expo-router'

export type TimelineItem = { key: string; ts: number; node: React.ReactNode }

export function useThreadTimeline(threadId: string): TimelineItem[] {
  const { eventsForThread } = useAcp()
  const { messagesByThread, toolCallsByThread } = useTinyvex()
  const acpUpdates = React.useMemo(() => eventsForThread(threadId), [eventsForThread, threadId])
  const msgRows: MessageRow[] = React.useMemo(() => messagesByThread[threadId] ?? [], [messagesByThread, threadId])
  const toolRows: ToolCallRow[] = React.useMemo(() => toolCallsByThread[threadId] ?? [], [toolCallsByThread, threadId])

  // DEBUG: Log message rows to diagnose rendering issue
  React.useEffect(() => {
    if (msgRows.length > 0) {
      console.log(`[Timeline Debug] threadId=${threadId} msgRows.length=${msgRows.length}`)
      msgRows.forEach((r, idx) => {
        console.log(`[Timeline Debug] msg[${idx}]: id=${r.id} kind=${r.kind} role=${r.role} text=${r.text?.substring(0, 50)}...`)
      })
    }
  }, [threadId, msgRows])

  const items: TimelineItem[] = []
  // Tinyvex messages: transform via shared utility
  const rendered = renderMessages(msgRows)
  for (const it of rendered) {
    const content: { type: 'text'; text: string } = { type: 'text', text: it.text }
    if (it.role === 'assistant') {
      items.push({ key: `tvx-a-${it.key}`, ts: it.ts, node: <SessionUpdateAgentMessageChunk content={content} /> })
    } else {
      items.push({ key: `tvx-u-${it.key}`, ts: it.ts, node: <SessionUpdateUserMessageChunk content={content} /> })
    }
  }

  // DEBUG: Log final items count
  console.log(`[Timeline Debug] Final items count: ${items.length}`)
  items.forEach((item, idx) => {
    console.log(`[Timeline Debug] Item[${idx}]: key=${item.key}`)
  })

  // ACP updates
  // Watermark: if Tinyvex has already persisted messages up to a timestamp,
  // suppress older ACP chat chunks to avoid duplicate rendering of the same
  // content (live ACP → persisted Tinyvex).
  const tvxMaxTs = (() => {
    try {
      if (!msgRows.length) return 0
      return Number(msgRows[msgRows.length - 1]?.ts || 0)
    } catch { return 0 }
  })()
  for (let i = 0; i < acpUpdates.length; i++) {
    const n = acpUpdates[i]
    if (!n || !('update' in n)) continue
    const u = n.update
    if (!u) continue
    const ts = Number(n.addedAt || Date.now())
    // Do not render ACP user messages; Tinyvex persists user rows immediately
    // at send time to avoid duplicates.
    if (u.sessionUpdate === 'user_message_chunk') {
      continue
    }
    if (u.sessionUpdate === 'agent_message_chunk') {
      const m = u as AgentMessageChunk
      // Extract text from content union type - content is an array
      const contentArray = Array.isArray(m.content) ? m.content : []
      const textContent = contentArray.find((c: { type: string }) => c.type === 'text')
      const fullText = textContent && 'text' in textContent ? String(textContent.text || '') : ''
      const content: { type: 'text'; text: string } = { type: 'text', text: fullText }
      if (ts > tvxMaxTs) {
        items.push({ key: `acp-a-${ts}-${i}`, ts, node: <SessionUpdateAgentMessageChunk content={content} /> })
      }
      continue
    }
    if (u.sessionUpdate === 'plan') {
      const p = u as PlanUpdate
      items.push({ key: `acp-plan-${ts}-${i}`, ts, node: <SessionUpdatePlan entries={p.entries} /> })
      continue
    }
    // Do not render ACP tool_call/create/update inline; rely on Tinyvex-hydrated
    // tool call rows so we can dedupe and provide navigation to details.
    if (u.sessionUpdate === 'tool_call' || u.sessionUpdate === 'tool_call_update') {
      continue
    }
  }

  // Helpers: normalizers for Tinyvex tool calls
  function normalizeKind(raw?: string | null): 'execute'|'edit'|'search'|'fetch'|'read'|'delete'|'move'|'think'|'switch_mode'|'other' {
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
  function normalizeStatus(raw?: string | null): 'completed'|'failed'|'in_progress'|'pending' {
    const s = (raw || '').toLowerCase()
    if (s.includes('complete')) return 'completed'
    if (s.includes('fail')) return 'failed'
    if (s.includes('progress')) return 'in_progress'
    return 'pending'
  }
  function parseLocations(json?: string | null): { path: string; line?: number }[] {
    if (!json) return []
    try {
      const v = JSON.parse(json)
      return Array.isArray(v) ? v.slice(0, 8).map((x: any) => ({ path: String(x?.path || ''), line: (typeof x?.line === 'number' ? x.line : undefined) })).filter((x: any) => !!x.path) : []
    } catch { return [] }
  }

  // Hydrated tool calls (Tinyvex) — dedupe by tool_call_id to latest
  function stripRunPrefix(raw?: string | null): string {
    const s = String(raw || '')
    return s.replace(/^\s*Run:\s*/i, '')
  }
  const latestById = new Map<string, ToolCallRow>()
  for (const r of toolRows) {
    const id = String(r.tool_call_id || '')
    if (!id) continue
    const prev = latestById.get(id)
    if (!prev || Number(r.updated_at) > Number(prev.updated_at)) {
      latestById.set(id, r)
    }
  }
  const deduped = Array.from(latestById.values()).sort((a, b) => a.updated_at - b.updated_at)
  for (const r of deduped) {
    const ts = r.updated_at
    const key = `tvx-tool-${r.tool_call_id}`
    const title = stripRunPrefix(r.title || 'Tool')
    const status = normalizeStatus(r.status)
    const kind = normalizeKind(r.kind)
    const locations = parseLocations(r.locations_json)
    const props: ToolCallLike = { title, status, kind, content: [], locations }
    const go = () => { try { router.push(`/thread/${encodeURIComponent(threadId)}/tool/${encodeURIComponent(r.tool_call_id)}`) } catch {} }
    items.push({ key, ts, node: (
      <Pressable onPress={go} accessibilityRole="button">
        <SessionUpdateToolCall {...props} />
      </Pressable>
    ) })
  }

  items.sort((a, b) => a.ts - b.ts)
  return items
}
