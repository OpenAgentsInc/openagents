import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { khalaTheme } from '@effect-native/tokens'
import { describe, expect, test } from 'vitest'

const cssPath = fileURLToPath(new URL('./effect-native-theme.css', import.meta.url))

// Aiur's static `effect-native-theme.css` hand-mirrors @effect-native/tokens'
// khalaTheme.color (openagents#8813 Lane A) since Aiur is a plain Tailwind
// app and never mounts the Effect Native DOM renderer that would otherwise
// lower these `--en-color-*` vars at runtime. This test is the parity guard
// against silent drift between the two.
describe('effect-native-theme.css parity with khalaTheme.color', () => {
  const css = readFileSync(cssPath, 'utf8')

  const cases: ReadonlyArray<readonly [string, keyof typeof khalaTheme.color]> = [
    ['--en-color-background', 'background'],
    ['--en-color-surface', 'surface'],
    ['--en-color-surfaceRaised', 'surfaceRaised'],
    ['--en-color-border', 'border'],
    ['--en-color-borderStrong', 'borderStrong'],
    ['--en-color-accent', 'accent'],
    ['--en-color-accentHover', 'accentHover'],
    ['--en-color-info', 'info'],
    ['--en-color-textPrimary', 'textPrimary'],
    ['--en-color-textMuted', 'textMuted'],
    ['--en-color-textFaint', 'textFaint'],
    ['--en-color-success', 'success'],
    ['--en-color-warning', 'warning'],
    ['--en-color-danger', 'danger'],
  ]

  test.each(cases)('%s matches khalaTheme.color.%s', (cssVar, colorKey) => {
    const value = khalaTheme.color[colorKey]
    expect(css).toContain(`${cssVar}: ${value};`)
  })
})
