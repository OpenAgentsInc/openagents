import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  REACTOR_DATA_LIBERATION_ADAPTERS,
  REACTOR_DATA_LIBERATION_FIXTURE_REPORTS,
  REACTOR_DATA_LIBERATION_SYNTHETIC_EXPORTS,
  ReactorDataLiberationAdapterConfig,
  ReactorDataLiberationPipelineReport,
  runReactorDataLiberationPipeline,
} from '@openagentsinc/reactor-contracts'

describe('Reactor data liberation pipeline fixtures', () => {
  test('keeps adapter configs synthetic and customer-data disallowed', () => {
    const generic = S.decodeUnknownSync(ReactorDataLiberationAdapterConfig)(
      REACTOR_DATA_LIBERATION_ADAPTERS.genericCsvApiSaas,
    )
    const salesforce = S.decodeUnknownSync(ReactorDataLiberationAdapterConfig)(
      REACTOR_DATA_LIBERATION_ADAPTERS.salesforceContactExport,
    )

    expect(generic).toMatchObject({
      adapterKind: 'generic_csv_api_saas_export',
      customerDataAllowed: false,
      fixtureTruth: 'synthetic_public_fixture',
    })
    expect(salesforce).toMatchObject({
      adapterKind: 'salesforce_contact_export',
      customerDataAllowed: false,
      fixtureTruth: 'synthetic_public_fixture',
    })
  })

  test('verifies a passing generic CSV/API export without logging raw rows', () => {
    const report = S.decodeUnknownSync(ReactorDataLiberationPipelineReport)(
      REACTOR_DATA_LIBERATION_FIXTURE_REPORTS.genericCsvApiSaasPassed,
    )

    expect(report.status).toBe('passed')
    expect(report.sourceRowCount).toBe(2)
    expect(report.loadedRowCount).toBe(2)
    expect(report.failedRowCount).toBe(0)
    expect(report.customerDataLogged).toBe(false)
    expect(report.customerEngagementAuthorized).toBe(false)
    expect(report.packageCopyAuthorized).toBe(false)
    expect(report.verificationReceipts[0]?.sourceChecksum).toMatch(/^fnv1a32:/)
    expect(report.verificationReceipts[0]?.loadedChecksum).toMatch(/^fnv1a32:/)
    expect(JSON.stringify(report)).not.toContain('ALICE.EXAMPLE')
    expect(JSON.stringify(report)).not.toContain('bob.example@example.test')
  })

  test('reports a partial Salesforce-style export instead of dropping failed rows', () => {
    const report = S.decodeUnknownSync(ReactorDataLiberationPipelineReport)(
      REACTOR_DATA_LIBERATION_FIXTURE_REPORTS.salesforceContactPartial,
    )

    expect(report.status).toBe('partial')
    expect(report.sourceRowCount).toBe(3)
    expect(report.loadedRowCount).toBe(2)
    expect(report.failedRowCount).toBe(1)
    expect(report.verificationReceipts[0]?.failedRowRefs).toEqual([
      'row.synthetic.salesforce.contact.003',
    ])
    expect(report.sourceRowCount).toBe(
      report.loadedRowCount + report.failedRowCount + report.partialRowCount,
    )
    expect(report.blockerRefs).toEqual(
      expect.arrayContaining([
        'blocker.reactor.data_liberation.pipeline_partial',
        'reason.reactor.data_liberation.missing_required_field:Email',
      ]),
    )
  })

  test('same runner handles renamed columns through config', () => {
    const baseMapping =
      REACTOR_DATA_LIBERATION_ADAPTERS.genericCsvApiSaas.recordClassMappings[0]!
    const report = runReactorDataLiberationPipeline({
      adapter: {
        ...REACTOR_DATA_LIBERATION_ADAPTERS.genericCsvApiSaas,
        adapterRef:
          'reactor.data_liberation.adapter.generic_csv_api_saas.worker_renamed_fixture',
        recordClassMappings: [
          {
            ...baseMapping,
            fieldMappings: [
              {
                mappingRef:
                  'reactor.data_liberation.mapping.worker_renamed_contact.identifier',
                required: true,
                sourceField: 'legacy_identifier',
                targetField: 'external_id',
                transformRefs: ['trim'],
              },
              {
                mappingRef:
                  'reactor.data_liberation.mapping.worker_renamed_contact.email',
                required: true,
                sourceField: 'mailbox',
                targetField: 'email',
                transformRefs: ['trim', 'lowercase'],
              },
            ],
            schemaMappingRef:
              'reactor.data_liberation.schema_mapping.worker_renamed_contact_to_open_crm.v1',
          },
        ],
      },
      customerControlledStoreRef:
        'customer_controlled_store.synthetic.open_crm.worker_renamed_columns',
      exportRows: [
        {
          fields: {
            legacy_identifier: ' worker-renamed-001 ',
            mailbox: ' WORKER-RENAMED@example.test ',
          },
          rowRef: 'row.synthetic.worker_renamed.contact.001',
          sourceRecordTypeRef: 'generic_saas.contact',
        },
      ],
      generatedAt: '2026-07-04T16:20:00.000Z',
      reportRef: 'reactor.data_liberation.report.worker_renamed_columns.001',
    })

    expect(report.status).toBe('passed')
    expect(report.verificationReceipts[0]?.schemaMappingRef).toBe(
      'reactor.data_liberation.schema_mapping.worker_renamed_contact_to_open_crm.v1',
    )
  })

  test('source fixtures are synthetic while reports stay raw-value-free', () => {
    const sourceRows = JSON.stringify(REACTOR_DATA_LIBERATION_SYNTHETIC_EXPORTS)
    const reports = JSON.stringify(REACTOR_DATA_LIBERATION_FIXTURE_REPORTS)

    expect(sourceRows).toContain('example.test')
    expect(sourceRows).not.toMatch(/@(gmail|yahoo|outlook|hotmail)\./i)
    expect(reports).not.toContain('example.test')
  })
})
