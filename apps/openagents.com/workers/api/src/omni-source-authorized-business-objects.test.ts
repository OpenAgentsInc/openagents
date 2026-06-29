import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OMNI_BUSINESS_OBJECT_WRITE_FIXTURE,
  OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE,
  OMNI_SOURCE_AUTHORITY_CONTRACT_ONLY,
  OmniBusinessObjectWriteProjection,
  OmniBusinessObjectWriteRecord,
  OmniSourceAuthorityBinding,
  OmniSourceAuthorityBindingProjection,
  OmniSourceAuthorityUnsafe,
  decideOmniBusinessObjectWrite,
  omniSourceAuthorityIsContractOnly,
  omniSourceAuthorityProjectionHasPrivateMaterial,
  projectOmniBusinessObjectWrite,
  projectOmniSourceAuthorityBinding,
} from './omni-source-authorized-business-objects'

const nowIso = '2026-06-19T05:30:00.000Z'

const binding = (
  overrides: Partial<OmniSourceAuthorityBinding> = {},
): OmniSourceAuthorityBinding =>
  S.decodeUnknownSync(OmniSourceAuthorityBinding)({
    ...OMNI_SOURCE_AUTHORITY_BINDING_FIXTURE,
    ...overrides,
  })

const write = (
  overrides: Partial<OmniBusinessObjectWriteRecord> = {},
): OmniBusinessObjectWriteRecord =>
  S.decodeUnknownSync(OmniBusinessObjectWriteRecord)({
    ...OMNI_BUSINESS_OBJECT_WRITE_FIXTURE,
    ...overrides,
  })

