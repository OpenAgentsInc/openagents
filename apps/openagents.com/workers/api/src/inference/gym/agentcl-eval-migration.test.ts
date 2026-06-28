import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

const migrationPath = join(
  import.meta.dirname,
  '../../../migrations/0256_agentcl_eval_v0.sql',
)

const makeDb = (): DatabaseSync => {
  const db = new DatabaseSync(':memory:')
  db.exec('PRAGMA foreign_keys = ON;')
  db.exec(readFileSync(migrationPath, 'utf8'))
  return db
}

const tableNames = (db: DatabaseSync): ReadonlyArray<string> =>
  db
    .prepare(
      `
        SELECT name
          FROM sqlite_master
         WHERE type = 'table'
           AND name LIKE 'gym_agentcl_eval_%'
         ORDER BY name ASC
      `,
    )
    .all()
    .map(row => String(row.name))

describe('AgentCL eval v0 D1 migration', () => {
  test('applies cleanly and creates the normalized eval tables', () => {
    const db = makeDb()

    expect(tableNames(db)).toEqual([
      'gym_agentcl_eval_gain_metrics',
      'gym_agentcl_eval_phase_metrics',
      'gym_agentcl_eval_prompt_mutations',
      'gym_agentcl_eval_run_state_events',
      'gym_agentcl_eval_runs',
    ])

    expect(() => db.exec(readFileSync(migrationPath, 'utf8'))).not.toThrow()
  })

  test('stores run state, PG/SG/GG metrics, and prompt mutation history', () => {
    const db = makeDb()
    const now = '2026-06-28T00:00:00.000Z'

    db.prepare(
      `
        INSERT INTO gym_agentcl_eval_runs (
          eval_ref,
          schema_version,
          environment_ref,
          experiment_id,
          stream_kind,
          run_ref,
          task_set_ref,
          verifier_ref,
          runner_config_id,
          seam_id,
          seam_can_spend,
          state,
          decision_grade,
          public_claim_eligible,
          collapse_gains_into_one_number,
          run_metadata_json,
          proof_refs_json,
          caveat_refs_json,
          blocker_refs_json,
          started_at,
          completed_at,
          created_at,
          updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `,
    ).run(
      'eval.gym.agentcl.router_memory.v0',
      'openagents.gym.agentcl_eval.v0',
      'agentcl-repo-reuse',
      'gym-agentcl-repo-reuse-two-pass-fixture-v0',
      'compositional',
      'run.gym.agentcl.router_memory.v0',
      'task_set.gym.agentcl.repo_reuse.v0',
      'verifier.gym.agentcl.repo_reuse.v0',
      'gym:gym-agentcl-repo-reuse-two-pass-fixture-v0',
      'fixture',
      0,
      'completed',
      0,
      0,
      0,
      '{"publicTaskRefsOnly":true}',
      '["proof.gym.agentcl.fixture"]',
      '["caveat.public.gym.agentcl_eval.pg_sg_gg_reported_separately"]',
      '["blocker.gym.agentcl.not_decision_grade"]',
      now,
      now,
      now,
      now,
    )

    const insertPhase = db.prepare(
      `
        INSERT INTO gym_agentcl_eval_phase_metrics (
          eval_ref,
          phase,
          task_role,
          task_count,
          accepted_outcome_rate,
          score_bps,
          report_ref,
          receipt_ref,
          created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
      `,
    )
    insertPhase.run(
      'eval.gym.agentcl.router_memory.v0',
      'baseline',
      'source',
      6,
      0.45,
      4500,
      'report.gym.agentcl.baseline',
      'receipt.gym.agentcl.baseline',
      now,
    )
    insertPhase.run(
      'eval.gym.agentcl.router_memory.v0',
      'first_pass',
      'complex',
      6,
      0.62,
      6200,
      'report.gym.agentcl.first_pass',
      'receipt.gym.agentcl.first_pass',
      now,
    )

    const insertGain = db.prepare(
      `
        INSERT INTO gym_agentcl_eval_gain_metrics (
          eval_ref,
          gain_kind,
          gain_value,
          gain_bps,
          baseline_phase,
          comparison_phase,
          evidence_refs_json,
          created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
      `,
    )
    insertGain.run(
      'eval.gym.agentcl.router_memory.v0',
      'plasticity',
      0.17,
      1700,
      'baseline',
      'first_pass',
      '["evidence.gym.agentcl.pg"]',
      now,
    )
    insertGain.run(
      'eval.gym.agentcl.router_memory.v0',
      'stability',
      -0.04,
      -400,
      'first_pass',
      'frozen_second_pass',
      '["evidence.gym.agentcl.sg"]',
      now,
    )
    insertGain.run(
      'eval.gym.agentcl.router_memory.v0',
      'generalization',
      -0.04,
      -400,
      'held_out_baseline',
      'held_out_pass',
      '["evidence.gym.agentcl.gg"]',
      now,
    )

    db.prepare(
      `
        INSERT INTO gym_agentcl_eval_run_state_events (
          event_ref,
          eval_ref,
          event_index,
          state,
          observed_at,
          state_metadata_json
        ) VALUES (?, ?, ?, ?, ?, ?)
      `,
    ).run(
      'event.gym.agentcl.router_memory.completed',
      'eval.gym.agentcl.router_memory.v0',
      1,
      'completed',
      now,
      '{"taskAttemptCount":15}',
    )

    db.prepare(
      `
        INSERT INTO gym_agentcl_eval_prompt_mutations (
          mutation_ref,
          eval_ref,
          run_ref,
          pass,
          task_ref,
          step_index,
          template_ref,
          memory_before_refs_json,
          memory_after_refs_json,
          feedback_ref,
          mutation_json,
          created_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `,
    ).run(
      'mutation.public.agentcl.first_pass.step_12.agentcl.repo_reuse.complex.pg_sg_gg_report.v0',
      'eval.gym.agentcl.router_memory.v0',
      'run.gym.agentcl.router_memory.v0',
      'first_pass',
      'agentcl.repo_reuse.complex.pg_sg_gg_report.v0',
      12,
      'template.public.artanis.continual_learning.pg_sg_gg_report.v0',
      '["memory.public.agentcl.before"]',
      '["memory.public.agentcl.after"]',
      'feedback.public.agentcl.pg_sg_gg_report.v0',
      '{"rawPromptStored":false}',
      now,
    )

    const gains = db
      .prepare(
        `
          SELECT gain_kind, gain_bps
            FROM gym_agentcl_eval_gain_metrics
           WHERE eval_ref = ?
           ORDER BY gain_kind ASC
        `,
      )
      .all('eval.gym.agentcl.router_memory.v0')

    expect(gains).toEqual([
      { gain_bps: -400, gain_kind: 'generalization' },
      { gain_bps: 1700, gain_kind: 'plasticity' },
      { gain_bps: -400, gain_kind: 'stability' },
    ])

    const mutation = db
      .prepare(
        `
          SELECT step_index, mutation_json
            FROM gym_agentcl_eval_prompt_mutations
           WHERE eval_ref = ?
        `,
      )
      .get('eval.gym.agentcl.router_memory.v0')

    expect(mutation).toEqual({
      mutation_json: '{"rawPromptStored":false}',
      step_index: 12,
    })
  })

  test('refuses collapsed gain claims and orphan metric rows', () => {
    const db = makeDb()

    expect(() =>
      db
        .prepare(
          `
            INSERT INTO gym_agentcl_eval_runs (
              eval_ref,
              schema_version,
              environment_ref,
              experiment_id,
              stream_kind,
              run_ref,
              state,
              collapse_gains_into_one_number,
              run_metadata_json,
              proof_refs_json,
              caveat_refs_json,
              blocker_refs_json,
              created_at,
              updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
          `,
        )
        .run(
          'eval.gym.agentcl.collapsed',
          'openagents.gym.agentcl_eval.v0',
          'agentcl-repo-reuse',
          'gym-agentcl-repo-reuse-two-pass-fixture-v0',
          'compositional',
          'run.gym.agentcl.collapsed',
          'completed',
          1,
          '{}',
          '[]',
          '[]',
          '[]',
          '2026-06-28T00:00:00.000Z',
          '2026-06-28T00:00:00.000Z',
        ),
    ).toThrow()

    expect(() =>
      db
        .prepare(
          `
            INSERT INTO gym_agentcl_eval_gain_metrics (
              eval_ref,
              gain_kind,
              gain_value,
              gain_bps,
              baseline_phase,
              comparison_phase,
              evidence_refs_json,
              created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
          `,
        )
        .run(
          'eval.gym.agentcl.missing',
          'plasticity',
          0.1,
          1000,
          'baseline',
          'first_pass',
          '["evidence.gym.agentcl.pg"]',
          '2026-06-28T00:00:00.000Z',
        ),
    ).toThrow()
  })
})
