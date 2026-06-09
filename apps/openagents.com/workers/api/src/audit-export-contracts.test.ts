import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OpenAgentsAuditExportBundle,
  OpenAgentsAuditExportBundleProjection,
  OpenAgentsAuditExportItem,
  OpenAgentsAuditExportRequest,
  OpenAgentsAuditExportUnsafe,
  auditExportClassificationRequiresOperatorReview,
  buildOpenAgentsAuditExportBundle,
  openAgentsAuditExportProjectionHasPrivateMaterial,
  projectOpenAgentsAuditExportBundle,
} from './audit-export-contracts'
import {
  OmniDataPolicyEnvelope,
} from './omni-data-classification'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-06T23:45:00.000Z'

const dataPolicy = (
  overrides: Partial<OmniDataPolicyEnvelope> = {},
): OmniDataPolicyEnvelope =>
  S.decodeUnknownSync(OmniDataPolicyEnvelope)({
    classificationCaveatRef: 'classification.caveat.reviewed_safe',
    dataClassification: 'public',
    evidenceRefs: ['evidence.public_safe'],
    exportPolicyRefs: ['export.public_safe'],
    providerEligibilityRefs: [],
    redactionPolicyRefs: [],
    retentionPolicyRefs: ['retention.standard_fulfillment'],
    subjectRef: 'site.public_otec',
    surface: 'site',
    trustTier: 'reviewed',
    ...overrides,
  })

const request = (
  overrides: Partial<OpenAgentsAuditExportRequest> = {},
): OpenAgentsAuditExportRequest =>
  S.decodeUnknownSync(OpenAgentsAuditExportRequest)({
    approvedByRef: 'approved_by.operator.review',
    audience: 'operator',
    caveatRefs: ['caveat.audit_export.evidence_only'],
    createdAtIso: '2026-06-06T23:40:00.000Z',
    exportPolicyRefs: ['export.operator_safe.fulfillment_audit'],
    generatedAtIso: nowIso,
    id: 'audit_export.otec.fulfillment_1',
    requestedScopeRefs: ['scope.order', 'scope.site_revision'],
    requestedScopes: ['order', 'site_revision'],
    requesterRef: 'requested_by.operator.chris',
    retentionPolicyRefs: ['retention.standard_fulfillment'],
    ...overrides,
  })

const item = (
  overrides: Partial<OpenAgentsAuditExportItem> = {},
): OpenAgentsAuditExportItem =>
  S.decodeUnknownSync(OpenAgentsAuditExportItem)({
    caveatRefs: ['caveat.item.public_safe'],
    createdAtIso: '2026-06-06T23:42:00.000Z',
    dataPolicy: dataPolicy(),
    evidenceRefs: ['evidence.site_revision.reviewed'],
    exportPolicyRefs: ['export.public_safe'],
    itemRef: 'site_revision.otec.v3',
    receiptRefs: ['receipt.site_revision.v3'],
    retentionPolicyRefs: ['retention.standard_fulfillment'],
    scope: 'site_revision',
    sourceRefs: ['source.public_brief'],
    ...overrides,
  })