describe('Omni source-authorized business objects', () => {
  test('fixtures carry the contract-only authority boundary', () => {
    expect(
      omniSourceAuthorityIsContractOnly(OMNI_SOURCE_AUTHORITY_CONTRACT_ONLY),
    ).toBe(true)
    expect(
      omniSourceAuthorityIsContractOnly({
        ...OMNI_SOURCE_AUTHORITY_CONTRACT_ONLY,
        noSpendAuthority: false,
      }),
    ).toBe(false)
  })

  test('projects a source-authority binding with friendly time labels', () => {
    const projection = projectOmniSourceAuthorityBinding(
      binding(),
      'operator',
      nowIso,
    )

    expect(
      S.decodeUnknownSync(OmniSourceAuthorityBindingProjection)(projection),
    ).toEqual(projection)
    expect(projection.createdAtDisplay).toBe('30 minutes ago')
    expect(JSON.stringify(projection)).not.toContain('2026-06-19T')
    expect(omniSourceAuthorityProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('public binding projection redacts principal and workroom refs', () => {
    const projection = projectOmniSourceAuthorityBinding(
      binding(),
      'public',
      nowIso,
    )

    expect(projection.principalRef).toBe('redacted')
    expect(projection.workroomRef).toBe('redacted')
    expect(omniSourceAuthorityProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('projects a closed business-object write without granting mutation authority', () => {
    const record = write()
    const projection = projectOmniBusinessObjectWrite(record, 'operator', nowIso)

    expect(
      S.decodeUnknownSync(OmniBusinessObjectWriteProjection)(projection),
    ).toEqual(projection)
    expect(omniSourceAuthorityIsContractOnly(record.authority)).toBe(true)
    expect(projection.mutationApplied).toBe(true)
    expect(projection.approvalRecorded).toBe(true)
    expect(projection.closeoutReady).toBe(true)
    expect(projection.connectorReadReceiptRefs).toEqual([])
    expect(omniSourceAuthorityProjectionHasPrivateMaterial(projection)).toBe(
      false,
    )
  })

  test('public write projection redacts source, approval, and object refs', () => {
    const projection = projectOmniBusinessObjectWrite(write(), 'public', nowIso)

    expect(projection.sourceRefs).toEqual([])
    expect(projection.approvalRefs).toEqual([])
    expect(projection.proposedChangeRefs).toEqual([])
    expect(projection.bindingRef).toBe('redacted')
    expect(projection.businessObjectRef).toBe('redacted')
    expect(projection.operatorDiagnosticRefs).toEqual([])
    expect(projection.connectorReadReceiptRefs).toEqual([])
  })

  // -------------------------------------------------------------------------
  // Approval-gated write-decision engine
  // -------------------------------------------------------------------------

  test('allows an approved write that the binding fully authorizes', () => {
    const decision = decideOmniBusinessObjectWrite(binding(), write())

    expect(decision.applyAllowed).toBe(true)
    expect(decision.approvalRequired).toBe(true)
    expect(decision.blockerRefs).toEqual([])
    expect(decision.reasonRef).toBe(
      'reason.source_authority.approved_write_applyable',
    )
  })

  test('blocks an update write that has no recorded approval', () => {
    const decision = decideOmniBusinessObjectWrite(
      binding(),
      write({ state: 'proposed' }),
    )

    expect(decision.applyAllowed).toBe(false)
    expect(decision.approvalRequired).toBe(true)
    expect(decision.blockerRefs).toContain(
      'blocker.source_authority.approval_required',
    )
  })

  test('blocks a write whose object kind is not in the binding', () => {
    const decision = decideOmniBusinessObjectWrite(
      binding({ businessObjectKinds: ['company'] }),
      write(),
    )

    expect(decision.applyAllowed).toBe(false)
    expect(decision.blockerRefs).toContain(
      'blocker.source_authority.object_kind_not_authorized',
    )
  })

  test('blocks a write whose operation is not in the binding', () => {
    const decision = decideOmniBusinessObjectWrite(
      binding({ allowedOperations: ['append'] }),
      write(),
    )

    expect(decision.blockerRefs).toContain(
      'blocker.source_authority.operation_not_authorized',
    )
  })

  test('blocks a write whose source kind is not in the binding', () => {
    const decision = decideOmniBusinessObjectWrite(
      binding({ allowedSourceKinds: ['connector_read'] }),
      write(),
    )

    expect(decision.blockerRefs).toContain(
      'blocker.source_authority.source_kind_not_authorized',
    )
  })

test('decideOmniBusinessObjectWrite allows connector read write with connector read receipt refs', () => {
    const validBinding = binding({ allowedSourceKinds: ['connector_read'] });
    const validWrite = write({
      sourceKind: 'connector_read',
      connectorReadReceiptRefs: ['receipt.connector_read.123'],
      bindingRef: validBinding.id,
      principalKind: validBinding.principalKind,
      principalRef: validBinding.principalRef,
    });

    const decision = decideOmniBusinessObjectWrite(validBinding, validWrite);
    expect(decision.blockerRefs).not.toContain('blocker.source_authority.operation_not_authorized');
    expect(decision.blockerRefs).not.toContain('blocker.source_authority.source_kind_not_authorized');
  })

    test('blocks a write whose principal does not match the binding', () => {
    const decision = decideOmniBusinessObjectWrite(
      binding(),
      write({ principalRef: 'principal.someone_else' }),
    )

    expect(decision.blockerRefs).toContain(
      'blocker.source_authority.principal_mismatch',
    )
  })

  test('blocks a write whose binding ref or workroom does not match', () => {
    const wrongBinding = decideOmniBusinessObjectWrite(
      binding(),
      write({ bindingRef: 'source_authority_binding.other' }),
    )
    expect(wrongBinding.blockerRefs).toContain(
      'blocker.source_authority.binding_ref_mismatch',
    )

    const wrongWorkroom = decideOmniBusinessObjectWrite(
      binding(),
      write({ workroomRef: 'workroom.other' }),
    )
    expect(wrongWorkroom.blockerRefs).toContain(
      'blocker.source_authority.workroom_mismatch',
    )
  })

  test('waives approval only for low-risk append/create on append-only kinds', () => {
    const decision = decideOmniBusinessObjectWrite(
      binding({
        businessObjectKinds: ['artifact'],
        allowedOperations: ['create'],
        requiresApproval: false,
      }),
      write({
        approvalRefs: [],
        businessObjectKind: 'artifact',
        businessObjectRef: 'business_object.artifact.deliverable',
        operation: 'create',
        state: 'applied',
      }),
    )

    expect(decision.approvalRequired).toBe(false)
    expect(decision.applyAllowed).toBe(true)
    expect(decision.reasonRef).toBe(
      'reason.source_authority.low_risk_write_applyable',
    )
  })

  test('still requires approval for an update even when the binding waives it', () => {
    const decision = decideOmniBusinessObjectWrite(
      binding({ requiresApproval: false }),
      write({ approvalRefs: [], state: 'proposed' }),
    )

    expect(decision.approvalRequired).toBe(true)
    expect(decision.applyAllowed).toBe(false)
    expect(decision.blockerRefs).toContain(
      'blocker.source_authority.approval_required',
    )
  })

  // -------------------------------------------------------------------------
  // Safety / redaction guards
  // -------------------------------------------------------------------------

  test('rejects a write with no source refs (chat-text-only inference)', () => {
    expect(() =>
      projectOmniBusinessObjectWrite(
        write({ sourceRefs: [] }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniSourceAuthorityUnsafe)
  })

  test('rejects a binding that is not contract-only', () => {
    expect(() =>
      projectOmniSourceAuthorityBinding(
        binding({
          authority: {
            ...OMNI_SOURCE_AUTHORITY_CONTRACT_ONLY,
            noBusinessObjectMutationWithoutApproval: false,
          },
        }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniSourceAuthorityUnsafe)
  })

  test('rejects refs that carry raw private material', () => {
    expect(() =>
      projectOmniBusinessObjectWrite(
        write({ sourceRefs: ['contact_email.alice@example.com'] }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniSourceAuthorityUnsafe)
  })

  test('blocked write requires blocker refs', () => {
    expect(() =>
      projectOmniBusinessObjectWrite(
        write({ blockerRefs: [], state: 'blocked' }),
        'operator',
        nowIso,
      ),
    ).toThrow(OmniSourceAuthorityUnsafe)
  })

test('allows connector read write with connector read receipt refs', () => {
    const validWrite = write({
      sourceKind: 'connector_read',
      connectorReadReceiptRefs: ['receipt.connector_read.123'],
    });

    // Test projection succeeds (meaning assertRecordSafe passes)
    const projected = projectOmniBusinessObjectWrite(validWrite, 'operator', nowIso);
    expect(projected.connectorReadReceiptRefs).toEqual(['receipt.connector_read.123']);
  })

    test('blocks connector read without connector read receipt refs', () => {
    try {
      projectOmniBusinessObjectWrite(
        write({
          sourceKind: 'connector_read',
          connectorReadReceiptRefs: [],
        }),
        'operator',
        nowIso
      )
      expect.unreachable()
    } catch (e: any) {
      expect(e.reason).toBe('Connector read writes require connector read receipt refs.')
    }
  })
})
