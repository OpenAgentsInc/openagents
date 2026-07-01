import { describe, expect, test } from 'bun:test'
import { existsSync, readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

import { themeCssVars } from '@openagentsinc/design-tokens'

// #6046 part 2: the component stylesheets must reference the central --oa-* tokens, and every
// var(--oa-*, FALLBACK) fallback must EQUAL the token's canonical value so the
// migration is theme-preserving. We also assert NO raw hex colors remain.

const here = (rel: string) => fileURLToPath(new URL(rel, import.meta.url))

const componentCssFiles = [
  // @openagentsinc/ui
  here('../src/shared.css'),
  here('../src/forms.css'),
  here('../src/feedback.css'),
  here('../src/workroom-styles.css'),
  here('../src/ai-elements/prompt-input.css'),
  here('../src/ai-elements/command-composer.css'),
  here('../src/ai-elements/shimmer.css'),
  // @openagentsinc/autopilot-ui
  here('../../autopilot-ui/src/view.css'),
  here('../../autopilot-ui/src/domain-styles.css'),
]

const read = (file: string) => readFileSync(file, 'utf8')

// Strip /* … */ comments so the `#6046` marker (and any other commentary) does
// not count as a color or a token reference.
const stripComments = (css: string) => css.replace(/\/\*[\s\S]*?\*\//g, '')

// Matches `var(--oa-<name>, <fallback>)` capturing the var name and the literal
// fallback (which may itself contain commas, e.g. rgba(...)). The fallback runs
// up to the matching close paren of the var(), so we balance one level of
// nested parens (rgba/color-mix never nest var()s in our fallbacks).
const varRefRe = /var\(\s*(--oa-[a-z0-9-]+)\s*,\s*((?:[^()]|\([^()]*\))*?)\s*\)/g

describe('component CSS is migrated onto the central --oa-* tokens', () => {
  const vars = themeCssVars()

  test('@openagentsinc/ui/styles.css exports the shared design-token stylesheet', () => {
    const css = stripComments(read(here('../src/styles.css')))
    expect(css).toContain("@import '@openagentsinc/design-tokens/theme.css';")
    expect(css.indexOf("@import '@openagentsinc/design-tokens/theme.css';")).toBeLessThan(
      css.indexOf("@import './shared.css';"),
    )
  })

  test('every var(--oa-*, fallback) fallback equals the canonical token value', () => {
    let checked = 0
    for (const file of componentCssFiles) {
      const css = stripComments(read(file))
      for (const match of css.matchAll(varRefRe)) {
        const name = match[1]
        const fallback = match[2].trim()
        expect(vars).toHaveProperty(name)
        // theme-preserving: the inline fallback must be the exact token value,
        // so resolved-or-fallback renders identically to the original literal.
        expect(`${name} -> ${fallback}`).toBe(`${name} -> ${vars[name]}`)
        checked += 1
      }
    }
    // Guard against the regex silently matching nothing.
    expect(checked).toBeGreaterThan(40)
  })

  test('0 raw hex colors remain in any component stylesheet', () => {
    const hexRe = /#[0-9a-fA-F]{3,8}\b/g
    for (const file of componentCssFiles) {
      const css = stripComments(read(file))
      // Remove the var() fallbacks (which legitimately carry the hex literal as
      // a safety fallback) before scanning for stray raw hex.
      const withoutVarFallbacks = css.replace(varRefRe, 'var($1)')
      const stray = withoutVarFallbacks.match(hexRe) ?? []
      expect({ file, stray }).toEqual({ file, stray: [] })
    }
  })

  test('every component stylesheet references at least one --oa-* token', () => {
    for (const file of componentCssFiles) {
      const css = stripComments(read(file))
      expect({ file, hasOaToken: css.includes('var(--oa-') }).toEqual({
        file,
        hasOaToken: true,
      })
    }
  })

  test('legacy style adapter module is absent from the component layer', () => {
    const legacyName = ['style', 'x'].join('')
    const legacyModule = `${legacyName}-foldkit`
    const scannedFiles = [
      here('../src/index.ts'),
      here('../src/shared.ts'),
      here('../src/forms.ts'),
      here('../src/feedback.ts'),
      here('../src/workroom.ts'),
      here('../src/workroom-styles.ts'),
      here('../src/ai-elements/prompt-input.ts'),
      here('../../autopilot-ui/src/view.ts'),
      here('../../autopilot-ui/src/domain-styles.ts'),
    ]

    expect(existsSync(here(`../src/${legacyModule}.ts`))).toBe(false)
    for (const file of scannedFiles) {
      expect(read(file).includes(legacyModule)).toBe(false)
    }
  })
})
