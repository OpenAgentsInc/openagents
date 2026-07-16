import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, test } from 'vitest'

describe('documentation shell contract', () => {
  test('uses one fixed public header with integrated docs controls', () => {
    const layout = readFileSync(path.resolve(import.meta.dirname, 'DocsLayout.tsx'), 'utf8')
    const docsCss = readFileSync(path.resolve(import.meta.dirname, 'docs.css'), 'utf8')
    const headerCss = readFileSync(path.resolve(import.meta.dirname, '../public-header.css'), 'utf8')
    const globalCss = readFileSync(path.resolve(import.meta.dirname, '../styles.css'), 'utf8')

    expect(layout).toContain('<PublicHeader')
    expect(layout).toContain('docsActive')
    expect(layout).toContain('utility={<DocsSearch />}')
    expect(layout).not.toContain('className="docs-toolbar"')
    expect(docsCss).toContain('--docs-header-height: 4.25rem')
    expect(docsCss).not.toContain('.docs-toolbar {')
    expect(headerCss).toContain('position: fixed')
    expect(headerCss).toContain('.oa-unified-header-spacer')
    expect(globalCss).toContain('overscroll-behavior: none')
  })
})
