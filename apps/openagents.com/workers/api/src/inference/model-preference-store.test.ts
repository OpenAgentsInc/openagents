import { DatabaseSync } from 'node:sqlite'

import { describe, expect, test } from 'vitest'

import { ALL_LANES_UNARMED, type SupplyLaneArming } from './model-serving-policy'
import { HYDRALISK_GPT_OSS_20B_MODEL_ID, KHALA_MODEL_ID } from './pricing'
import {
  AUTO_EXECUTION_TARGET_ID,
  DEFAULT_EXECUTION_TARGET_ID,
  DEFAULT_MODEL_PREFERENCE_ID,
  isExecutionTargetIdAvailable,
  isModelIdAvailable,
  normalizeExecutionTargetId,
  readUserModelPreference,
  resolveAvailableExecutionTargetIds,
  resolveAvailableModelIds,
  resolveExecutionTargetPreference,
  resolveModelPreference,
  writeUserModelPreference,
} from './model-preference-store'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }

  async first<T = Row>(): Promise<T | null> {
    const row = this.db.prepare(this.sql).get(...(this.bound as never[]))
    return (row ?? null) as T | null
  }

  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}

  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
}

const SCHEMA = `
CREATE TABLE user_model_preferences (
  user_id TEXT PRIMARY KEY NOT NULL,
  model_id TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
`

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(SCHEMA)
  return new SqliteD1(raw) as unknown as D1Database
}

const NOW = '2026-07-05T12:00:00.000Z'
const USER = 'github:1'

const armedGemini: SupplyLaneArming = {
  ...ALL_LANES_UNARMED,
  'vertex-gemini': true,
}

const armedFireworksAndGemini: SupplyLaneArming = {
  ...ALL_LANES_UNARMED,
  fireworks: true,
  'vertex-gemini': true,
}

describe('resolveAvailableModelIds (MM-F1, #8484)', () => {
  test('reports nothing servable when every lane is unarmed', () => {
    expect(resolveAvailableModelIds(ALL_LANES_UNARMED)).toEqual([])
  })

  test('exposes the stable gemini alias (not the dated catalog row) when Vertex Gemini is armed', () => {
    const ids = resolveAvailableModelIds(armedGemini)
    expect(ids).toContain('gemini')
    expect(ids).not.toContain('gemini-3.5-flash')
  })

  test('exposes Fireworks open-model catalog ids when the Fireworks lane is armed', () => {
    const ids = resolveAvailableModelIds(armedFireworksAndGemini)
    expect(ids).toContain('gpt-oss-20b')
    expect(ids).toContain('gemini')
  })

  test('a raw Hydralisk model id is gated on its OWN specific arming, not the blanket lane boolean', () => {
    const arming: SupplyLaneArming = {
      ...ALL_LANES_UNARMED,
      hydralisk: true,
      hydraliskModels: {
        [HYDRALISK_GPT_OSS_20B_MODEL_ID]: false,
        'openagents/glm-5.2-reap-504b': true,
        'openai/gpt-oss-120b': false,
      },
    }
    const ids = resolveAvailableModelIds(arming)
    // The blanket lane boolean is true, but this SPECIFIC hydralisk model is
    // marked unarmed — it must not be offered.
    expect(ids).not.toContain(HYDRALISK_GPT_OSS_20B_MODEL_ID)
    // The Khala alias rides the blanket lane boolean (no per-model entry of
    // its own in hydraliskModels), so it IS offered.
    expect(ids).toContain(KHALA_MODEL_ID)
  })
})

describe('isModelIdAvailable (MM-F1, #8484)', () => {
  test('matches case/whitespace-insensitively', () => {
    expect(isModelIdAvailable('  Gemini  ', ['gemini'])).toBe(true)
    expect(isModelIdAvailable('SONNET', ['sonnet'])).toBe(true)
    expect(isModelIdAvailable('nope', ['gemini'])).toBe(false)
  })

  test('the khala slug normalizes to the canonical id both ways', () => {
    expect(isModelIdAvailable('khala', [KHALA_MODEL_ID])).toBe(true)
  })
})

