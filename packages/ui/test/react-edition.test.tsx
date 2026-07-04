import { describe, expect, test } from 'bun:test'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { renderToStaticMarkup } from 'react-dom/server'

import {
  Button,
  ReactEditionSmokeFixture,
  TextField,
  openAgentsNativeWindTokens,
  openAgentsReactTailwindTokens,
} from '../src/react'

const reactCssPath = fileURLToPath(new URL('../src/react.css', import.meta.url))

describe('@openagentsinc/ui React edition', () => {
  test('renders the Storybook-less fixture with the required component families', () => {
    const html = renderToStaticMarkup(<ReactEditionSmokeFixture />)

    expect(html).toContain('oa-react-ui-root')
    expect(html).toContain('Turn agent work into visible proof.')
    expect(html).toContain('aria-label="Primary"')
    expect(html).toContain('Worker-safe')
    expect(html).toContain('name="workspace"')
    expect(html).toContain('src/routes/index.tsx')
    expect(html).toContain('bg-oa-accent')
  })

  test('keeps the component surface dark-only without Tailwind dark/light variants', () => {
    const css = readFileSync(reactCssPath, 'utf8')
    const html = renderToStaticMarkup(<ReactEditionSmokeFixture />)

    expect(css).toContain("@import 'tailwindcss';")
    expect(css).toContain('@theme inline')
    expect(css).toContain('color-scheme: dark;')
    expect(`${css}\n${html}`).not.toContain('dark:')
    expect(`${css}\n${html}`).not.toContain('light:')
  })

  test('exposes literal NativeWind tokens from the same StarCraft palette', () => {
    expect(openAgentsNativeWindTokens.colors.bg).toBe('#000')
    expect(openAgentsNativeWindTokens.colors.accent).toBe('#4fd0ff')
    expect(openAgentsNativeWindTokens.colors.border).toBe('#1d2a44')
    expect(openAgentsNativeWindTokens.colors.text).toBe('#f1efe8')
    expect(openAgentsNativeWindTokens.borderRadius.xl).toBe('8px')
    expect(JSON.stringify(openAgentsNativeWindTokens)).not.toContain('var(')
  })

  test('documents the Tailwind 4 token names consumed by React components', () => {
    expect(openAgentsReactTailwindTokens.colors.bg).toBe('var(--oa-color-bg)')
    expect(openAgentsReactTailwindTokens.colors.accent).toBe(
      'var(--oa-color-khala-energy-cyan)',
    )
    expect(openAgentsReactTailwindTokens.fontFamily.mono).toBe('var(--oa-font-mono)')
  })

  test('defaults buttons to type=button and connects field labels to inputs', () => {
    const buttonHtml = renderToStaticMarkup(<Button>Open</Button>)
    const fieldHtml = renderToStaticMarkup(
      <TextField id="team" label="Team" name="team" />,
    )

    expect(buttonHtml).toContain('type="button"')
    expect(fieldHtml).toContain('for="team"')
    expect(fieldHtml).toContain('id="team"')
    expect(fieldHtml).toContain('name="team"')
  })
})
