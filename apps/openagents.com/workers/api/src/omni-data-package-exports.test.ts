import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_DATA_PACKAGE_EXPORT_READ_ONLY_AUTHORITY,
  OmniDataPackageExportProjection,
  OmniDataPackageExportRecord,
  OmniDataPackageExportUnsafe,
  exampleOmniDataPackageExport,
  projectOmniDataPackageExport,
} from './omni-data-package-exports'

const nowIso = '2026-06-06T22:30:00.000Z'

const packageRecord = (
  overrides: Partial<OmniDataPackageExportRecord> = {},
): OmniDataPackageExportRecord =>
  S.decodeUnknownSync(OmniDataPackageExportRecord)({
    ...exampleOmniDataPackageExport(),
    ...overrides,
  })

describe('Omni data package exports', () => {
  test('projects published packages with manifests, digests, receipts, and no mutation authority', () => {
    const projection = projectOmniDataPackageExport(
      exampleOmniDataPackageExport(),
      'public',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniDataPackageExportProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      downloadMutationAllowed: false,
      fileHostingMutationAllowed: false,
      liveWalletSpendAllowed: false,
      publicClaimUpgradeAllowed: false,
      publishedClaimAllowed: true,
      readyForSharing: true,
      receiptMutationAllowed: false,
      rightsMutationAllowed: false,
      state: 'published',
      stateLabel: 'Published',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_DATA_PACKAGE_EXPORT_READ_ONLY_AUTHORITY,
    )
    expect(projection.artifactDigests[0]).toMatchObject({
      artifactKind: 'dataset',
      digestAlgorithm: 'sha256',
      sizeBytes: 120048,
    })
    expect(projection.provenance).toMatchObject({
      sourceBundleRefs: ['bundle.public.otec_research_sources'],
      sourceRefs: ['source.public.openagents_transcript_230'],
      spanRefs: ['span.public.transcript_230_intro'],
    })
    expect(projection.rights).toMatchObject({
      rightsState: 'allowed',
      rightsPolicyRef: 'rights.public.web_citation_allowed',
    })
    expect(projection.redaction).toMatchObject({
      redactionState: 'redacted',
      removedFieldRefs: ['field.public.customer_notes_removed'],
      retainedFieldRefs: ['field.public.source_refs'],
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
  })

  test('preserves package-ready, reviewed, published, and revoked state boundaries', () => {
    const ready = projectOmniDataPackageExport(
      packageRecord({
        receiptRefs: [],
        reviewStateRef: null,
        state: 'package_ready',
      }),
      'team',
      nowIso,
    )
    const reviewed = projectOmniDataPackageExport(
      packageRecord({
        receiptRefs: [],
        state: 'reviewed',
      }),
      'team',
      nowIso,
    )
    const published = projectOmniDataPackageExport(
      exampleOmniDataPackageExport(),
      'team',
      nowIso,
    )
    const revoked = projectOmniDataPackageExport(
      packageRecord({
        receiptRefs: [],
        redaction: {
          ...exampleOmniDataPackageExport().redaction,
          blockedReasonRefs: ['reason.public.rights_revoked'],
          redactionState: 'blocked',
        },
        rights: {
          ...exampleOmniDataPackageExport().rights,
          rightsState: 'revoked',
        },
        state: 'revoked',
      }),
      'team',
      nowIso,
    )

    expect(ready.stateLabel).toBe('Package ready')
    expect(ready.readyForSharing).toBe(true)
    expect(ready.publishedClaimAllowed).toBe(false)
    expect(reviewed.stateLabel).toBe('Reviewed')
    expect(reviewed.readyForSharing).toBe(true)
    expect(reviewed.publishedClaimAllowed).toBe(false)
    expect(published.publishedClaimAllowed).toBe(true)
    expect(revoked.stateLabel).toBe('Revoked')
    expect(revoked.readyForSharing).toBe(false)
    expect(revoked.publishedClaimAllowed).toBe(false)
  })

  test('redacts private package, schema, rights, artifact, receipt, provenance, and redaction refs publicly', () => {
    const projection = projectOmniDataPackageExport(
      packageRecord({
        artifactDigests: [
          ...exampleOmniDataPackageExport().artifactDigests,
          {
            artifactKind: 'file',
            artifactRef: 'artifact.private.operator_file',
            digestAlgorithm: 'sha256',
            digestRef: 'digest.private.operator_file',
            sizeBytes: 800,
          },
        ],
        packageRef: 'package.private.operator_export',
        provenance: {
          ...exampleOmniDataPackageExport().provenance,
          reviewRefs: [
            'review.public.operator_approved',
            'review.private.operator_notes',
          ],
          sourceRefs: [
            'source.public.openagents_transcript_230',
            'source.private.operator_source',
          ],
          spanRefs: [
            'span.public.transcript_230_intro',
            'span.private.operator_span',
          ],
        },
        receiptRefs: [
          'receipt.public.otec_research_export',
          'receipt.private.operator_receipt',
        ],
        redaction: {
          ...exampleOmniDataPackageExport().redaction,
          redactionPolicyRefs: [
            'policy.public.redacted_archive_only',
            'policy.private.operator_redaction',
          ],
          reviewerRefs: [
            'reviewer.public.operator_review',
            'reviewer.private.operator_identity',
          ],
        },
        rights: {
          ...exampleOmniDataPackageExport().rights,
          allowedAudienceRefs: [
            'audience.public.investor_review',
            'audience.private.operator_only',
          ],
          licenseRefs: [
            'license.public.openagents_docs',
            'license.private.operator_terms',
          ],
        },
        schema: {
          ...exampleOmniDataPackageExport().schema,
          fieldRefs: [
            'field.public.source_ref',
            'field.private.operator_field',
          ],
          schemaRef: 'schema.private.operator_schema',
        },
        titleRef: 'title.private.operator_export',
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.packageRef).toBe('package.redacted')
    expect(projection.titleRef).toBe('title.redacted')
    expect(projection.schema.schemaRef).toBe('schema.redacted')
    expect(projection.artifactDigests).toHaveLength(1)
    expect(projection.receiptRefs).toEqual(['receipt.public.otec_research_export'])
    expect(projection.rights.licenseRefs).toEqual([
      'license.public.openagents_docs',
    ])
    expect(projection.provenance.sourceRefs).toEqual([
      'source.public.openagents_transcript_230',
    ])
    expect(serialized).not.toMatch(
      /(artifact|audience|digest|field|license|package|policy|receipt|review|rights|schema|source|span|title)\.private/,
    )
  })

  test('requires artifact digests, schema fields, rights policy, redaction, provenance, receipts for publish, and review refs for review', () => {
    for (const record of [
      packageRecord({ artifactDigests: [] }),
      packageRecord({
        schema: { ...exampleOmniDataPackageExport().schema, fieldRefs: [] },
      }),
      packageRecord({
        rights: { ...exampleOmniDataPackageExport().rights, licenseRefs: [] },
      }),
      packageRecord({
        rights: {
          ...exampleOmniDataPackageExport().rights,
          usageCaveatRefs: [],
        },
      }),
      packageRecord({
        redaction: {
          ...exampleOmniDataPackageExport().redaction,
          redactionPolicyRefs: [],
        },
      }),
      packageRecord({
        provenance: {
          ...exampleOmniDataPackageExport().provenance,
          sourceBundleRefs: [],
        },
      }),
      packageRecord({
        receiptRefs: [],
        state: 'published',
      }),
      packageRecord({
        provenance: {
          ...exampleOmniDataPackageExport().provenance,
          reviewRefs: [],
        },
        state: 'reviewed',
      }),
      packageRecord({
        reviewStateRef: null,
        state: 'reviewed',
      }),
      packageRecord({
        rights: {
          ...exampleOmniDataPackageExport().rights,
          rightsState: 'revoked',
        },
        state: 'published',
      }),
      packageRecord({
        redaction: {
          ...exampleOmniDataPackageExport().redaction,
          blockedReasonRefs: [],
        },
        rights: {
          ...exampleOmniDataPackageExport().rights,
          rightsState: 'allowed',
        },
        state: 'revoked',
      }),
    ]) {
      expect(() =>
        projectOmniDataPackageExport(record, 'operator', nowIso),
      ).toThrow(OmniDataPackageExportUnsafe)
    }
  })

  test('rejects false authority, invalid digest sizes, raw source archives, private repo, payment, wallet, provider, and timestamp material', () => {
    for (const record of [
      packageRecord({
        authority: {
          ...OMNI_DATA_PACKAGE_EXPORT_READ_ONLY_AUTHORITY,
          noRightsMutation: false,
        },
      }),
      packageRecord({
        artifactDigests: [
          {
            ...exampleOmniDataPackageExport().artifactDigests[0]!,
            sizeBytes: -1,
          },
        ],
      }),
      packageRecord({
        artifactDigests: [
          {
            ...exampleOmniDataPackageExport().artifactDigests[0]!,
            artifactRef: 'raw_archive.operator_source',
          },
        ],
      }),
      packageRecord({
        provenance: {
          ...exampleOmniDataPackageExport().provenance,
          sourceRefs: ['github.com/team/private'],
        },
      }),
      packageRecord({ receiptRefs: ['payment_preimage.secret'] }),
      packageRecord({
        rights: {
          ...exampleOmniDataPackageExport().rights,
          rightsPolicyRef: 'wallet.secret.material',
        },
      }),
      packageRecord({
        caveatRefs: ['caveat.public.2026-06-06T22:00:00Z'],
      }),
      packageRecord({
        provenance: {
          ...exampleOmniDataPackageExport().provenance,
          generationRefs: ['provider_payload.raw_record'],
        },
      }),
    ]) {
      expect(() =>
        projectOmniDataPackageExport(record, 'operator', nowIso),
      ).toThrow(OmniDataPackageExportUnsafe)
    }
  })
})
