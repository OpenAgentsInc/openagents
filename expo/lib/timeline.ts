import type { MessageRow } from '../providers/tinyvex'

export type MessageRender = {
  key: string
  ts: number
  role: 'assistant' | 'user'
  text: string
}

// Build renderable message items from Tinyvex message rows.
// - Skips reason rows
// - Skips XML/system context blocks (text starting with '<')
// - Deduplicates identical text within the same batch
// - Classifies assistant vs user
export function renderMessages(msgRows: MessageRow[]): MessageRender[] {
  const items: MessageRender[] = []
  const seenTexts = new Set<string>()
  for (const r of msgRows) {
    const ts = Number(r.ts || r.updated_at || r.created_at || Date.now())
    const kind = String(r.kind || '').toLowerCase()
    const text = String(r.text || '')

    if (kind === 'reason') continue
    if (text.trim().startsWith('<')) continue
    if (seenTexts.has(text)) continue
    seenTexts.add(text)

    const isAssistant = kind === 'assistant' || (kind === 'message' && String(r.role || '').toLowerCase() === 'assistant')
    const role: 'assistant' | 'user' = isAssistant ? 'assistant' : 'user'
    items.push({ key: `msg-${r.id}`, ts, role, text })
  }
  return items
}
