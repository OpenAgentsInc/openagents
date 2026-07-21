import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { aisdkDocsSourceDefinitions } from '../aisdk/aisdk-content'
import {
  aisdkDocsManifest,
  loadAisdkDocsPage,
} from '../aisdk/generated/aisdk-manifest.generated'
import { AisdkDocsNotFound, AisdkDocsPageView } from './-aisdk-docs-page'

describe('Start /aisdk/docs content', () => {
  test('the generated manifest matches the declared source allowlist exactly', () => {
    expect(aisdkDocsManifest.map(entry => entry.slug)).toEqual(
      aisdkDocsSourceDefinitions.map(definition => definition.slug),
    )
  })

  test('the overview page renders docs/ai-sdk/README.md', () => {
    const page = loadAisdkDocsPage('')
    expect(page).toBeDefined()
    const html = renderToStaticMarkup(<AisdkDocsPageView page={page!} />)

    expect(html).toContain('data-route="aisdk-docs"')
    expect(html).toContain('The OpenAgents AI SDK')
    expect(html).toContain('OpenAgentsInc/ai')
    expect(html).toContain('href="/aisdk"')
  })

  test('the getting-started page renders the three verified examples', () => {
    const page = loadAisdkDocsPage('getting-started')
    expect(page).toBeDefined()
    const html = renderToStaticMarkup(<AisdkDocsPageView page={page!} />)

    expect(html).toContain('Getting started')
    expect(html).toContain('@openagentsinc/ai@rc')
    expect(html).toContain('makeReferenceAdapter')
    expect(html).toContain('suspendTurn')
    expect(html).toContain('continueTurn')
    expect(html).toContain('khalaEventToUiChunks')
    expect(html).toContain('applyUiChunk')
    expect(html).toContain('buildHistoryCorpus')
    expect(html).toContain('recallTierD')
  })

  test('the packages page renders every published package', () => {
    const page = loadAisdkDocsPage('packages')
    expect(page).toBeDefined()
    const html = renderToStaticMarkup(<AisdkDocsPageView page={page!} />)

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
      expect(html).toContain(name)
    }
  })

  test('an unknown slug loads nothing so the route 404s', () => {
    expect(loadAisdkDocsPage('does-not-exist')).toBeUndefined()
    expect(loadAisdkDocsPage('README')).toBeUndefined()
  })

  test('every page navigation links each declared document', () => {
    const page = loadAisdkDocsPage('')
    const html = renderToStaticMarkup(<AisdkDocsPageView page={page!} />)

    expect(html).toContain('href="/aisdk/docs"')
    expect(html).toContain('href="/aisdk/docs/getting-started"')
    expect(html).toContain('href="/aisdk/docs/packages"')
  })

  test('the not-found view routes the reader back to the docs overview', () => {
    const html = renderToStaticMarkup(<AisdkDocsNotFound />)

    expect(html).toContain('data-route="aisdk-docs-not-found"')
    expect(html).toContain('Page not found')
    expect(html).toContain('href="/aisdk/docs"')
  })

  test('generated HTML carries no raw script or iframe payloads', () => {
    for (const entry of aisdkDocsManifest) {
      const page = loadAisdkDocsPage(entry.slug)
      expect(page).toBeDefined()
      expect(page!.html).not.toMatch(/<script\b/i)
      expect(page!.html).not.toMatch(/<iframe\b/i)
    }
  })
})
