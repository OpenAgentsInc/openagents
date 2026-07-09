import { viewStructure } from '@effect-native/render-dom'
import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import {
  TASSADAR_AGENT_INSTRUCTIONS,
  TassadarEffectNativePage,
  initialTassadarLandingState,
  tassadarLandingView,
} from './-tassadar-effect-native-page'

describe('EN-4 /tassadar Effect Native route', () => {
  test('server render is only a thin mount shim, not landing-content React', () => {
    const html = renderToStaticMarkup(<TassadarEffectNativePage />)

    expect(html).toContain('data-route="tassadar"')
    expect(html).toContain('data-tassadar-effect-native-root=""')
    expect(html).not.toContain('LLM-computer idea')
  })

  test('authored content is a typed Effect Native tree with the copy intent wired', () => {
    const tree = tassadarLandingView(initialTassadarLandingState)
    const structure = viewStructure(tree)
    const serialized = JSON.stringify(tree)

    expect(structure).toMatchObject({
      tag: 'Stack',
      key: 'tassadar-root',
    })
    expect(serialized).toContain('"catalogVersion":"effect-native/v19"')
    expect(serialized).toContain('Copy Agent Instructions')
    expect(serialized).toContain('TassadarCopyAgentInstructions')
    expect(serialized).toContain(
      'Read https://openagents.com/AGENTS.md and join the OpenAgents Tassadar training run.',
    )
    expect(serialized).toContain('pylon training claim --base-url https://openagents.com')
    expect(serialized).not.toContain('className')
  })

  test('copied state flips the button label', () => {
    const tree = tassadarLandingView({ copied: true })
    const serialized = JSON.stringify(tree)

    expect(serialized).toContain('"label":"Copied"')
    expect(serialized).not.toContain('"label":"Copy Agent Instructions"')
  })

  test('agent instructions constant matches the original interim-React copy verbatim', () => {
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain(
      'curl -X POST https://openagents.com/api/agents/register',
    )
    expect(TASSADAR_AGENT_INSTRUCTIONS).toContain('npx @openagentsinc/pylon')
  })

  test('source boundary uses Effect Native packages instead of direct DOM/JSX content authoring', () => {
    const source = readFileSync(
      join(process.cwd(), 'src/routes/-tassadar-effect-native-page.tsx'),
      'utf8',
    )

    expect(source).toContain("from '@effect-native/core'")
    expect(source).toContain("from '@effect-native/render-dom'")
    expect(source).not.toContain('lucide-react')
  })
})
