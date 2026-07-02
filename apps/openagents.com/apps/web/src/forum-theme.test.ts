import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { describe, expect, test } from 'vitest'

const readSource = (relativePath: string): string =>
  readFileSync(fileURLToPath(new URL(relativePath, import.meta.url)), 'utf8')

describe('Forum Khala theme guard', () => {
  test('maps forum utility tokens onto the Khala palette', () => {
    const css = readSource('./styles.css')

    for (const expected of [
      '--color-forum-heading: var(--oa-color-khala-text-bright);',
      '--color-forum-header: var(--oa-color-khala-surface-active);',
      '--color-forum-link: var(--oa-color-khala-energy-soft);',
      '--color-forum-link-hover: var(--oa-color-khala-energy-cyan);',
      '--color-forum-navbar: var(--oa-color-khala-surface-muted);',
      '--color-forum-page: var(--oa-color-khala-surface);',
      '--color-forum-panel: var(--oa-color-khala-surface-raised);',
      '--color-forum-payment: var(--oa-color-khala-energy-line);',
      '--color-forum-post-link: var(--oa-color-khala-energy-blue);',
      '--color-forum-row-c: var(--oa-color-khala-border);',
      '--color-forum-text: var(--oa-color-khala-text-muted);',
      '--color-forum-wrap-border: var(--oa-color-khala-border);',
    ]) {
      expect(css).toContain(expected)
    }

    expect(css).toContain('[data-forum-shell]')
    expect(css).toContain('html:has([data-forum-shell])')
    expect(css).toContain('background: var(--oa-color-khala-surface);')
    expect(css).toContain('color-scheme: dark')
    expect(css).not.toContain(':root[data-forum-theme')
    expect(css).not.toContain('--color-forum-alert:')
    expect(css).not.toContain('--color-forum-online:')
  })

  test('does not reintroduce forum theme selector machinery', () => {
    const forumSource = readSource('./page/forum.ts')
    const headerSource = readSource('./page/publicHeader.ts')
    const combined = `${forumSource}\n${headerSource}`

    for (const forbidden of [
      'data-forum-theme',
      'forum-theme-select',
      'oa.forum.v1:theme',
      'prefers-color-scheme',
      'System theme',
      'from-[#5a9ad9]',
      'to-[#3a72b0]',
      '#ffb400',
      'bg-[#010102]',
      'border-[#222]',
      'text-white',
      'bg-white',
      'from-white',
    ]) {
      expect(combined).not.toContain(forbidden)
    }

    expect(headerSource).toContain('khala-focus')
    expect(headerSource).toContain('var(--oa-color-khala-surface)')
    expect(forumSource).toContain('khala-panel')
    expect(forumSource).toContain('khala-index')
  })
})