describe('OpenAgents audit export contracts', () => {
  test('builds valid operator bundles with included, omitted, and denied items', () => {
    const bundle = buildOpenAgentsAuditExportBundle(request(), [
      item(),
      item({
        dataPolicy: dataPolicy({
          dataClassification: 'customer',
          evidenceRefs: ['evidence.order.customer_safe'],
          exportPolicyRefs: ['export.customer_safe'],
          subjectRef: 'order.customer_summary',
          surface: 'order',
        }),
        itemRef: 'order.otec.request_summary',
        scope: 'order',
      }),
      item({
        dataPolicy: dataPolicy({
          dataClassification: 'payment_private',
          evidenceRefs: ['evidence.payment.redacted_summary'],
          exportPolicyRefs: [],
          subjectRef: 'payment_ref.otec.redacted',
          surface: 'payment_ref',
        }),
        exportPolicyRefs: [],
        itemRef: 'billing_payment.otec.redacted',
        scope: 'billing_payment',
      }),
      item({
        dataPolicy: dataPolicy({
          dataClassification: 'deletion_retention_sensitive',
          exportPolicyRefs: ['export.operator_safe.deleted_subject_summary'],
          retentionPolicyRefs: ['retention.delete_requested'],
          subjectRef: 'customer_asset.deleted_subject',
          surface: 'customer_asset',
        }),
        itemRef: 'artifact.deleted_subject_summary',
        retentionPolicyRefs: ['retention.delete_requested'],
        scope: 'artifact',
      }),
    ])
    const projection = projectOpenAgentsAuditExportBundle(bundle, nowIso)

    expect(S.decodeUnknownSync(OpenAgentsAuditExportBundle)(bundle)).toEqual(
      bundle,
    )
    expect(S.decodeUnknownSync(OpenAgentsAuditExportBundleProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.itemCount).toBe(2)
    expect(projection.omittedItemCount).toBe(1)
    expect(projection.deniedItemCount).toBe(1)
    expect(projection.generatedAtDisplay).toBe('Just now')
    expect(projection.createdAtDisplay).toBe('5 minutes ago')
    expect(projection.denialRefs).toContain(
      'denial.audit_export.classification_not_exportable',
    )
    expect(openAgentsAuditExportProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('hides operator refs and omits customer-only items from public exports', () => {
    const bundle = buildOpenAgentsAuditExportBundle(
      request({
        approvedByRef: 'approved_by.operator.review',
        audience: 'public',
        requesterRef: 'requested_by.operator.chris',
      }),
      [
        item(),
        item({
          dataPolicy: dataPolicy({
            dataClassification: 'customer',
            evidenceRefs: ['evidence.order.customer_safe'],
            redactionPolicyRefs: ['redaction.order.public_summary'],
            subjectRef: 'order.customer_summary',
            surface: 'order',
          }),
          itemRef: 'order.customer_safe_summary',
          scope: 'order',
        }),
      ],
    )
    const projection = projectOpenAgentsAuditExportBundle(bundle, nowIso)

    expect(projection.requesterRef).toBeNull()
    expect(projection.approvedByRef).toBeNull()
    expect(projection.itemCount).toBe(1)
    expect(projection.omittedItemCount).toBe(1)
    expect(projection.includedItemRefs).toEqual(['site_revision.otec.v3'])
    expect(openAgentsAuditExportProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('includes payment-private evidence only with explicit operator-safe export policy', () => {
    const bundle = buildOpenAgentsAuditExportBundle(request(), [
      item({
        dataPolicy: dataPolicy({
          dataClassification: 'payment_private',
          evidenceRefs: ['evidence.payment.redacted_summary'],
          exportPolicyRefs: ['export.operator_safe.redacted_payment_summary'],
          subjectRef: 'payment_ref.otec.redacted',
          surface: 'payment_ref',
        }),
        exportPolicyRefs: ['export.operator_safe.redacted_payment_summary'],
        itemRef: 'billing_payment.otec.redacted_summary',
        scope: 'billing_payment',
      }),
    ])
    const projection = projectOpenAgentsAuditExportBundle(bundle, nowIso)

    expect(projection.itemCount).toBe(1)
    expect(projection.omittedItemCount).toBe(0)
    expect(projection.deniedItemCount).toBe(0)
    expect(projection.includedItems[0]?.dataPolicy.dataClassification).toBe(
      'payment_private',
    )
  })

  test('rejects unsafe refs and never projects raw timestamps', () => {
    for (const fixture of OPENAGENTS_UNSAFE_REDACTION_FIXTURES) {
      expect(() =>
        buildOpenAgentsAuditExportBundle(
          request({ requestedScopeRefs: [fixture.value] }),
          [item()],
        ),
      ).toThrow(OpenAgentsAuditExportUnsafe)
      expect(() =>
        buildOpenAgentsAuditExportBundle(request(), [
          item({ sourceRefs: [fixture.value] }),
        ]),
      ).toThrow(OpenAgentsAuditExportUnsafe)
    }

    const projection = projectOpenAgentsAuditExportBundle(
      buildOpenAgentsAuditExportBundle(request(), [item()]),
      nowIso,
    )

    expect(openAgentsSerializedValueContainsUnsafeFixture(projection))
      .toBe(false)
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
  })

  test('marks sensitive classifications as operator-review candidates', () => {
    expect(auditExportClassificationRequiresOperatorReview('payment_private'))
      .toBe(true)
    expect(auditExportClassificationRequiresOperatorReview('secret_bearing'))
      .toBe(true)
    expect(auditExportClassificationRequiresOperatorReview('public')).toBe(
      false,
    )
  })
})
