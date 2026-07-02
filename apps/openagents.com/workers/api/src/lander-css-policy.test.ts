import { describe, expect, test } from 'vitest'

import { renderBusinessNewHtml } from './business-new-routes'
import { LANDER_SHELL_CSS } from './lander-shell'
import { renderLander2Html } from './lander2-routes'
import { renderLander3Html } from './lander3-routes'
import { renderLander4Html } from './lander4-routes'
import { renderLander5Html } from './lander5-routes'

// Guard for the 2026-07-02 incident class: a `padding`/`margin` SHORTHAND on
// one selector silently zeroed the sides another rule owned, twice — the
// header lost its inline padding to `header.site{padding:18px 0 16px}`, then
// the hero lost its block padding to `.shell{padding:0 clamp(...)}` beating
// the bare `main{padding:...}` selector. Shorthands clobber all four sides at
// once, so ANY specificity overlap becomes a silent layout bug. Logical
// longhands (`padding-block` / `padding-inline`) compose per axis, which
// makes the collision structurally impossible: an inline-owning rule and a
// block-owning rule can never erase each other.
//
// Policy, enforced over every <style> block each lander page actually serves:
// 1. No `padding:` or `margin:` shorthand (the universal `*{...}` reset is
//    the one allowed exception — it exists to zero everything).
// 2. The shared `.shell` layout class owns the INLINE axis only; block
//    rhythm belongs to the page-section rules (header.site / main.shell /
//    footer.site), so the two can never fight again.

const styleBlocks = (html: string): readonly string[] =>
  [...html.matchAll(/<style>([\s\S]*?)<\/style>/g)].map(match => match[1] ?? '')

const shorthandOffenders = (css: string): readonly string[] => {
  const offenders: string[] = []
  for (const rule of css.split('}')) {
    const brace = rule.lastIndexOf('{')
    if (brace === -1) continue
    const selector = rule.slice(0, brace).trim()
    if (selector === '*' || selector.endsWith('*')) continue
    for (const declaration of rule.slice(brace + 1).split(';')) {
      if (/^\s*(padding|margin)\s*:/.test(declaration)) {
        offenders.push(`${selector} -> ${declaration.trim()}`)
      }
    }
  }
  return offenders
}

const pages: ReadonlyArray<readonly [string, string]> = [
  ['lander2', renderLander2Html(1)],
  ['lander3', renderLander3Html(1)],
  ['lander4', renderLander4Html(1)],
  ['lander5', renderLander5Html(1)],
  ['business-new', renderBusinessNewHtml(1)],
]

describe('lander CSS policy (specificity-collision guard)', () => {
  test.each(pages)('%s serves no padding/margin shorthands', (_name, html) => {
    const blocks = styleBlocks(html)
    expect(blocks.length).toBeGreaterThan(0)
    for (const css of blocks) {
      expect(shorthandOffenders(css)).toEqual([])
    }
  })

  test('the shared .shell class owns the inline axis only', () => {
    const shellRule = LANDER_SHELL_CSS.split('}').find(rule =>
      rule.includes('\n.shell{'),
    )
    expect(shellRule).toBeDefined()
    expect(shellRule).toContain('padding-inline:')
    expect(shellRule).not.toContain('padding-block')
    // Block rhythm lives on the page-section rules instead — and the
    // inverse holds: sections never touch the inline axis (a
    // footer.site{padding-inline:0} beat .shell's inline padding on
    // 2026-07-02 and knocked the footer out of alignment).
    for (const section of ['header\\.site', 'main\\.shell', 'footer\\.site']) {
      expect(LANDER_SHELL_CSS).toMatch(new RegExp(`${section}\\{[^}]*padding-block:`))
      expect(LANDER_SHELL_CSS).not.toMatch(new RegExp(`${section}\\{[^}]*padding-inline`))
    }
  })
})
