import { describe, expect, it } from 'vitest'

import {
  parseTassadarW3EvalReport,
  rebuildTassadarW3SweepProjection,
  TASSADAR_W3_EVAL_REPORT_VERSION,
  type TassadarW3EvalReport,
} from './w3-student-report'

const suite = (id: string) => ({
  branch_accuracy: null,
  divergence_causes: { memory_read: 3, output: 1 },
  divergence_step_histogram: { '<2': 3, '<8': 1 },
  exact_rollout_pass_at_1: 0.25,
  first_divergence_step_median: 12,
  first_divergence_step_median_diverged: 2,
  first_divergence_step_p90: 64,
  memory_read_accuracy: 0.5,
  output_digest_match_rate: 0.25,
  records: 8,
  replay_verifier_acceptance_rate: 0.25,
  suite: id,
  tokens_per_sec: 1000,
  valid_prefix_tokens_median: 240,
})

const validReport = (
  baseline: TassadarW3EvalReport['baseline'],
): TassadarW3EvalReport =>
  ({
    baseline,
    checkpoint_sha256: 'a'.repeat(64),
    config_digest: 'b'.repeat(64),
    corpus_id: 'corpus.tassadar_trace.v0_2.w3_100m',
    dataset_snapshot_digest: 'c'.repeat(64),
    eval_prep_sha256: 'd'.repeat(64),
    overall: suite('overall'),
    report_version: TASSADAR_W3_EVAL_REPORT_VERSION,
    suites: [suite('heldout_economic_short'), suite('adversarial_8x')],
    threads: 8,
    wall_seconds: 12.5,
  }) as TassadarW3EvalReport

describe('parseTassadarW3EvalReport', () => {
  it('accepts a well-formed report', () => {
    const result = parseTassadarW3EvalReport(validReport('baseline_a_next_token'))
    expect(result.ok).toBe(true)
  })

  it('rejects a report missing its checkpoint hash', () => {
    const report = {
      ...validReport('baseline_b_aux_state'),
      checkpoint_sha256: 'not-a-digest',
    }
    const result = parseTassadarW3EvalReport(report)
    expect(result.ok).toBe(false)
    if (!result.ok) {
      expect(
        result.failures.some(
          failure =>
            failure.kind === 'malformed_digest' &&
            'field' in failure &&
            failure.field === 'checkpoint_sha256',
        ),
      ).toBe(true)
    }
  })

  it('rejects unknown baselines and wrong versions', () => {
    const report = {
      ...validReport('baseline_a_next_token'),
      baseline: 'baseline_z',
      report_version: 'tassadar_student_eval_report.v9',
    }
    const result = parseTassadarW3EvalReport(report)
    expect(result.ok).toBe(false)
    if (!result.ok) {
      const kinds = result.failures.map(failure => failure.kind)
      expect(kinds).toContain('unknown_baseline')
      expect(kinds).toContain('wrong_version')
    }
  })

  it('rejects rates outside [0, 1]', () => {
    const broken = validReport('baseline_a_next_token')
    const report = {
      ...broken,
      overall: { ...broken.overall, replay_verifier_acceptance_rate: 1.5 },
    }
    const result = parseTassadarW3EvalReport(report)
    expect(result.ok).toBe(false)
  })
})

describe('rebuildTassadarW3SweepProjection', () => {
  it('rebuilds a single-snapshot projection from validated reports', () => {
    const reports = [
      validReport('baseline_a_next_token'),
      validReport('baseline_d_frozen_executor_learned_interface'),
    ]
    const result = rebuildTassadarW3SweepProjection(reports)
    expect(result.ok).toBe(true)
    if (result.ok) {
      expect(result.projection.baselines).toHaveLength(2)
      expect(result.projection.baselines[0]?.topDivergenceCause).toBe(
        'memory_read',
      )
    }
  })

  it('refuses to project across different snapshots', () => {
    const result = rebuildTassadarW3SweepProjection([
      validReport('baseline_a_next_token'),
      { ...validReport('baseline_b_aux_state'), dataset_snapshot_digest: 'e'.repeat(64) },
    ])
    expect(result.ok).toBe(false)
  })

  it('refuses an empty sweep', () => {
    expect(rebuildTassadarW3SweepProjection([]).ok).toBe(false)
  })
})