describe('execution target selection (CX-4, #8548)', () => {
  test('exposes gemini, auto, hosted khala, and connected Codex accounts as targets', () => {
    const targets = resolveAvailableExecutionTargetIds({
      availableModelIds: ['gemini', KHALA_MODEL_ID],
      codexAccountRefHashes: ['acct_a', 'acct_b'],
      claudeAccountRefHashes: ['claude_a'],
    })
    expect(targets).toEqual([
      DEFAULT_EXECUTION_TARGET_ID,
      AUTO_EXECUTION_TARGET_ID,
      'khala',
      'codex:acct_a',
      'codex:acct_b',
      'claude:claude_a',
    ])
  })

  test('normalizes execution targets while preserving account-ref hash case', () => {
    expect(normalizeExecutionTargetId('  Gemini  ')).toBe('gemini')
    expect(normalizeExecutionTargetId('CODEX:AccountRefHash_01')).toBe(
      'codex:AccountRefHash_01',
    )
    expect(normalizeExecutionTargetId('CLAUDE:AccountRefHash_02')).toBe(
      'claude:AccountRefHash_02',
    )
  })

  test('only accepts account-specific provider targets that are surfaced as available', () => {
    const availableTargetIds = resolveAvailableExecutionTargetIds({
      availableModelIds: ['gemini'],
      codexAccountRefHashes: ['owner-codex'],
      claudeAccountRefHashes: ['owner-claude'],
    })
    expect(isExecutionTargetIdAvailable('codex:owner-codex', availableTargetIds)).toBe(true)
    expect(isExecutionTargetIdAvailable('codex:missing', availableTargetIds)).toBe(false)
    expect(isExecutionTargetIdAvailable('claude:owner-claude', availableTargetIds)).toBe(true)
    expect(isExecutionTargetIdAvailable('claude:missing', availableTargetIds)).toBe(false)
  })

  test('defaults to gemini when no target is set and gemini is available', () => {
    expect(
      resolveExecutionTargetPreference({
        availableTargetIds: ['gemini', 'auto', 'codex:owner-codex'],
        storedTargetId: null,
      }),
    ).toEqual({
      effectiveModelId: 'gemini',
      fallback: 'no_preference_set',
      preferredModelId: null,
      usedPreference: false,
    })
  })

  test('honors an available account-specific Codex target', () => {
    expect(
      resolveExecutionTargetPreference({
        availableTargetIds: ['gemini', 'auto', 'codex:owner-codex'],
        storedTargetId: 'codex:owner-codex',
      }),
    ).toEqual({
      effectiveModelId: 'codex:owner-codex',
      fallback: 'none',
      preferredModelId: 'codex:owner-codex',
      usedPreference: true,
    })
  })

  test('honors an available account-specific Claude target', () => {
    expect(
      resolveExecutionTargetPreference({
        availableTargetIds: ['gemini', 'auto', 'claude:owner-claude'],
        storedTargetId: 'claude:owner-claude',
      }),
    ).toEqual({
      effectiveModelId: 'claude:owner-claude',
      fallback: 'none',
      preferredModelId: 'claude:owner-claude',
      usedPreference: true,
    })
  })

  test('TYPED FALLBACK: unavailable execution targets report both requested and effective targets', () => {
    const resolution = resolveExecutionTargetPreference({
      availableTargetIds: ['gemini', 'auto'],
      storedTargetId: 'codex:exhausted-account',
    })
    expect(resolution).toEqual({
      effectiveModelId: 'gemini',
      fallback: 'preference_unavailable',
      preferredModelId: 'codex:exhausted-account',
      usedPreference: false,
    })
  })
})

