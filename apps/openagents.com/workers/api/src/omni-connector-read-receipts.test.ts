import { describe, expect, test } from 'vitest'

import {
  OMNI_CONNECTOR_READ_RECEIPT_FIXTURE,
  OmniConnectorReadReceiptRecord,
  OmniConnectorReadReceiptUnsafe,
  omniConnectorReadReceiptProjectionHasPrivateMaterial,
  projectOmniConnectorReadReceipt,
} from './omni-connector-read-receipts'

describe('OmniConnectorReadReceiptRecord', () => {
  const nowIso = '2026-06-21T05:00:00.000Z'

  const record = (
    overrides?: Partial<OmniConnectorReadReceiptRecord>,
  ): OmniConnectorReadReceiptRecord => ({
    ...OMNI_CONNECTOR_READ_RECEIPT_FIXTURE,
    ...overrides,
  })

  test('projects safely for public audience', () => {
    const projection = projectOmniConnectorReadReceipt(record(), 'public', nowIso)

    expect(projection.agentRef).toBe('redacted')
    expect(projection.connectorRef).toBe('redacted')
    expect(projection.workroomRef).toBe('redacted')
    expect(projection.evidenceRefs).toEqual(['evidence.connector_read.summary'])
    expect(omniConnectorReadReceiptProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('projects safely for team audience', () => {
    const projection = projectOmniConnectorReadReceipt(record(), 'team', nowIso)

    expect(projection.agentRef).toBe('agent.workroom_assistant')
    expect(projection.connectorRef).toBe('connector.github_pull_requests')
    expect(projection.workroomRef).toBe('workroom.acme_delivery')
    expect(projection.evidenceRefs).toEqual(['evidence.connector_read.summary'])
    expect(omniConnectorReadReceiptProjectionHasPrivateMaterial(projection)).toBe(false)
  })

  test('blocks unsafe refs with raw timestamp', () => {
    try {
      projectOmniConnectorReadReceipt(
        record({ evidenceRefs: ['evidence.connector.read_2026-06-20T05:00:00'] }),
        'public',
        nowIso,
      )
      expect.unreachable('Should have thrown')
    } catch (e: any) {
      expect(e).toBeInstanceOf(OmniConnectorReadReceiptUnsafe)
      expect(e.reason).toContain('raw timestamp')
    }
  })

  test('blocks unsafe refs with raw provider material', () => {
    try {
      projectOmniConnectorReadReceipt(
        record({ sourcePayloadRef: 'raw_provider_payload_json' }),
        'public',
        nowIso,
      )
      expect.unreachable('Should have thrown')
    } catch (e: any) {
      expect(e).toBeInstanceOf(OmniConnectorReadReceiptUnsafe)
      expect(e.reason).toContain('provider, connector payload')
    }
  })
})
