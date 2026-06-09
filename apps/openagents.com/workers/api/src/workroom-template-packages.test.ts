import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE,
  WORKROOM_TEMPLATE_PACKAGE_REVIEW_ONLY_AUTHORITY,
  WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE,
  WorkroomTemplatePackageProjection,
  WorkroomTemplatePackageRecord,
  WorkroomTemplatePackageUnsafe,
  WorkroomTemplatePackageVersionProjection,
  projectWorkroomTemplatePackage,
  projectWorkroomTemplatePackageVersion,
  workroomTemplatePackageAuthorityIsReviewOnly,
  workroomTemplatePackageProjectionHasPrivateMaterial,
} from './workroom-template-packages'
import {
  OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
  openAgentsSerializedValueContainsUnsafeFixture,
} from './redaction-regression-fixtures'

const nowIso = '2026-06-07T09:40:00.000Z'

const packageRecord = (
  overrides: Partial<WorkroomTemplatePackageRecord> = {},
): WorkroomTemplatePackageRecord =>
  S.decodeUnknownSync(WorkroomTemplatePackageRecord)({
    ...WORKROOM_TEMPLATE_PACKAGE_RECORD_FIXTURE,
    ...overrides,
  })

describe('workroom template package model', () => {
  test('projects template package versions with friendly time labels', () => {
    const projection = projectWorkroomTemplatePackageVersion(
      WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE,
      'agent',
      nowIso,
    )

    expect(S.decodeUnknownSync(WorkroomTemplatePackageVersionProjection)(
      projection,
    )).toEqual(projection)
    expect(projection.sourceRefs).toEqual([])
    expect(projection.createdAtDisplay).toBe('40 minutes ago')
    expect(projection.updatedAtDisplay).toBe('35 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-07T')
    expect(workroomTemplatePackageProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('projects package records without runtime, marketplace, deploy, runner, or payment authority', () => {
    const record = packageRecord()
    const projection = projectWorkroomTemplatePackage(record, 'operator', nowIso)

    expect(S.decodeUnknownSync(WorkroomTemplatePackageProjection)(projection))
      .toEqual(projection)
    expect(workroomTemplatePackageAuthorityIsReviewOnly(record.authority))
      .toBe(true)
    expect(projection.validationRecorded).toBe(true)
    expect(projection.reviewRecorded).toBe(true)
    expect(projection.orgPrivateEnablementRecorded).toBe(true)
    expect(projection.publicProjectionReady).toBe(true)
    expect(projection.runtimePromotionRequested).toBe(true)
    expect(projection.runtimePromotionAllowed).toBe(false)
    expect(projection.marketplaceListingAllowed).toBe(false)
    expect(projection.externalRunnerLaunchAllowed).toBe(false)
    expect(projection.deploymentAllowed).toBe(false)
    expect(projection.paymentMutationAllowed).toBe(false)
    expect(projection.createdAtDisplay).toBe('30 minutes ago')
    expect(projection.updatedAtDisplay).toBe('5 minutes ago')
    expect(workroomTemplatePackageProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('hides source, review, promotion, enablement, and diagnostics from public projection', () => {
    const projection = projectWorkroomTemplatePackage(
      packageRecord(),
      'public',
      nowIso,
    )

    expect(projection.sourceRefs).toEqual([])
    expect(projection.reviewRefs).toEqual([])
    expect(projection.promotionRefs).toEqual([])
    expect(projection.orgPrivateEnablementRefs).toEqual([])
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.runtimePromotionAllowed).toBe(false)
    expect(openAgentsSerializedValueContainsUnsafeFixture(projection)).toBe(
      false,
    )
    expect(workroomTemplatePackageProjectionHasPrivateMaterial(projection))
      .toBe(false)
  })

  test('keeps validation, review, private enablement, public projection, and promotion request states separate', () => {
    const validationOnly = packageRecord({
      orgPrivateEnablementRefs: [],
      promotionRefs: [],
      publicProjectionRefs: [],
      reviewRefs: [],
      state: 'validation_recorded',
    })
    const reviewOnly = packageRecord({
      orgPrivateEnablementRefs: [],
      promotionRefs: [],
      publicProjectionRefs: [],
      state: 'review_recorded',
    })
    const publicProjectionOnly = packageRecord({
      promotionRefs: [],
      state: 'public_projection_ready',
    })

    expect(projectWorkroomTemplatePackage(
      validationOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      orgPrivateEnablementRecorded: false,
      publicProjectionReady: false,
      reviewRecorded: false,
      runtimePromotionRequested: false,
      validationRecorded: true,
    })
    expect(projectWorkroomTemplatePackage(reviewOnly, 'operator', nowIso))
      .toMatchObject({
        orgPrivateEnablementRecorded: false,
        publicProjectionReady: false,
        reviewRecorded: true,
        runtimePromotionRequested: false,
      })
    expect(projectWorkroomTemplatePackage(
      publicProjectionOnly,
      'operator',
      nowIso,
    )).toMatchObject({
      publicProjectionReady: true,
      runtimePromotionAllowed: false,
      runtimePromotionRequested: false,
    })
  })

  test('rejects mutable authority and blocked records without blockers', () => {
    expect(() =>
      projectWorkroomTemplatePackage(
        packageRecord({
          authority: {
            ...WORKROOM_TEMPLATE_PACKAGE_REVIEW_ONLY_AUTHORITY,
            noRuntimePromotion: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(WorkroomTemplatePackageUnsafe)
    expect(() =>
      projectWorkroomTemplatePackage(
        packageRecord({
          blockerRefs: [],
          state: 'blocked',
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(WorkroomTemplatePackageUnsafe)
  })

  test('rejects private package source, raw prompts, provider payloads, secrets, and timestamps', () => {
    for (const fixture of [
      ...OPENAGENTS_UNSAFE_REDACTION_FIXTURES,
      { label: 'private package source', value: 'package_source_private.repo' },
      { label: 'raw prompt', value: 'raw_prompt.system' },
      { label: 'provider payload', value: 'provider_payload.raw' },
      { label: 'raw package', value: 'raw_package.bundle' },
    ]) {
      expect(() =>
        projectWorkroomTemplatePackage(
          packageRecord({ sourceRefs: [fixture.value] }),
          'operator',
          nowIso,
        ),
      ).toThrow(WorkroomTemplatePackageUnsafe)
    }
  })
})
