import { describe, expect, test } from 'vitest'

import { buildSandboxRentalReceipt } from './sandbox-compute-receipt'
import { type Sandbox } from './sandbox-compute-service-routes'

describe('buildSandboxRentalReceipt', () => {
  test('projects a dereferenceable sandbox rental receipt that binds surface and ledger refs', () => {
    const sandbox: Sandbox = {
      sandboxId: 'sbx_test_123',
      accountRef: 'agent:tester',
      image: 'oa-sandbox-base',
      ttlSeconds: 900,
      status: 'ready',
      connectionRef: 'session:ref',
      createdAt: '2026-06-20T00:00:00.000Z',
      expiresAtHint: null,
    }

    const usage = { wallSeconds: 300, cpuSeconds: 150 }
    const receipt = buildSandboxRentalReceipt(sandbox, usage, 5000, true)

    expect(receipt.schemaVersion).toBe('openagents.cloud.sandbox_compute.rental_receipt.v1')
    expect(receipt.sandboxId).toBe('sbx_test_123')
    expect(receipt.accountRef).toBe('agent:tester')
    expect(receipt.image).toBe('oa-sandbox-base')
    expect(receipt.ttlSeconds).toBe(900)
    expect(receipt.usage).toEqual(usage)
    expect(receipt.chargeMsat).toBe(5000)
    expect(receipt.billed).toBe(true)

    // Validates the reconciliation of the two disparate receipt refs
    expect(receipt.receiptRef).toBe('receipt.cloud.sandbox_compute.rental.sbx_test_123')
    expect(receipt.ledgerReceiptRef).toBe('receipt.cloud.sandbox_compute.rental.charge.sbx_test_123')
  })
})
