import { describe, expect, test } from 'bun:test'
import type { Html } from 'foldkit/html'

import { AiElements } from '../src/index'
import { parseUnifiedDiff } from '../src/ai-elements/code-highlight'

type VNodeLike = {
  sel?: string
  data?: { attrs?: Record<string, unknown>; class?: Record<string, boolean> }
  children?: ReadonlyArray<VNodeLike | string>
  text?: string
}
const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null
const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const classNames = Object.entries(node.data?.class ?? {}).flatMap(
    ([c, on]) => (on ? [c] : []),
  )
  const entries = [
    ...Object.entries(attrs),
    ...(classNames.length > 0 ? [['class', classNames.join(' ')]] : []),
  ]
  return entries
    .map(([k, v]) => (v === '' ? ` ${k}=""` : ` ${k}="${String(v)}"`))
    .join('')
}
const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) return ''
  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(c => (typeof c === 'string' ? c : renderHtml(c as Html)))
    .join('')
  return `<${tag}${attrsToString(html)}>${html.text ?? ''}${children}</${tag}>`
}

const SAMPLE = `diff --git a/greet.ts b/greet.ts
index 1111111..2222222 100644
--- a/greet.ts
+++ b/greet.ts
@@ -1,4 +1,4 @@
 export const greet = (name: string): string => {
-  return "Hello " + name
+  return \`Hello, \${name}!\`
 }
`

describe('parseUnifiedDiff', () => {
  test('counts adds/removes, extracts filename, and numbers lines', () => {
    const parsed = parseUnifiedDiff(SAMPLE)
    expect(parsed.filename).toBe('greet.ts')
    expect(parsed.added).toBe(1)
    expect(parsed.removed).toBe(1)

    const kinds = parsed.rows.map(r => r.kind)
    expect(kinds).toEqual(['hunk', 'context', 'remove', 'add', 'context'])

    // git/index/---/+++ header noise is consumed, not rendered.
    expect(parsed.rows.some(r => r.text.startsWith('diff --git'))).toBe(false)

    const add = parsed.rows.find(r => r.kind === 'add')
    const remove = parsed.rows.find(r => r.kind === 'remove')
    expect(remove?.oldNo).toBe(2)
    expect(add?.newNo).toBe(2)
  })

  test('honors a filename override', () => {
    const parsed = parseUnifiedDiff('@@ -1 +1 @@\n-a\n+b\n', 'custom.rs')
    expect(parsed.filename).toBe('custom.rs')
    expect(parsed.added).toBe(1)
    expect(parsed.removed).toBe(1)
  })

  test('keeps bare diff snippets visible when no patch file is present', () => {
    const parsed = parseUnifiedDiff('-old\n+new\n')
    expect(parsed.added).toBe(1)
    expect(parsed.removed).toBe(1)
    expect(parsed.rows.map(row => row.kind)).toEqual(['remove', 'add'])
  })
})

describe('ai-elements diff', () => {
  test('renders a framed, copyable diff with stats and tinted lines', () => {
    const rendered = renderHtml(
      AiElements.diff({ props: { patch: SAMPLE, language: 'typescript' } }),
    )

    expect(rendered).toContain('ai-elements:diff/Diff')
    expect(rendered).toContain('ai-elements:diff/DiffHeader')
    expect(rendered).toContain('ai-elements:diff/DiffHunk')
    expect(rendered).toContain('ai-elements:diff/DiffLine')

    // Header stats + filename.
    expect(rendered).toContain('greet.ts')
    expect(rendered).toContain('+1')
    expect(rendered).toContain('−1')

    // Add/remove line tints + copy hooks (reuses the code-block copy controller).
    expect(rendered).toContain('data-diff-line="add"')
    expect(rendered).toContain('data-diff-line="remove"')
    expect(rendered).toContain('bg-[#0f3320]') // green add tint
    expect(rendered).toContain('bg-[#3a161a]') // red remove tint
    expect(rendered).toContain('border-[#2ea043]') // green accent bar
    expect(rendered).toContain('border-[#e5484d]') // red accent bar
    expect(rendered).toContain('data-oa-code-copy=""')
    expect(rendered).toContain('data-oa-code-source=""')
  })
})
