import { describe, expect, test } from 'vitest'

import {
  buildMirrorCodeBackstopBatchPlan,
  buildMirrorCodeBackstopBurnReport,
  MIRRORCODE_BACKSTOP_ISSUE_NUMBER,
  MIRRORCODE_BACKSTOP_LEDGER_REF,
  MIRRORCODE_BACKSTOP_TASK_REF,
} from './mirrorcode-backstop-burn'
import { buildMirrorCodeRun } from './mirrorcode-contract'

describe('MirrorCode standing backstop burn', () => {
  test('plans a bounded public-target batch for issue #6710 with gym demand tags', () => {
    const plan = buildMirrorCodeBackstopBatchPlan({
      batchRef: 'batch.public.gym.mirrorcode.backstop_burn.2026-06-28',
      maxTasks: 4,
    })

    expect(plan.schemaVersion).toBe(
      'openagents.gym.mirrorcode_backstop_burn.v1',
    )
    expect(plan.issueNumber).toBe(MIRRORCODE_BACKSTOP_ISSUE_NUMBER)
    expect(plan.taskRef).toBe(MIRRORCODE_BACKSTOP_TASK_REF)
    expect(plan.ledgerRef).toBe(MIRRORCODE_BACKSTOP_LEDGER_REF)
    expect(plan.demandKind).toBe('internal')
    expect(plan.demandSource).toBe('gym_mirrorcode')
    expect(plan.taskCount).toBe(4)
    expect(plan.tasks.map(task => task.taskId)).toEqual([
      'qsv_select',
      'jq_simple',
      'gron',
      'bitwise',
    ])
    expect(plan.tasks.every(task => task.bucket === 'S')).toBe(true)
    expect(plan.tasks.every(task => task.grade === 'smoke')).toBe(true)
    expect(
      plan.tasks.every(task =>
        task.ledgerTraceRef.startsWith(
          'trace.public.gym.mirrorcode.backstop.',
        ),
      ),
    ).toBe(true)
  })

  test('skips completed and active task keys before filling the next batch', () => {
    const plan = buildMirrorCodeBackstopBatchPlan({
      maxTasks: 3,
      completedTaskRefs: ['S:qsv_select:python'],
      activeTaskRefs: ['S:jq_simple:python'],
    })

    expect(plan.tasks.map(task => task.taskId)).toEqual([
      'gron',
      'bitwise',
      'hexyl',
    ])
  })

  test('builds a ledger-ready report with pass rate and exact token evidence separated', () => {
    const passed = buildMirrorCodeRun({
      runId: 'mc-backstop-s-cal-python-0001',
      model: 'openagents/khala',
      taskId: 'cal',
      bucket: 'S',
      language: 'python',
      status: 'passed',
      passRate: 0.8,
      tokens: { total: 1_000 },
      exactTokenUsageEventRefs: ['token_usage_event.gym_mirrorcode.cal.0001'],
      startedAt: '2026-06-28T00:00:00.000Z',
      finishedAt: '2026-06-28T00:30:00.000Z',
      summary: 'Standing backstop public cal run through openagents/khala.',
      grade: 'smoke',
    })
    const failed = buildMirrorCodeRun({
      runId: 'mc-backstop-s-choose-python-0001',
      model: 'openagents/khala',
      taskId: 'choose',
      bucket: 'S',
      language: 'python',
      status: 'failed',
      passRate: 0.2,
      tokens: { total: 2_000 },
      exactTokenUsageEventRefs: [],
      startedAt: '2026-06-28T00:30:00.000Z',
      finishedAt: '2026-06-28T01:00:00.000Z',
      summary: 'Standing backstop public choose run through openagents/khala.',
      grade: 'smoke',
    })

    const report = buildMirrorCodeBackstopBurnReport([failed, passed])

    expect(report.schemaVersion).toBe(
      'openagents.gym.mirrorcode_backstop_burn.v1',
    )
    expect(report.issueNumber).toBe(6710)
    expect(report.runCount).toBe(2)
    expect(report.terminalRunCount).toBe(2)
    expect(report.passedRunCount).toBe(1)
    expect(report.passRateBps).toBe(5_000)
    expect(report.totalTokensBurned).toBe(3_000)
    expect(report.exactTokenBackedTokens).toBe(1_000)
    expect(report.tokenBurnReport.unprovenTokenTotal).toBe(2_000)
    expect(report.ledgerTraces).toHaveLength(2)
    expect(report.ledgerTraces[0]).toMatchObject({
      runId: 'mc-backstop-s-choose-python-0001',
      taskId: 'choose',
      passRateBps: 2_000,
      exactTokenUsageEventRefs: [],
    })
    expect(report.ledgerTraces[1]?.tokenAttributionProofRef).toBe(
      'proof.gym.mirrorcode.exact_token_rows.mc-backstop-s-cal-python-0001',
    )
  })
})
