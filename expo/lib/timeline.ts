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
    const ts = Number((r as any).ts || (r as any).updated_at || (r as any).created_at || Date.now())
    const kind = String((r as any).kind || '').toLowerCase()
    const text = String((r as any).text || '')

    if (kind === 'reason') continue
    if (text.trim().startsWith('<')) continue
    if (seenTexts.has(text)) continue
    seenTexts.add(text)

    const isAssistant = kind === 'assistant' || (kind === 'message' && String((r as any).role || '').toLowerCase() === 'assistant')
    const role: 'assistant' | 'user' = isAssistant ? 'assistant' : 'user'
    items.push({ key: `msg-${(r as any).id}`, ts, role, text })
  }
  return items
}

