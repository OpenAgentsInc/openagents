import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, test } from 'vitest'

import { CrmBatchApprovalPanel } from './crm-batch-console'

describe('CrmBatchApprovalPanel (initial render smoke)', () => {
  test('renders the shell, invariant note, and action controls without throwing', () => {
    const html = renderToStaticMarkup(<CrmBatchApprovalPanel />)

    expect(html).toContain('CRM draft batch approval')
    expect(html).toContain('no_send_without_approval_receipt')
    expect(html).toContain('data-testid="crm-batch-approve"')
    expect(html).toContain('data-testid="crm-batch-reload"')
    expect(html).toContain('data-testid="crm-batch-select-all"')
    expect(html).toContain('Approve selected')
  })
})
