import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { isKnownStartDocumentPath } from '../route-table'
import { AisdkPage } from './-aisdk-page'

describe('Start /aisdk route', () => {
  test('server-renders the route contract, product sentence, and install command', () => {
    const html = renderToStaticMarkup(<AisdkPage />)

    expect(html).toContain('data-route="aisdk"')
    expect(html).toContain('The OpenAgents AI SDK')
    expect(html).toContain('Effect-native toolkit')
    expect(html).toContain('durable, cursor-exact streams')
    expect(html).toContain('pnpm add @openagentsinc/ai@rc')
  })

  test('renders the L0..L6 layer diagram with the one rule', () => {
    const html = renderToStaticMarkup(<AisdkPage />)

    for (const layerId of ['L0', 'L1', 'L2', 'L3', 'L4', 'L5', 'L6']) {
      expect(html).toContain(`>${layerId}<`)
    }
    expect(html).toContain('KhalaRuntimeEvent')
    expect(html).toContain('One event union. One durable cursor.')
  })

  test('renders the roster with npm links and the GitHub repository link', () => {
    const html = renderToStaticMarkup(<AisdkPage />)

    for (const name of [
      '@openagentsinc/ai',
      '@openagentsinc/agent-runtime-schema',
      '@openagentsinc/agent-harness-contract',
      '@openagentsinc/ai-model',
      '@openagentsinc/history-corpus',
      '@openagentsinc/rlm',
      '@openagentsinc/ai-sdk-sandbox-local',
      '@openagentsinc/ai-sdk-sandbox-openagents',
    ]) {
      expect(html).toContain(`href="https://www.npmjs.com/package/${name}"`)
    }
    expect(html).toContain('href="https://github.com/OpenAgentsInc/ai"')
  })

  test('renders the six differentiators', () => {
    const html = renderToStaticMarkup(<AisdkPage />)

    for (const title of [
      'Durable cursor-exact streams',
      'Suspend and continue that persists',
      'Coding-agent harnesses',
      'Redaction as a schema field',
      'Recall instead of compaction',
      'Honest failure vocabulary',
    ]) {
      expect(html).toContain(title)
    }
  })

  test('renders a quickstart using published umbrella symbols and links the docs', () => {
    const html = renderToStaticMarkup(<AisdkPage />)

    expect(html).toContain('makeReferenceAdapter')
    expect(html).toContain('khalaEventToUiChunks')
    expect(html).toContain('applyUiChunk')
    expect(html).toContain('href="/aisdk/docs"')
    expect(html).toContain('href="/aisdk/docs/getting-started"')
    expect(html).toContain('href="/aisdk/docs/packages"')
  })

  test('the shared route table owns /aisdk and /aisdk/docs and rejects strangers', () => {
    expect(isKnownStartDocumentPath('/aisdk')).toBe(true)
    expect(isKnownStartDocumentPath('/aisdk/docs')).toBe(true)
    expect(isKnownStartDocumentPath('/aisdk/docs/getting-started')).toBe(true)
    expect(isKnownStartDocumentPath('/aisdk/docs/packages')).toBe(true)
    expect(isKnownStartDocumentPath('/aisdk/docs/a/b')).toBe(false)
    expect(isKnownStartDocumentPath('/aisdk/other')).toBe(false)
  })
})
