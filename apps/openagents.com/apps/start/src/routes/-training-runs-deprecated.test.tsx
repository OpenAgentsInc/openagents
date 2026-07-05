import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { TrainingRunsDeprecatedPage } from './-training-runs-deprecated-page'

describe('Start /training/runs deprecated notice', () => {
  test('renders the temporarily-unavailable notice instead of the real page', () => {
    const html = renderToStaticMarkup(<TrainingRunsDeprecatedPage />)

    expect(html).toContain('data-route="training-runs-deprecated"')
    expect(html).toContain('This page is temporarily unavailable')
    expect(html).toContain('Temporarily unavailable')
    expect(html).not.toContain(
      'No Worker-authoritative training runs are recorded yet.',
    )
  })
})
