import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { describe, expect, test } from 'vitest'

const readCss = (relativePath: string): string =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8')

describe('Khala scrollbar theme', () => {
  test('uses the shared Starcraft energy palette for visible scrollbars', () => {
    const css = readCss('./styles.css')

    expect(css).toContain('--oa-scrollbar-track:')
    expect(css).toContain('--oa-scrollbar-thumb:')
    expect(css).toContain('--oa-scrollbar-thumb-highlight:')
    expect(css).toContain('*::-webkit-scrollbar-thumb')
    expect(css).toContain('*::-webkit-scrollbar-thumb:hover')
    expect(css).toContain('var(--oa-scrollbar-thumb-hover)')
    expect(css).toMatch(
      /scrollbar-color:\s*var\(--oa-scrollbar-thumb-highlight\)\s+var\(--oa-scrollbar-track\)/,
    )
    expect(css).not.toContain('scrollbar-color: #333 transparent')
    expect(css).not.toContain('background-color: #2a2a2a')
  })

  test('keeps intentionally hidden workroom scroll regions hidden in WebKit', () => {
    const css = readCss(
      '../../../../../packages/ui/src/workroom-styles.css',
    )

    for (const selector of [
      '.oa-ui-workroom-tool-output::-webkit-scrollbar',
      '.oa-ui-workroom-bash-scroll::-webkit-scrollbar',
      '.oa-ui-workroom-write-content::-webkit-scrollbar',
    ]) {
      expect(css).toContain(selector)
    }

    expect(css.match(/::-webkit-scrollbar\s*\{[^}]*display:\s*none/g)).toHaveLength(
      3,
    )
  })
})
