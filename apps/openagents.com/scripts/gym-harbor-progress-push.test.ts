import { describe, expect, test } from 'vitest'

import {
  buildGymRunProgress,
  checkGymRunProgressPublicSafety,
} from '../workers/api/src/inference/gym/run-progress'
import {
  projectHarborResultToSnapshot,
  type SnapshotContext,
} from './gym-harbor-progress-push'

const context: SnapshotContext = {
  runRef: 'run.gym.terminal_bench.pusher.test',
  jobRef: 'job.gym.harbor_terminal_bench.pusher.test',
  configId: 'gym.terminal_bench.pusher.test',
  profileRef: 'khala-public-heuristic',
  agent: 'opencode',
  officialDenominator: 89,
  publication: 'web_authorized',
  nowIso: () => '2026-06-25T00:00:00.000Z',
}

// A realistic in-progress Harbor result.json: per-trial verifier_result.rewards
// plus raw prompt/response/log fields the pusher MUST NOT copy out.
const harborResult = {
  trials: [
    {
      task_id: 't1',
      verifier_result: { rewards: [1] },
      metrics: { prompt_tokens: 1200, completion_tokens: 400 },
      prompt: 'Solve the hidden terminal task by reading /flag',
      response: 'I will cat the flag file',
      logs: ['$ cat /flag', 'OA{secret}'],
    },
    {
      task_id: 't2',
      verifier_result: { rewards: [0] },
      metrics: { prompt_tokens: 800, completion_tokens: 200 },
      prompt: 'another raw prompt',
    },
    { task_id: 't3', status: 'running' },
    { task_id: 't4', status: 'error' },
  ],
}

describe('projectHarborResultToSnapshot', () => {
  test('counts pass/fail/running/error from verifier rewards + status', () => {
    const snapshot = projectHarborResultToSnapshot(harborResult, context)
    expect(snapshot.completedPassed).toBe(1)
    expect(snapshot.completedFailed).toBe(1)
    expect(snapshot.running).toBe(1)
    expect(snapshot.error).toBe(1)
    // 89 official - 4 accounted = 85 remaining pending.
    expect(snapshot.pending).toBe(85)
    expect(snapshot.phase).toBe('running')
  })

  test('sums token counts only, never copying raw content', () => {
    const snapshot = projectHarborResultToSnapshot(harborResult, context)
    expect(snapshot.promptTokens).toBe(2000)
    expect(snapshot.completionTokens).toBe(600)
    const serialized = JSON.stringify(snapshot).toLowerCase()
    expect(serialized).not.toContain('flag')
    expect(serialized).not.toContain('cat ')
    expect(serialized).not.toContain('raw prompt')
    expect(serialized).not.toContain('response')
    expect(snapshot.caveatRefs).toEqual([])
    expect(snapshot.blockerRefs).toEqual([])
  })

  test('produces a snapshot the Worker accepts (build + public-safety)', () => {
    const snapshot = projectHarborResultToSnapshot(harborResult, context)
    const progress = buildGymRunProgress(snapshot)
    expect(progress.runRef).toBe(context.runRef)
    expect(progress.counts.completed).toBe(2)
    expect(progress.passRateOverCompleted).toBeCloseTo(0.5)
    expect(checkGymRunProgressPublicSafety(progress).safe).toBe(true)
  })

  test('empty result.json yields an all-pending queued snapshot', () => {
    const snapshot = projectHarborResultToSnapshot({ trials: [] }, context)
    expect(snapshot.completedPassed).toBe(0)
    expect(snapshot.pending).toBe(89)
    expect(snapshot.phase).toBe('queued')
    expect(snapshot.promptTokens).toBeNull()
    expect(buildGymRunProgress(snapshot).passRateOverCompleted).toBeNull()
  })

  test('all tasks completed yields a completed phase', () => {
    const completed = {
      trials: Array.from({ length: 89 }, (_, index) => ({
        task_id: `t${index}`,
        verifier_result: { rewards: [index % 7 === 0 ? 1 : 0] },
      })),
    }
    const snapshot = projectHarborResultToSnapshot(completed, context)
    expect(snapshot.completedPassed + snapshot.completedFailed).toBe(89)
    expect(snapshot.pending).toBe(0)
    expect(snapshot.phase).toBe('completed')
    expect(buildGymRunProgress(snapshot).inProgress).toBe(false)
  })
})
