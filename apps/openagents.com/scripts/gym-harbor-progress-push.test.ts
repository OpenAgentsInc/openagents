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

// The REAL Terminal-Bench 2.0 / Harbor `result.json` shape: NO per-trial array,
// a `.stats` summary with `n_*_trials` counts, summed token counts, and a
// per-eval `reward_stats.reward` map keyed by reward value (lists of task ids).
// Mirrors the live file at:
//   .tmp/terminalbench-6253-live/harbor-full/glm52-reap-mtp2-full-*/result.json
const harborStatsResult = {
  id: '23ffcbb8-c727-43dd-a993-7198969e25e0',
  started_at: '2026-06-25T09:17:22.580335',
  updated_at: '2026-06-25T17:16:06.145024Z',
  finished_at: null,
  n_total_trials: 89,
  stats: {
    n_completed_trials: 16,
    n_errored_trials: 4,
    n_running_trials: 1,
    n_pending_trials: 72,
    n_cancelled_trials: 0,
    n_retries: 0,
    evals: {
      'terminus-2__glm-5.2-reap-504b-g4__terminal-bench': {
        n_trials: 16,
        n_errors: 4,
        metrics: [{ mean: 0.5625 }],
        pass_at_k: {},
        reward_stats: {
          reward: {
            '0.0': [
              'gpt2-codegolf__ZgjuJfQ',
              'write-compressor__gTRLuMQ',
              'log-summary-date-ranges__icuCJaW',
              'pytorch-model-cli__rRprZyG',
              'path-tracing-reverse__LU5uq3N',
              'regex-chess__RGa9gUw',
              'torch-tensor-parallelism__XrCcshC',
            ],
            '1.0': [
              'llm-inference-batching-scheduler__VhXBGaW',
              'break-filter-js-from-html__sYWm2nT',
              'reshard-c4-data__gjbDoJt',
              'merge-diff-arc-agi-task__th9QSpY',
              'winning-avg-corewars__NXyJ5cC',
              'largest-eigenval__PYGgQbw',
              'password-recovery__bQ3AGa3',
              'portfolio-optimization__kjVqgog',
              'modernize-scientific-stack__yg9xFtb',
            ],
          },
        },
        exception_stats: {
          AgentTimeoutError: [
            'gpt2-codegolf__ZgjuJfQ',
            'write-compressor__gTRLuMQ',
            'path-tracing-reverse__LU5uq3N',
            'regex-chess__RGa9gUw',
          ],
        },
      },
    },
    n_input_tokens: 53390300,
    n_cache_tokens: 0,
    n_output_tokens: 261591,
  },
}

describe('projectHarborResultToSnapshot — real TB2.0 .stats summary shape', () => {
  test('maps n_*_trials counts and the official denominator', () => {
    const snapshot = projectHarborResultToSnapshot(harborStatsResult, context)
    expect(snapshot.officialDenominator).toBe(89)
    expect(snapshot.running).toBe(1)
    expect(snapshot.pending).toBe(72)
    expect(snapshot.error).toBe(4)
    expect(snapshot.cancelled).toBe(0)
    // completed (passed+failed) = n_completed_trials, errored stays a subset.
    expect(snapshot.completedPassed + snapshot.completedFailed).toBe(16)
    // disjoint lifecycle partition: completed + running + pending = 89.
    expect(16 + snapshot.running + snapshot.pending).toBe(89)
  })

  test('derives pass/fail from reward_stats.reward list lengths (passed=9, failed=7)', () => {
    const snapshot = projectHarborResultToSnapshot(harborStatsResult, context)
    expect(snapshot.completedPassed).toBe(9)
    expect(snapshot.completedFailed).toBe(7)
    const progress = buildGymRunProgress(snapshot)
    // Matches Harbor's own metrics.mean = 9/16 = 0.5625.
    expect(progress.passRateOverCompleted).toBeCloseTo(0.5625)
    expect(progress.counts.completed).toBe(16)
  })

  test('maps summed token counts from n_input_tokens / n_output_tokens', () => {
    const snapshot = projectHarborResultToSnapshot(harborStatsResult, context)
    expect(snapshot.promptTokens).toBe(53390300)
    expect(snapshot.completionTokens).toBe(261591)
  })

  test('derives running phase while pending remains, ignoring finished_at=null', () => {
    const snapshot = projectHarborResultToSnapshot(harborStatsResult, context)
    expect(snapshot.phase).toBe('running')
  })

  test('honors finished_at for a completed run', () => {
    const finished = {
      ...harborStatsResult,
      finished_at: '2026-06-25T18:00:00.000Z',
      stats: {
        ...harborStatsResult.stats,
        n_completed_trials: 89,
        n_running_trials: 0,
        n_pending_trials: 0,
      },
    }
    const snapshot = projectHarborResultToSnapshot(finished, context)
    expect(snapshot.phase).toBe('completed')
  })

  test('produces a Worker-accepted, public-safe object — never leaks task ids', () => {
    const snapshot = projectHarborResultToSnapshot(harborStatsResult, context)
    const progress = buildGymRunProgress(snapshot)
    expect(checkGymRunProgressPublicSafety(progress).safe).toBe(true)
    const serialized = JSON.stringify(snapshot)
    // The reward lists are task ids and eval content — they MUST NOT appear.
    expect(serialized).not.toContain('gpt2-codegolf')
    expect(serialized).not.toContain('terminus-2__glm')
    expect(serialized).not.toContain('reward_stats')
    expect(serialized).not.toContain('AgentTimeoutError')
  })
})
