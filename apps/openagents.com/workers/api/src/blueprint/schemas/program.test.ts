import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BlueprintProgramSignature,
  type BlueprintProgramType,
  BlueprintProgramSignature as BlueprintProgramSignatureSchema,
  blueprintProgramSignatureSupportsFamily,
  BlueprintProgramType as BlueprintProgramTypeSchema,
  blueprintProgramTypeRequiredReceiptRefs,
  blueprintProgramTypeRequiresApproval,
} from './program'

const programTypeFixture: BlueprintProgramType = {
  allowedStrategyRefs: ['strategy.evidence_first_revision'],
  directMutationAllowed: false,
  evidenceRequirements: [
    {
      descriptionRef: 'evidence.context_pack_required',
      kind: 'context_pack_ref',
      minimumCount: 1,
      required: true,
    },
  ],
  family: 'continuation',
  id: 'program_type.autopilot_continuation',
  instructionRefs: ['instructions.autopilot_continuation'],
  instructionsVersionRef: 'instructions.autopilot_continuation.v1',
  purposeRef: 'purpose.decide_next_autopilot_step',
  receiptRequirements: [
    {
      kind: 'program_run',
      receiptRef: 'receipt.program_run',
      required: true,
    },
    {
      kind: 'action_submission',
      receiptRef: 'receipt.action_submission',
      required: false,
    },
  ],
  releaseGates: [
    {
      evidenceRefs: ['fixture.continuation_regression'],
      gateKind: 'tests_passed',
      gateRef: 'gate.continuation_regression',
      required: true,
    },
  ],
  riskClass: 'medium',
  status: 'active',
  toolScopes: [
    {
      access: 'evidence',
      allowedSurfaces: ['agent_api', 'omni_workroom'],
      requiresApproval: false,
      toolRef: 'tool.context_pack.read',
    },
    {
      access: 'propose_action',
      allowedSurfaces: ['operator_dashboard'],
      requiresApproval: true,
      toolRef: 'tool.action_submission.propose',
    },
  ],
}

const programSignatureFixture: BlueprintProgramSignature = {
  decodePolicy: {
    unknownFieldPolicy: 'reject',
    validationMode: 'strict',
    validationPolicyRef: 'policy.strict_signature_validation',
  },
  evidenceRequirements: programTypeFixture.evidenceRequirements,
  id: 'program_signature.autopilot_continuation.v1',
  inputSchema: {
    kind: 'input',
    schemaRef: 'schema.autopilot_continuation.input',
    versionRef: 'schema.autopilot_continuation.input.v1',
  },
  outputSchema: {
    kind: 'output',
    schemaRef: 'schema.autopilot_continuation.output',
    versionRef: 'schema.autopilot_continuation.output.v1',
  },
  programTypeId: 'program_type.autopilot_continuation',
  receiptRequirements: programTypeFixture.receiptRequirements,
  status: 'active',
  supportsContext: false,
  supportsContinuation: true,
  supportsProofProjection: false,
  supportsReview: false,
  supportsRouting: false,
  toolScopes: programTypeFixture.toolScopes,
  versionRef: 'program_signature.autopilot_continuation.v1',
}

describe('Blueprint Program Type and Program Signature schemas', () => {
  test('decode Program Type with instructions, policy, tool scope, receipts, and gates', () => {
    expect(
      S.decodeUnknownSync(BlueprintProgramTypeSchema)(programTypeFixture),
    ).toEqual(programTypeFixture)
    expect(blueprintProgramTypeRequiresApproval(programTypeFixture)).toBe(true)
    expect(blueprintProgramTypeRequiredReceiptRefs(programTypeFixture)).toEqual([
      'receipt.program_run',
    ])
  })

  test('decode Program Signature with input/output schema refs and decode policy', () => {
    expect(
      S.decodeUnknownSync(BlueprintProgramSignatureSchema)(
        programSignatureFixture,
      ),
    ).toEqual(programSignatureFixture)
    expect(
      blueprintProgramSignatureSupportsFamily(
        programSignatureFixture,
        'continuation',
      ),
    ).toBe(true)
    expect(
      blueprintProgramSignatureSupportsFamily(programSignatureFixture, 'routing'),
    ).toBe(false)
  })

  test('rejects unknown Program Signature status values', () => {
    expect(() =>
      S.decodeUnknownSync(BlueprintProgramSignatureSchema)({
        ...programSignatureFixture,
        status: 'published',
      }),
    ).toThrow()
  })
})
