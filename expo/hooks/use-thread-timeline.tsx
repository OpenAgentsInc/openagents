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
  const { messagesByThread, toolCallsByThread, threads } = useTinyvex()
  const acpUpdates = React.useMemo(() => eventsForThread(threadId), [eventsForThread, threadId])
  // Build candidate thread ids (alias + canonical) and merge rows across them.
  const candidateThreadIds = React.useMemo(() => {
    try {
      const ids = new Set<string>()
      const id = String(threadId || '')
      if (id) ids.add(id)
      const list = Array.isArray(threads) ? threads : []
      // Resolve canonical for alias id
      const row = list.find((r) => String(r.id) === id)
      const resume = row?.resume_id ? String(row.resume_id) : null
      if (resume) ids.add(resume)
      // Resolve alias for canonical id
      const alias = list.find((r) => String(r.resume_id ?? '') === id)?.id
      if (alias) ids.add(String(alias))
      return Array.from(ids)
    } catch { return [threadId] }
  }, [threads, threadId])
  const msgRows: MessageRow[] = React.useMemo(() => {
    // Gather rows across candidates and dedupe by stable key that ignores thread alias
    const rows: MessageRow[] = []
    for (const id of candidateThreadIds) {
      const arr = messagesByThread[id]
      if (Array.isArray(arr)) rows.push(...arr)
    }
    type Row = MessageRow & { item_id?: string | null }
    const keyFor = (r: Row) => String((r as any).item_id || `${(r as any).seq ?? ''}:${r.ts}:${(r as any).role ?? ''}:${String((r as any).text ?? '')}`)
    const isFinal = (r: Row) => (r as any).partial == null || Number((r as any).partial) === 0
    const prefer = (a: Row | undefined, b: Row): Row => {
      if (!a) return b
      const aFinal = isFinal(a), bFinal = isFinal(b)
      if (aFinal !== bFinal) return bFinal ? b : a
      const aRole = !!(a as any).role, bRole = !!(b as any).role
      if (aRole !== bRole) return bRole ? b : a
      return b.ts >= a.ts ? b : a
    }
    const map = new Map<string, Row>()
    for (const r of rows) {
      const key = keyFor(r as Row)
      map.set(key, prefer(map.get(key), r as Row))
    }
    // Only keep chat messages and finalized rows
    const merged = Array.from(map.values()).filter((r) => String((r as any).kind || '').toLowerCase() === 'message' && isFinal(r))
    merged.sort((a, b) => a.ts - b.ts)
    return merged as MessageRow[]
  }, [candidateThreadIds, messagesByThread])
  const toolRows: ToolCallRow[] = React.useMemo(() => toolCallsByThread[threadId] ?? [], [toolCallsByThread, threadId])

  // No debug logs in production — timeline derives purely from Tinyvex rows and live ACP.

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

  // Do not log item keys in production.

  // ACP updates
  // Show live ACP agent chunks as soon as they arrive for responsiveness,
  // but dedupe against Tinyvex‑persisted assistant texts so we don’t render
  // the same response twice when persistence completes.
  const seenAssistantTexts = new Set<string>()
  try {
    // Seed from Tinyvex-rendered items we already built above
    for (const it of rendered) {
      if (it.role === 'assistant') seenAssistantTexts.add(it.text)
    }
  } catch {}
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
      if (fullText && !seenAssistantTexts.has(fullText)) {
        items.push({ key: `acp-a-${ts}-${i}`, ts, node: <SessionUpdateAgentMessageChunk content={content} /> })
        // Track so multiple ACP chunks with identical text don’t spam
        seenAssistantTexts.add(fullText)
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