describe('resolveModelPreference (MM-F1, #8484)', () => {
  const availableWithGemini = resolveAvailableModelIds(armedFireworksAndGemini)

  test('defaults to gemini when no preference is set and gemini is available', () => {
    const resolution = resolveModelPreference({
      availableModelIds: availableWithGemini,
      storedModelId: null,
    })
    expect(resolution).toEqual({
      effectiveModelId: DEFAULT_MODEL_PREFERENCE_ID,
      fallback: 'no_preference_set',
      preferredModelId: null,
      usedPreference: false,
    })
  })

  test('honors an available stored preference exactly', () => {
    const resolution = resolveModelPreference({
      availableModelIds: availableWithGemini,
      storedModelId: 'gpt-oss-20b',
    })
    expect(resolution).toEqual({
      effectiveModelId: 'gpt-oss-20b',
      fallback: 'none',
      preferredModelId: 'gpt-oss-20b',
      usedPreference: true,
    })
  })

  test('TYPED FALLBACK: never silently substitutes when the preference is unavailable', () => {
    const resolution = resolveModelPreference({
      availableModelIds: availableWithGemini,
      storedModelId: 'claude-opus-max-9000',
    })
    expect(resolution.usedPreference).toBe(false)
    expect(resolution.fallback).toBe('preference_unavailable')
    // The caller can always see BOTH what was asked for and what actually ran.
    expect(resolution.preferredModelId).toBe('claude-opus-max-9000')
    expect(resolution.effectiveModelId).toBe(DEFAULT_MODEL_PREFERENCE_ID)
  })

  test('TYPED FALLBACK: reports nothing servable rather than inventing a model when even the default is unavailable', () => {
    const resolution = resolveModelPreference({
      availableModelIds: [],
      storedModelId: 'sonnet',
    })
    expect(resolution.effectiveModelId).toBe(null)
    expect(resolution.fallback).toBe('default_unavailable')
    expect(resolution.preferredModelId).toBe('sonnet')
  })

  test('reports default_unavailable (not no_preference_set) when unset AND the default lane is down', () => {
    const resolution = resolveModelPreference({
      availableModelIds: [],
      storedModelId: null,
    })
    expect(resolution.fallback).toBe('default_unavailable')
    expect(resolution.effectiveModelId).toBe(null)
  })
})

describe('user_model_preferences D1 store (MM-F1, #8484)', () => {
  test('read returns null before any write', async () => {
    const db = makeDb()
    expect(await readUserModelPreference(db, USER)).toBe(null)
  })

  test('write then read round-trips', async () => {
    const db = makeDb()
    await writeUserModelPreference(db, { modelId: 'gemini', nowIso: NOW, userId: USER })
    const read = await readUserModelPreference(db, USER)
    expect(read?.modelId).toBe('gemini')
    expect(read?.updatedAt).toBe(NOW)
  })

  test('a later write overwrites (mutable single row, not an append-only ledger)', async () => {
    const db = makeDb()
    await writeUserModelPreference(db, { modelId: 'gemini', nowIso: NOW, userId: USER })
    await writeUserModelPreference(db, {
      modelId: 'sonnet',
      nowIso: '2026-07-05T13:00:00.000Z',
      userId: USER,
    })
    const read = await readUserModelPreference(db, USER)
    expect(read?.modelId).toBe('sonnet')

    const count = await db
      .prepare(`SELECT COUNT(*) AS n FROM user_model_preferences WHERE user_id = ?`)
      .bind(USER)
      .first<{ n: number }>()
    expect(count?.n).toBe(1)
  })

  test('preferences are isolated per user', async () => {
    const db = makeDb()
    await writeUserModelPreference(db, { modelId: 'gemini', nowIso: NOW, userId: 'github:1' })
    await writeUserModelPreference(db, { modelId: 'sonnet', nowIso: NOW, userId: 'github:2' })
    expect((await readUserModelPreference(db, 'github:1'))?.modelId).toBe('gemini')
    expect((await readUserModelPreference(db, 'github:2'))?.modelId).toBe('sonnet')
  })
})
