import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type BlueprintActionSubmission,
  blueprintActionSubmissionCanExecute,
  blueprintActionSubmissionHasDryRun,
  blueprintActionSubmissionIsApprovalGated,
  blueprintActionSubmissionIsTerminal,
  BlueprintActionSubmission as BlueprintActionSubmissionSchema,
} from './action-submission'

const proposedSubmission: BlueprintActionSubmission = {
  actionKind: 'deploy',
  approvalPolicyRef: 'policy.blueprint.action_submission.proposals_only.v1',
  approvalReceiptRef: null,
  approvalState: 'not_requested',
  approvedByRef: null,
  contentRedacted: true,
  contextPackRefs: ['context_pack.deploy_1'],
  createdAt: '2026-06-05T00:00:00.000Z',
  directExecution: false,
  directProgramRunExecutionAllowed: false,
  dryRunReceiptRef: null,
  dryRunRequired: true,
  evidenceRefs: ['evidence.deploy_summary_1'],
  executionReceiptRef: null,
  failureRef: null,
  id: 'blueprint_action_submission_1',
  idempotencyKey: 'action-submission:deploy:1',
  modelConfidenceBypassDisabled: true,
  programRunAuthorityBoundary: 'evidence_only',
  proposalOnly: true,
  proposedByProgramRunId: 'blueprint_program_run_1',
  proposedEffectRef: 'effect.deploy_site_revision_2',
  receiptRefs: [],
  sourceAuthorityRefs: ['source_authority.site_revision_2'],
  status: 'proposed',
  summaryRef: 'summary.deploy_site_revision_2',
  toolRefs: ['tool.action_submission.propose'],
  updatedAt: '2026-06-05T00:00:00.000Z',
}

const approvedSubmission: BlueprintActionSubmission = {
  ...proposedSubmission,
  approvalReceiptRef: 'receipt.operator_approval_1',
  approvalState: 'approved',
  approvedByRef: 'operator.chris',
  dryRunReceiptRef: 'receipt.deploy_dry_run_1',
  receiptRefs: ['receipt.deploy_dry_run_1', 'receipt.operator_approval_1'],
  status: 'approved',
}

describe('Blueprint Action Submission schema', () => {
  test('decodes proposed submissions without execution authority', () => {
    expect(
      S.decodeUnknownSync(BlueprintActionSubmissionSchema)(proposedSubmission),
    ).toEqual(proposedSubmission)
    expect(blueprintActionSubmissionHasDryRun(proposedSubmission)).toBe(false)
    expect(blueprintActionSubmissionCanExecute(proposedSubmission)).toBe(false)
    expect(blueprintActionSubmissionIsApprovalGated(proposedSubmission)).toBe(
      false,
    )
  })

  test('allows execution only after dry run and approval receipts exist', () => {
    expect(
      S.decodeUnknownSync(BlueprintActionSubmissionSchema)(approvedSubmission),
    ).toEqual(approvedSubmission)
    expect(blueprintActionSubmissionHasDryRun(approvedSubmission)).toBe(true)
    expect(blueprintActionSubmissionIsApprovalGated(approvedSubmission)).toBe(
      true,
    )
    expect(blueprintActionSubmissionCanExecute(approvedSubmission)).toBe(true)
  })

  test('does not allow terminal, failed, or already-executed submissions to execute', () => {
    expect(
      blueprintActionSubmissionCanExecute({
        ...approvedSubmission,
        executionReceiptRef: 'receipt.deploy_execution_1',
        status: 'receipt_recorded',
      }),
    ).toBe(false)
    expect(
      blueprintActionSubmissionIsTerminal({
        ...approvedSubmission,
        executionReceiptRef: 'receipt.deploy_execution_1',
        status: 'receipt_recorded',
      }),
    ).toBe(true)
    expect(
      blueprintActionSubmissionCanExecute({
        ...approvedSubmission,
        failureRef: 'failure.deploy_failed_1',
        status: 'failed',
      }),
    ).toBe(false)
  })
})
