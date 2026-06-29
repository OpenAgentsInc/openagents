import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  blueprintActionSubmissionCanExecute,
  blueprintActionSubmissionIsApprovalGated,
} from '../schemas/action-submission'
import {
  BlueprintActionSubmissionValidationError,
  blueprintActionSubmissionStudybenchEvidenceRefs,
  listBlueprintActionSubmissions,
  readBlueprintActionSubmissionById,
  recordBlueprintActionSubmissionProposal,
} from './action-submissions'

type ActionSubmissionRow = Readonly<{
  action_kind: 'create_pull_request'
  approval_policy_ref: string
  approval_receipt_ref: string | null
  approval_state: 'pending'
  approved_by_ref: string | null
  archived_at: string | null
  content_redacted: number
  context_pack_refs_json: string
  created_at: string
  direct_execution: number
  direct_program_run_execution_allowed: number
  dry_run_receipt_ref: string | null
  dry_run_required: number
  evidence_refs_json: string
  execution_receipt_ref: string | null
  failure_ref: string | null
  id: string
  idempotency_key: string
  metadata_json: string
  model_confidence_bypass_disabled: number
  program_run_authority_boundary: 'evidence_only'
  proposal_only: number
  proposed_by_program_run_id: string
  proposed_effect_ref: string
  receipt_refs_json: string
  source_authority_refs_json: string
  status: 'pending_approval'
  summary_ref: string
  tool_refs_json: string
  updated_at: string
}>

class ActionSubmissionStore {
  rows: Array<ActionSubmissionRow> = []
}

class ActionSubmissionStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ActionSubmissionStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(colName: string): Promise<T | null>
  first<T = Record<string, unknown>>(): Promise<T | null>
  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('WHERE idempotency_key = ?')) {
      const idempotencyKey = String(this.values[0])
      const row =
        this.store.rows.find(
          item =>
            item.idempotency_key === idempotencyKey &&
            item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    if (this.query.includes('WHERE id = ?')) {
      const id = String(this.values[0])
      const row =
        this.store.rows.find(
          item => item.id === id && item.archived_at === null,
        ) ?? null

      return Promise.resolve(row as T | null)
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (
      this.query.includes(
        'INSERT OR IGNORE INTO blueprint_action_submissions',
      )
    ) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.rows.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.rows.push({
          action_kind: this.values[2] as 'create_pull_request',
          approval_policy_ref: String(this.values[3]),
          approval_receipt_ref: null,
          approval_state: this.values[4] as 'pending',
          approved_by_ref: null,
          archived_at: null,
          content_redacted: Number(this.values[5]),
          context_pack_refs_json: String(this.values[6]),
          created_at: String(this.values[22]),
          direct_execution: Number(this.values[7]),
          direct_program_run_execution_allowed: Number(this.values[8]),
          dry_run_receipt_ref: null,
          dry_run_required: Number(this.values[9]),
          evidence_refs_json: String(this.values[10]),
          execution_receipt_ref: null,
          failure_ref: null,
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          metadata_json: String(this.values[21]),
          model_confidence_bypass_disabled: Number(this.values[11]),
          program_run_authority_boundary: this.values[12] as 'evidence_only',
          proposal_only: Number(this.values[13]),
          proposed_by_program_run_id: String(this.values[14]),
          proposed_effect_ref: String(this.values[15]),
          receipt_refs_json: String(this.values[16]),
          source_authority_refs_json: String(this.values[17]),
          status: this.values[18] as 'pending_approval',
          summary_ref: String(this.values[19]),
          tool_refs_json: String(this.values[20]),
          updated_at: String(this.values[23]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM blueprint_action_submissions')) {
      const limit = Number(this.values[0])
      const rows = [...this.store.rows]
        .filter(item => item.archived_at === null)
        .sort((left: ActionSubmissionRow, right: ActionSubmissionRow) =>
          right.created_at.localeCompare(left.created_at),
        )
        .slice(0, limit)

      return Promise.resolve({ results: rows } as unknown as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
    return options?.columnNames === true
      ? Promise.resolve([[]])
      : Promise.resolve([])
  }
}

const actionSubmissionsDb = (store: ActionSubmissionStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ActionSubmissionStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runtime = {
  makeActionSubmissionId: () => 'blueprint_action_submission_generated',
  nowIso: () => '2026-06-07T19:00:00.000Z',
}

const recordSubmission = (
  store: ActionSubmissionStore,
  overrides: Partial<
    Parameters<typeof recordBlueprintActionSubmissionProposal>[1]
  > = {},
) =>
  Effect.runPromise(
    recordBlueprintActionSubmissionProposal(
      actionSubmissionsDb(store),
      {
        actionKind: 'create_pull_request',
        approvalPolicyRef:
          'policy.blueprint.action_submission.proposals_only.v1',
        contextPackRefs: ['context_pack.probe.pr_1'],
        evidenceRefs: ['evidence.probe.pr_summary_1'],
        id: 'action_submission.probe.pr_1',
        idempotencyKey: 'action-submission:probe:pr:1',
        metadata: { assignmentRef: 'assignment.probe.pr_1' },
        proposedByProgramRunId: 'blueprint_program_run.probe.continue.1',
        proposedEffectRef: 'effect.probe.create_pull_request.1',
        receiptRefs: ['receipt.action_submission.probe.pr_1'],
        sourceAuthorityRefs: ['source_authority.repo.openagents.probe'],
        summaryRef: 'summary.probe.create_pull_request.1',
        toolRefs: ['tool.probe.propose_action_submission'],
        ...overrides,
      },
      runtime,
    ),
  )

describe('Blueprint Action Submission repository', () => {
  test('records idempotent pending approval proposals without execution authority', async () => {
    const store = new ActionSubmissionStore()
    const submission = await recordSubmission(store)
    const replay = await recordSubmission(store, {
      proposedEffectRef: 'effect.probe.changed',
    })

    expect(submission).toStrictEqual(replay)
    expect(store.rows).toHaveLength(1)
    expect(submission).toMatchObject({
      approvalPolicyRef: 'policy.blueprint.action_submission.proposals_only.v1',
      approvalState: 'pending',
      contentRedacted: true,
      directExecution: false,
      directProgramRunExecutionAllowed: false,
      executionReceiptRef: null,
      modelConfidenceBypassDisabled: true,
      programRunAuthorityBoundary: 'evidence_only',
      proposalOnly: true,
      receiptRefs: [
        'receipt.action_submission',
        'receipt.action_submission.probe.pr_1',
      ],
      status: 'pending_approval',
    })
    expect(blueprintActionSubmissionIsApprovalGated(submission)).toBe(true)
    expect(blueprintActionSubmissionCanExecute(submission)).toBe(false)
  })

  test('reads and lists action submissions', async () => {
    const store = new ActionSubmissionStore()
    const submission = await recordSubmission(store)
    const read = await Effect.runPromise(
      readBlueprintActionSubmissionById(
        actionSubmissionsDb(store),
        submission.id,
      ),
    )
    const listed = await Effect.runPromise(
      listBlueprintActionSubmissions(actionSubmissionsDb(store), 10),
    )

    expect(read).toStrictEqual(submission)
    expect(listed).toEqual([submission])
  })

  test('rejects proposals without evidence or with private/write material', async () => {
    const store = new ActionSubmissionStore()

    await expect(
      recordSubmission(store, {
        evidenceRefs: [],
      }),
    ).rejects.toBeInstanceOf(BlueprintActionSubmissionValidationError)

    await expect(
      recordSubmission(store, {
        evidenceRefs: ['evidence.safe'],
        proposedEffectRef: 'raw_email',
      }),
    ).rejects.toBeInstanceOf(BlueprintActionSubmissionValidationError)

    await expect(
      recordSubmission(store, {
        proposedEffectRef: 'effect.safe',
        metadata: { provider_payload: 'raw' },
      }),
    ).rejects.toBeInstanceOf(BlueprintActionSubmissionValidationError)
  })

  test('allows StudyBench closeout and study packet refs as pending proposal evidence', async () => {
    const store = new ActionSubmissionStore()
    const evidenceRefs = [
      'probe_closeout.probe_run.studybench_patch.openagents_launch_0006.sha',
      'rubric_score.probe.studybench_patch.openagents_launch_0006.sha',
      'study_packet.openagents_launch.v0',
    ]
    const submission = await recordSubmission(store, {
      evidenceRefs,
      id: 'action_submission.probe.studybench.closeout_ref',
      idempotencyKey: 'action-submission:probe:studybench:closeout-ref',
      metadata: { studyPacketRef: 'study_packet.openagents_launch.v0' },
      proposedByProgramRunId:
        'blueprint_program_run.probe.studybench_patch.openagents_launch_0006',
      proposedEffectRef: 'effect.probe.create_pull_request.studybench_patch',
      summaryRef: 'summary.probe.studybench.patch.closeout_ref',
    })

    expect(blueprintActionSubmissionStudybenchEvidenceRefs(evidenceRefs)).toEqual(
      evidenceRefs,
    )
    expect(submission).toMatchObject({
      approvalState: 'pending',
      directExecution: false,
      directProgramRunExecutionAllowed: false,
      evidenceRefs,
      programRunAuthorityBoundary: 'evidence_only',
      proposalOnly: true,
      status: 'pending_approval',
    })
    expect(blueprintActionSubmissionCanExecute(submission)).toBe(false)
  })

  test('rejects unsafe StudyBench evidence refs with raw or private material', async () => {
    const store = new ActionSubmissionStore()
    const unsafeEvidenceRefs = [
      'raw.source.archive.openagents',
      'raw.run.log.openagents',
      'payment.preimage.openagents',
      'wallet.secret.openagents',
      'provider.payload.openagents',
      'customer.email.openagents',
      'private.repo.openagents',
    ]

    for (const evidenceRef of unsafeEvidenceRefs) {
      await expect(
        recordSubmission(store, {
          evidenceRefs: [evidenceRef],
          idempotencyKey: `action-submission:probe:unsafe:${evidenceRef}`,
        }),
      ).rejects.toBeInstanceOf(BlueprintActionSubmissionValidationError)
    }
  })
})
