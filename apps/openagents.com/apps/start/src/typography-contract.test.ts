import { readFileSync } from 'node:fs'
import path from 'node:path'

import { describe, expect, test } from 'vitest'

const sourceDirectory = import.meta.dirname
const sharedTypographyPath = path.resolve(
  sourceDirectory,
  '../../../../../packages/ui/src/typography.css',
)

describe('shared typography authority', () => {
  test('self-hosts the owner-selected sans and mono faces', () => {
    const typography = readFileSync(sharedTypographyPath, 'utf8')

    expect(typography).toContain('font-family: "Zalando Sans"')
    expect(typography).toContain('font-family: "Disket Mono"')
    expect(typography).toContain('--oa-font-sans: "Zalando Sans", Inter')
    expect(typography).toContain('--oa-font-mono: "Disket Mono"')
    expect(typography).toContain('font-variation-settings: "wdth" 100')
  })

  test('wires website and docs to shared tokens without a Google Fonts request', () => {
    const styles = readFileSync(path.join(sourceDirectory, 'styles.css'), 'utf8')
    const publicSite = readFileSync(path.join(sourceDirectory, 'public-site.css'), 'utf8')
    const docs = readFileSync(path.join(sourceDirectory, 'docs/docs.css'), 'utf8')
    const root = readFileSync(path.join(sourceDirectory, 'routes/__root.tsx'), 'utf8')

    expect(styles).toContain("@import '@openagentsinc/ui/typography.css'")
    expect(styles).toContain('--font-sans: var(--oa-font-sans)')
    expect(styles).toContain('--font-mono: var(--oa-font-mono)')
    expect(publicSite).toContain('font-family: var(--oa-font-sans)')
    expect(publicSite).toContain('--oa-mono: var(--oa-font-mono)')
    expect(docs).toContain('font-family: var(--oa-font-sans)')
    expect(docs).toContain('font-family: var(--oa-font-mono)')
    expect(root).not.toContain('fonts.googleapis.com')
    expect(root).not.toContain('fonts.gstatic.com')
  })
})
