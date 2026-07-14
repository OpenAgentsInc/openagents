import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { OpsConsole } from './ops-console'

describe('OpsConsole (initial render smoke)', () => {
  test('renders the non-monetary operations panel shells without throwing', () => {
    const html = renderToStaticMarkup(<OpsConsole />)

    expect(html).toContain('Health strip')
    expect(html).toContain('CRM draft batch approval')
    expect(html).toContain('Recent org-cloud runs')
    expect(html).toContain('Daily sales ledger')
    expect(html).toContain('data-testid="ops-runs-list"')
    expect(html).toContain('data-testid="crm-batch-approve"')
  })
})
