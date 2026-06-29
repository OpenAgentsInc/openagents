import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_DOMAIN_AGENT_PACKAGE_READ_ONLY_AUTHORITY,
  OmniDomainAgentPackageProjection,
  OmniDomainAgentPackageRecord,
  OmniDomainAgentPackageUnsafe,
  exampleOmniDomainAgentPackage,
  omniDomainAgentPackageProjectionHasPrivateMaterial,
  projectOmniDomainAgentPackage,
} from './omni-domain-agent-packages'

const nowIso = '2026-06-06T22:30:00.000Z'

const packageRecord = (
  overrides: Partial<OmniDomainAgentPackageRecord> = {},
): OmniDomainAgentPackageRecord =>
  S.decodeUnknownSync(OmniDomainAgentPackageRecord)({
    ...exampleOmniDomainAgentPackage(),
    ...overrides,
  })

describe('Omni domain agent packages', () => {
  test('projects domain package lifecycle with fixtures, review, enablement, promotion, attribution, and no mutation authority', () => {
    const projection = projectOmniDomainAgentPackage(
      exampleOmniDomainAgentPackage(),
      'operator',
      nowIso,
    )

    expect(S.decodeUnknownSync(OmniDomainAgentPackageProjection)(projection))
      .toEqual(projection)
    expect(projection).toMatchObject({
      audience: 'operator',
      createdAtDisplay: '30 minutes ago',
      domainKind: 'site_builder',
      fixtureExecutionAllowed: false,
      fixtureValidated: true,
      marketplaceAttributionRecorded: true,
      marketplaceListingMutationAllowed: false,
      orgEnablementMutationAllowed: false,
      orgPrivateEnabled: true,
      paymentMutationAllowed: false,
      publicProjectionMutationAllowed: false,
      publicProjectionReady: true,
      reviewMutationAllowed: false,
      reviewRecorded: true,
      rollbackMutationAllowed: false,
      rollbackPosture: 'rollback_ready',
      runtimePromotionAllowed: false,
      runtimePromotionRequested: true,
      runtimePromoted: true,
      state: 'marketplace_attributed',
      stateLabel: 'Marketplace attributed',
      updatedAtDisplay: '5 minutes ago',
    })
    expect(projection.authority).toEqual(
      OMNI_DOMAIN_AGENT_PACKAGE_READ_ONLY_AUTHORITY,
    )
    expect(projection.fixtureRecords[0]).toMatchObject({
      scoreBps: 9600,
      state: 'passed',
    })
    expect(projection.promotionRecords[0]).toMatchObject({
      rollbackPosture: 'rollback_ready',
      state: 'promoted',
    })
    expect(JSON.stringify(projection)).not.toContain('2026-06-06T')
    expect(omniDomainAgentPackageProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps lifecycle transitions separated through validation, review, enablement, public projection, promotion, and attribution', () => {
    const validationOnly = packageRecord({
      attributionRecords: [],
      enablementRecords: [],
      promotionRecords: [],
      publicProjectionRefs: [],
      reviewRecords: [],
      state: 'fixture_validated',
    })
    const reviewOnly = packageRecord({
      attributionRecords: [],
      enablementRecords: [],
      promotionRecords: [],
      publicProjectionRefs: [],
      state: 'review_recorded',
    })
    const publicProjectionOnly = packageRecord({
      attributionRecords: [],
      promotionRecords: [],
      state: 'public_projection_ready',
    })
    const promotionRequested = packageRecord({
      attributionRecords: [],
      promotionRecords: [
        {
          ...exampleOmniDomainAgentPackage().promotionRecords[0]!,
          approvalReceiptRefs: [],
          rollbackPosture: 'rollback_required',
          rollbackRefs: [],
          runtimeReceiptRefs: [],
          state: 'requested',
        },
      ],
      state: 'runtime_promotion_requested',
    })

    expect(projectOmniDomainAgentPackage(
      validationOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      fixtureValidated: true,
      orgPrivateEnabled: false,
      publicProjectionReady: false,
      reviewRecorded: false,
      runtimePromoted: false,
    })
    expect(projectOmniDomainAgentPackage(reviewOnly, 'operator', nowIso))
      .toMatchObject({
        fixtureValidated: true,
        orgPrivateEnabled: false,
        publicProjectionReady: false,
        reviewRecorded: true,
        runtimePromotionRequested: false,
      })
    expect(projectOmniDomainAgentPackage(
      publicProjectionOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      marketplaceAttributionRecorded: false,
      publicProjectionReady: true,
      runtimePromotionRequested: false,
      runtimePromoted: false,
    })
    expect(projectOmniDomainAgentPackage(
      promotionRequested,
      'operator',
      nowIso,
    )).toMatchObject({
      rollbackPosture: 'rollback_required',
      runtimePromotionAllowed: false,
      runtimePromotionRequested: true,
      runtimePromoted: false,
    })
  })

  test('requires lifecycle gates, rollback posture, and attribution receipts before later states', () => {
    for (const badRecord of [
      packageRecord({ fixtureRecords: [], state: 'fixture_validated' }),
      packageRecord({ reviewRecords: [], state: 'review_recorded' }),
      packageRecord({ enablementRecords: [], state: 'org_private_enabled' }),
      packageRecord({
        publicProjectionRefs: [],
        state: 'public_projection_ready',
      }),
      packageRecord({ promotionRecords: [], state: 'runtime_promotion_requested' }),
      packageRecord({
        promotionRecords: [
          {
            ...exampleOmniDomainAgentPackage().promotionRecords[0]!,
            rollbackPosture: 'rollback_required',
          },
        ],
        state: 'runtime_promoted',
      }),
      packageRecord({ attributionRecords: [], state: 'marketplace_attributed' }),
      packageRecord({ blockerRefs: [], state: 'blocked' }),
    ]) {
      expect(() =>
        projectOmniDomainAgentPackage(badRecord, 'operator', nowIso),
      ).toThrow(OmniDomainAgentPackageUnsafe)
    }
  })

  test('redacts private package source, review, enablement, promotion, attribution, receipts, and provider refs publicly', () => {
    const projection = projectOmniDomainAgentPackage(
      packageRecord({
        attributionRecords: [
          {
            ...exampleOmniDomainAgentPackage().attributionRecords[0]!,
            attributionRef: 'attribution.private.operator_record',
            receiptRefs: [
              'receipt.public.attribution_recorded',
              'receipt.private.operator_receipt',
            ],
          },
        ],
        displayNameRef: 'title.private.operator_package',
        enablementRecords: [
          {
            ...exampleOmniDomainAgentPackage().enablementRecords[0]!,
            enablementRef: 'enablement.private.operator_enablement',
            receiptRefs: [
              'receipt.public.org_private_enabled',
              'receipt.private.operator_receipt',
            ],
          },
          exampleOmniDomainAgentPackage().enablementRecords[1]!,
        ],
        packageRef: 'package.private.operator_package',
        promotionRecords: [
          {
            ...exampleOmniDomainAgentPackage().promotionRecords[0]!,
            promotionRef: 'promotion.private.operator_promotion',
            runtimeReceiptRefs: [
              'receipt.public.runtime_promotion',
              'receipt.private.operator_runtime',
            ],
          },
        ],
        reviewRecords: [
          {
            ...exampleOmniDomainAgentPackage().reviewRecords[0]!,
            receiptRefs: [
              'receipt.public.operator_review',
              'receipt.private.operator_receipt',
            ],
            reviewRef: 'review.private.operator_review',
          },
        ],
        sourceRefs: [
          'source.public.package_manifest',
          'source.private.operator_notes',
        ],
      }),
      'public',
      nowIso,
    )

    const serialized = JSON.stringify(projection)

    expect(projection.displayNameRef).toBe('title.redacted')
    expect(projection.packageRef).toBe('package.redacted')
    expect(projection.sourceRefs).toEqual([])
    expect(projection.reviewRecords).toEqual([])
    expect(projection.enablementRecords.map(record => record.enablementRef))
      .toEqual(['enablement.public.public_projection_site_builder'])
    expect(projection.promotionRecords).toEqual([])
    expect(projection.attributionRecords).toEqual([])
    expect(serialized).not.toMatch(
      /(attribution|enablement|package|promotion|receipt|review|source|title)\.private/,
    )
    expect(omniDomainAgentPackageProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('rejects mutable authority, unsafe refs, private customer data, provider credentials, payment or wallet material, and raw timestamps', () => {
    for (const badInput of [
      () =>
        projectOmniDomainAgentPackage(
          packageRecord({
            authority: {
              ...OMNI_DOMAIN_AGENT_PACKAGE_READ_ONLY_AUTHORITY,
              noRuntimePromotionMutation: false,
            },
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniDomainAgentPackage(
          packageRecord({ sourceRefs: ['package_source_private.repo'] }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniDomainAgentPackage(
          packageRecord({ sourceRefs: ['provider_credential.oauth'] }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniDomainAgentPackage(
          packageRecord({ sourceRefs: ['customer_email.redacted'] }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniDomainAgentPackage(
          packageRecord({
            attributionRecords: [
              {
                ...exampleOmniDomainAgentPackage().attributionRecords[0]!,
                receiptRefs: ['receipt.public.payment_hash_abcd'],
              },
            ],
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniDomainAgentPackage(
          packageRecord({
            promotionRecords: [
              {
                ...exampleOmniDomainAgentPackage().promotionRecords[0]!,
                rollbackRefs: ['wallet.secret.rollback'],
              },
            ],
          }),
          'operator',
          nowIso,
        ),
      () =>
        projectOmniDomainAgentPackage(
          packageRecord({ sourceRefs: ['source.public.2026-06-06T22:25:00.000Z'] }),
          'operator',
          nowIso,
        ),
    ]) {
      expect(badInput).toThrow(OmniDomainAgentPackageUnsafe)
    }
  })
})
