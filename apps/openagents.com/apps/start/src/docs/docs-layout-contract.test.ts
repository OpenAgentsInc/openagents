import { readFileSync } from 'node:fs'
import path from 'node:path'
import { describe, expect, test } from 'vitest'

describe('documentation shell contract', () => {
  test('uses one fixed public header with integrated docs controls', () => {
    const layout = readFileSync(path.resolve(import.meta.dirname, 'DocsLayout.tsx'), 'utf8')
    const docsCss = readFileSync(path.resolve(import.meta.dirname, 'docs.css'), 'utf8')
    const headerCss = readFileSync(path.resolve(import.meta.dirname, '../public-header.css'), 'utf8')
    const globalCss = readFileSync(path.resolve(import.meta.dirname, '../styles.css'), 'utf8')
    const viteConfig = readFileSync(path.resolve(import.meta.dirname, '../../vite.config.ts'), 'utf8')
    const bodyRule = globalCss.match(/\n  body \{([\s\S]*?)\n  \}/)?.[1]

    expect(layout).toContain('<PublicHeader')
    expect(layout).toContain('docsActive')
    expect(layout).toContain('utility={<DocsSearch />}')
    expect(layout).toContain('variant="docs"')
    expect(layout).not.toContain('className="docs-toolbar"')
    expect(docsCss).toContain('--docs-header-height: 4.25rem')
    expect(docsCss).not.toContain('.docs-toolbar {')
    expect(headerCss).toContain('position: fixed')
    expect(headerCss).toContain('.oa-unified-header-spacer')
    expect(headerCss).toContain('.oa-unified-header--docs .oa-unified-nav')
    expect(headerCss).toContain('grid-template-columns: 16rem minmax(0, 1fr)')
    expect(docsCss).toContain('.docs-shell {')
    expect(docsCss).toContain('width: 100%')
    expect(docsCss).toContain('left: 0')
    expect(docsCss).toContain('html:has(.docs-drawer[open])')
    expect(docsCss).toContain('position: sticky')
    expect(docsCss).toContain('.docs-prose > h2:first-child')
    expect(docsCss).toMatch(/\.docs-prose > h2:first-child \{[\s\S]*?border-top: 0/)
    expect(viteConfig).toContain("routeId.startsWith('/docs') ? [] : undefined")
    expect(bodyRule).toContain('overflow-x: clip')
    expect(bodyRule).not.toContain('overflow-x: hidden')
    expect(globalCss).toContain('overscroll-behavior: none')
    expect(globalCss).toContain('overscroll-behavior: auto')
    expect(globalCss).not.toContain('body > main {\n    overscroll-behavior: none')
    expect(layout).not.toContain("document.documentElement.style.overflow = 'hidden'")
  })
})
