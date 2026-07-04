import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { ArtanisTracesPage } from './-artanis-traces-page'

describe('Start Artanis RLM trace tree route', () => {
  test('renders the FRLM conductor tree with Blueprint governance refs', () => {
    const html = renderToStaticMarkup(<ArtanisTracesPage />)

    expect(html).toContain('data-route="artanis-traces"')
    expect(html).toContain('Artanis execution tree')
    expect(html).toContain('FrlmConductor')
    expect(html).toContain('SubQuery.Submit')
    expect(html).toContain('SubQuery.Return')
    expect(html).toContain('Run.Done')
    expect(html).toContain('program_signature.frlm_conductor.v1')
    expect(html).toContain('program_signature.rlm_leaf_executor.v1')
    expect(html).toContain('/api/operator/rlm/traces')
    expect(html).toContain('No direct execution authority')
  })

  test('keeps private trace material out of the public visualizer', () => {
    const html = renderToStaticMarkup(<ArtanisTracesPage />)

    expect(html).not.toMatch(
      /raw_prompt|raw_trace|rawEvents|trajectory_json|bearer|api[_-]?key|sk-[a-z0-9]/i,
    )
  })
})
