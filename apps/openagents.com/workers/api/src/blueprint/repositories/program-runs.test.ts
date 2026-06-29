import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  blueprintProgramRunHasWriteAuthority,
  blueprintProgramRunIsEvidenceOnly,
} from '../schemas/program-run'
import {
  BlueprintProgramRunValidationError,
  listBlueprintProgramRuns,
  readBlueprintProgramRunById,
  recordBlueprintProgramRun,
} from './program-runs'

type ProgramRunRow = Readonly<{
  actor_ref: string
  archived_at: string | null
  authority_boundary: 'evidence_only'
  confidence: number
  cost_ref: string
  created_at: string
  direct_mutation_disabled: number
  evidence_refs_json: string
  id: string
  idempotency_key: string
  input_snapshot_hash: string
  latency_ms: number
  metadata_json: string
  module_version_id: string
  no_deploy: number
  no_email: number
  no_source_mutation: number
  no_spend: number
  program_signature_id: string
  program_type_id: string
  purpose_ref: string
  receipt_refs_json: string
  route_ref: string
  typed_output_json: string
  updated_at: string
}>

class ProgramRunStore {
  rows: Array<ProgramRunRow> = []
}

class ProgramRunStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ProgramRunStore,
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
    if (this.query.includes('INSERT OR IGNORE INTO blueprint_program_runs')) {
      const idempotencyKey = String(this.values[1])

      if (
        this.store.rows.every(
          item => item.idempotency_key !== idempotencyKey,
        )
      ) {
        this.store.rows.push({
          actor_ref: String(this.values[2]),
          archived_at: null,
          authority_boundary: this.values[15] as 'evidence_only',
          confidence: Number(this.values[9]),
          cost_ref: String(this.values[11]),
          created_at: String(this.values[22]),
          direct_mutation_disabled: Number(this.values[16]),
          evidence_refs_json: String(this.values[13]),
          id: String(this.values[0]),
          idempotency_key: idempotencyKey,
          input_snapshot_hash: String(this.values[7]),
          latency_ms: Number(this.values[12]),
          metadata_json: String(this.values[21]),
          module_version_id: String(this.values[6]),
          no_deploy: Number(this.values[17]),
          no_email: Number(this.values[18]),
          no_source_mutation: Number(this.values[20]),
          no_spend: Number(this.values[19]),
          program_signature_id: String(this.values[5]),
          program_type_id: String(this.values[4]),
          purpose_ref: String(this.values[3]),
          receipt_refs_json: String(this.values[14]),
          route_ref: String(this.values[10]),
          typed_output_json: String(this.values[8]),
          updated_at: String(this.values[23]),
        })
      }

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('FROM blueprint_program_runs')) {
      const limit = Number(this.values[0])
      const rows = [...this.store.rows]
        .filter(item => item.archived_at === null)
        .sort((left: ProgramRunRow, right: ProgramRunRow) =>
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
    return options?.columnNames === true ? Promise.resolve([[]]) : Promise.resolve([])
  }
}

const programRunsDb = (store: ProgramRunStore): D1Database => ({
  batch: () => Promise.reject(new Error('D1 batch should not be used')),
  dump: () => Promise.reject(new Error('D1 dump should not be used')),
  exec: () => Promise.reject(new Error('D1 exec should not be used')),
  prepare: query => new ProgramRunStatement(query, store),
  withSession: () => {
    throw new Error('D1 session should not be used')
  },
})

const runtime = {
  makeProgramRunId: () => 'blueprint_program_run_generated',
  nowIso: () => '2026-06-06T03:40:00.000Z',
}

const recordRun = (
  store: ProgramRunStore,
  overrides: Partial<Parameters<typeof recordBlueprintProgramRun>[1]> = {},
) =>
  Effect.runPromise(
    recordBlueprintProgramRun(
      programRunsDb(store),
      {
        actorRef: 'actor.adjutant',
        confidence: 0.87,
        costRef: 'cost.autopilot_run_1',
        evidenceRefs: ['evidence.context_pack_1'],
        id: 'blueprint_program_run_1',
        idempotencyKey: 'program-run:continuation:1',
        inputSnapshotHash: 'sha256.input_snapshot_1',
        latencyMs: 1200,
        metadata: { fixtureRef: 'fixture.continuation_1' },
        moduleVersionId: 'module_version.continuation_prompt.v1',
        programSignatureId: 'program_signature.autopilot_continuation.v1',
        programTypeId: 'program_type.autopilot_continuation',
        purposeRef: 'purpose.decide_next_autopilot_step',
        receiptRefs: ['receipt.program_run_1'],
        routeRef: 'route.continue',
        typedOutput: { action: 'continue', reasonRef: 'reason.more_work' },
        ...overrides,
      },
      runtime,
    ),
  )

describe('Blueprint Program Run repository', () => {
  test('records idempotent evidence-only program runs', async () => {
    const store = new ProgramRunStore()
    const run = await recordRun(store)
    const replay = await recordRun(store, {
      typedOutput: { action: 'stop' },
    })

    expect(run).toStrictEqual(replay)
    expect(store.rows).toHaveLength(1)
    expect(run).toMatchObject({
      actorRef: 'actor.adjutant',
      authorityBoundary: 'evidence_only',
      directMutationDisabled: true,
      noDeploy: true,
      noEmail: true,
      noSourceMutation: true,
      noSpend: true,
      typedOutput: { action: 'continue', reasonRef: 'reason.more_work' },
    })
    expect(blueprintProgramRunIsEvidenceOnly(run)).toBe(true)
    expect(blueprintProgramRunHasWriteAuthority(run)).toBe(false)
  })

  test('reads program runs by id', async () => {
    const store = new ProgramRunStore()
    const run = await recordRun(store)
    const read = await Effect.runPromise(
      readBlueprintProgramRunById(programRunsDb(store), run.id),
    )

    expect(read).toStrictEqual(run)
  })

  test('lists active program runs newest first with a bounded limit', async () => {
    const store = new ProgramRunStore()
    const first = await recordRun(store, {
      id: 'blueprint_program_run_1',
      idempotencyKey: 'program-run:continuation:1',
    })
    const second = await recordRun(store, {
      id: 'blueprint_program_run_2',
      idempotencyKey: 'program-run:continuation:2',
    })

    store.rows[0] = {
      ...store.rows[0]!,
      created_at: '2026-06-06T03:40:00.000Z',
      updated_at: '2026-06-06T03:40:00.000Z',
    }
    store.rows[1] = {
      ...store.rows[1]!,
      created_at: '2026-06-06T03:41:00.000Z',
      updated_at: '2026-06-06T03:41:00.000Z',
    }

    const runs = await Effect.runPromise(
      listBlueprintProgramRuns(programRunsDb(store), 1),
    )

    expect(first.id).toBe('blueprint_program_run_1')
    expect(runs).toEqual([{ ...second, createdAt: '2026-06-06T03:41:00.000Z', updatedAt: '2026-06-06T03:41:00.000Z' }])
  })

  test('rejects refs and outputs that imply write authority or private data', async () => {
    const store = new ProgramRunStore()

    await expect(
      recordRun(store, {
        routeRef: 'deploy_now',
      }),
    ).rejects.toBeInstanceOf(BlueprintProgramRunValidationError)

    await expect(
      recordRun(store, {
        routeRef: 'route.safe',
        typedOutput: { provider_payload: 'raw' },
      }),
    ).rejects.toBeInstanceOf(BlueprintProgramRunValidationError)
  })
})
