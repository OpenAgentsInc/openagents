import { describe, expect, test } from 'bun:test'
import type { MessageRow } from '../providers/tinyvex'
import { renderMessages } from './timeline'

function row(partial: Partial<MessageRow>): MessageRow {
  // Provide sensible defaults; cast after constructing minimal required fields
  const base = {
    id: 1,
    thread_id: 't-1',
    role: null as any,
    kind: 'message',
    text: '',
    item_id: null as any,
    partial: null as any,
    seq: null as any,
    ts: Date.now(),
    created_at: Date.now(),
    updated_at: Date.now(),
  }
  return { ...(base as any), ...(partial as any) } as MessageRow
}

describe('renderMessages', () => {
  test('classifies user and assistant messages', () => {
    const rows: MessageRow[] = [
      row({ id: 1, kind: 'message', role: 'user' as any, text: 'Hi' }),
      row({ id: 2, kind: 'message', role: 'assistant' as any, text: 'Hello' }),
    ]
    const items = renderMessages(rows)
    expect(items.length).toBe(2)
    expect(items[0].role).toBe('user')
    expect(items[0].text).toBe('Hi')
    expect(items[1].role).toBe('assistant')
    expect(items[1].text).toBe('Hello')
  })

  test('treats kind=assistant as assistant even if role missing', () => {
    const rows: MessageRow[] = [row({ id: 3, kind: 'assistant', role: null as any, text: 'Hey there' })]
    const items = renderMessages(rows)
    expect(items.length).toBe(1)
    expect(items[0].role).toBe('assistant')
    expect(items[0].text).toBe('Hey there')
  })

  test('skips duplicate texts', () => {
    const rows: MessageRow[] = [
      row({ id: 10, kind: 'message', role: 'assistant' as any, text: 'Same text' }),
      row({ id: 11, kind: 'message', role: 'assistant' as any, text: 'Same text' }),
    ]
    const items = renderMessages(rows)
    expect(items.length).toBe(1)
    expect(items[0].text).toBe('Same text')
  })

  test('skips XML/system context blocks', () => {
    const rows: MessageRow[] = [
      row({ id: 20, kind: 'message', role: 'assistant' as any, text: '<environment_context>...</environment_context>' }),
      row({ id: 21, kind: 'message', role: 'assistant' as any, text: 'Visible' }),
    ]
    const items = renderMessages(rows)
    expect(items.map((i) => i.text)).toEqual(['Visible'])
  })

  test('skips reason rows', () => {
    const rows: MessageRow[] = [
      row({ id: 30, kind: 'reason', role: null as any, text: '**thinking**' }),
      row({ id: 31, kind: 'message', role: 'user' as any, text: 'Hi' }),
    ]
    const items = renderMessages(rows)
    expect(items.length).toBe(1)
    expect(items[0].text).toBe('Hi')
  })
})

