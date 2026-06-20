import { describe, expect, it } from 'vitest'

import {
  buildOmniConnectorReadReceipt,
  projectOmniConnectorReadReceipt,
  validateOmniConnectorReadReceipt,
} from './omni-connector-read-receipt'

describe('Omni Connector Read Receipts', () => {
  const nowIso = '2026-06-20T12:00:00.000Z'

  it('builds a canonical verifiable read receipt', () => {
    const receipt = buildOmniConnectorReadReceipt({
      connectorKind: 'linear',
      connectorRef: 'connector.linear.acme_corp',
      id: 'receipt.connector_read.123',
      queryRef: 'query.issues.needs_triage',
      readAtIso: '2026-06-20T10:00:00.000Z',
      recordCount: 15,
      workroomRef: 'workroom.acme_delivery',
    })

    expect(receipt.line).toBe(
      'Connector linear (connector.linear.acme_corp) read 15 records for query query.issues.needs_triage into workroom workroom.acme_delivery at 2026-06-20T10:00:00.000Z.',
    )
    expect(validateOmniConnectorReadReceipt(receipt)).toBe(true)
  })

  it('fails validation if tampered', () => {
    const receipt = buildOmniConnectorReadReceipt({
      connectorKind: 'hubspot',
      connectorRef: 'connector.hubspot.sales',
      id: 'receipt.connector_read.456',
      queryRef: 'query.contacts.recent',
      readAtIso: '2026-06-20T11:00:00.000Z',
      recordCount: 5,
      workroomRef: 'workroom.sales_ops',
    })

    expect(validateOmniConnectorReadReceipt(receipt)).toBe(true)

    // Tamper record count
    const tamperedCount = { ...receipt, recordCount: 50 } as any
    expect(validateOmniConnectorReadReceipt(tamperedCount)).toBe(false)

    // Tamper line
    const tamperedLine = { ...receipt, line: 'tampered' } as any
    expect(validateOmniConnectorReadReceipt(tamperedLine)).toBe(false)
  })

  it('projects safely based on audience', () => {
    const receipt = buildOmniConnectorReadReceipt({
      connectorKind: 'github',
      connectorRef: 'connector.github.private_repo',
      id: 'receipt.connector_read.789',
      queryRef: 'query.prs.open',
      readAtIso: nowIso,
      recordCount: 3,
      workroomRef: 'workroom.engineering',
    })

    const publicProj = projectOmniConnectorReadReceipt(receipt, 'public', nowIso)
    expect(publicProj.connectorRef).toBe('redacted')
    expect(publicProj.queryRef).toBe('redacted')
    expect(publicProj.workroomRef).toBe('redacted')
    expect(publicProj.line).toBe('redacted')

    const operatorProj = projectOmniConnectorReadReceipt(receipt, 'operator', nowIso)
    expect(operatorProj.connectorRef).toBe('connector.github.private_repo')
    expect(operatorProj.queryRef).toBe('query.prs.open')
    expect(operatorProj.line).toBe(receipt.line)
  })
})
