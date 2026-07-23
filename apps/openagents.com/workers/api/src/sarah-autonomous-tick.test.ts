import { resolveAuthorityDecision } from '@openagentsinc/authority'
import type { PushResponse } from '@openagentsinc/khala-sync'
import type { SyncSql } from '@openagentsinc/khala-sync-server'
import { SARAH_RUNTIME_AUTHORITY_PROFILE } from '@openagentsinc/sarah'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { SarahOperationAuthorityOutcome } from './sarah-owner-routes'
import type { SarahAutonomousTurnResult } from './sarah-autonomous-tick'
import {
  SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES,
  SARAH_AUTONOMOUS_TICK_MAX_INTERVAL_MINUTES,
  SARAH_AUTONOMOUS_TICK_MIN_INTERVAL_MINUTES,
  SARAH_AUTONOMOUS_TICK_OBJECTIVE,
  appendSarahAutonomousUpdateToThread,
  claimSarahAutonomousTickInterval,
  isSarahAutonomousTickEnabled,
  resolveSarahAutonomousTickIntervalMinutes,
  resolveSarahAutonomousTickOwners,
  runSarahAutonomousTickForOwner,
  sarahAutonomousTickIntervalBucket,
  sarahAutonomousTickRef,
} from './sarah-autonomous-tick'

const THREAD = 'thread.sarah.deadbeefdeadbeefdeadbeef'
const OWNER = 'owner-1'

// ---------------------------------------------------------------------------
// Flag + interval configuration
// ---------------------------------------------------------------------------

describe('isSarahAutonomousTickEnabled', () => {
  test('is OFF by default and for every non-affirmative value', () => {
    expect(isSarahAutonomousTickEnabled(undefined)).toBe(false)
    expect(isSarahAutonomousTickEnabled({})).toBe(false)
    expect(isSarahAutonomousTickEnabled({ SARAH_AUTONOMOUS_TICK_ENABLED: '' })).toBe(false)
    expect(isSarahAutonomousTickEnabled({ SARAH_AUTONOMOUS_TICK_ENABLED: 'false' })).toBe(false)
    expect(isSarahAutonomousTickEnabled({ SARAH_AUTONOMOUS_TICK_ENABLED: 'no' })).toBe(false)
  })

  test('is ON only for the explicit affirmative values', () => {
    for (const value of ['true', '1', 'on']) {
      expect(isSarahAutonomousTickEnabled({ SARAH_AUTONOMOUS_TICK_ENABLED: value })).toBe(true)
    }
  })
})

describe('resolveSarahAutonomousTickIntervalMinutes', () => {
  test('defaults when unset or unparseable', () => {
    expect(resolveSarahAutonomousTickIntervalMinutes(undefined)).toBe(
      SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES,
    )
    expect(
      resolveSarahAutonomousTickIntervalMinutes({ SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES: '' }),
    ).toBe(SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES)
    expect(
      resolveSarahAutonomousTickIntervalMinutes({ SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES: 'abc' }),
    ).toBe(SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES)
    expect(
      resolveSarahAutonomousTickIntervalMinutes({ SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES: '10.5' }),
    ).toBe(SARAH_AUTONOMOUS_TICK_DEFAULT_INTERVAL_MINUTES)
  })

  test('clamps to the safe band', () => {
    expect(
      resolveSarahAutonomousTickIntervalMinutes({ SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES: '1' }),
    ).toBe(SARAH_AUTONOMOUS_TICK_MIN_INTERVAL_MINUTES)
    expect(
      resolveSarahAutonomousTickIntervalMinutes({ SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES: '99999' }),
    ).toBe(SARAH_AUTONOMOUS_TICK_MAX_INTERVAL_MINUTES)
    expect(
      resolveSarahAutonomousTickIntervalMinutes({ SARAH_AUTONOMOUS_TICK_INTERVAL_MINUTES: '30' }),
    ).toBe(30)
  })
})

describe('interval bucketing + deterministic tick ref', () => {
  test('bucket is stable within an interval and advances across it', () => {
    const interval = 15
    const base = 1_700_000_000_000
    const a = sarahAutonomousTickIntervalBucket(base, interval)
    const sameWindow = sarahAutonomousTickIntervalBucket(base + 60_000, interval)
    const nextWindow = sarahAutonomousTickIntervalBucket(base + 15 * 60_000, interval)
    expect(sameWindow).toBe(a)
    expect(nextWindow).toBe(a + 1)
  })

  test('tick ref is derived from the opaque thread suffix and never leaks the owner id', () => {
    const ref = sarahAutonomousTickRef(THREAD, 42)
    expect(ref).toBe('tick.sarah.deadbeefdeadbeefdeadbeef.b42')
    expect(ref).not.toContain(OWNER)
  })
})

// ---------------------------------------------------------------------------
// Owner resolution (reuses the exact hosted authority gate)
// ---------------------------------------------------------------------------

const makeCandidatesSql = (
  candidates: ReadonlyArray<{ owner_user_id: string; thread_ref: string }>,
): SyncSql => {
  const sql = (strings: TemplateStringsArray) => {
    const text = strings.join(' ')
    if (text.includes('FROM sarah_authority_decision_receipts')) {
      return Promise.resolve(candidates)
    }
    throw new Error(`unexpected query: ${text}`)
  }
  return sql as unknown as SyncSql
}

describe('resolveSarahAutonomousTickOwners', () => {
  test('only owners that pass the live authority gate are admitted', async () => {
    const sql = makeCandidatesSql([
      { owner_user_id: 'owner-admitted', thread_ref: 'thread.sarah.aaaaaaaaaaaaaaaaaaaaaaaa' },
      { owner_user_id: 'owner-revoked', thread_ref: 'thread.sarah.bbbbbbbbbbbbbbbbbbbbbbbb' },
    ])
    const owners = await resolveSarahAutonomousTickOwners({
      hasThreadAuthority: async (_sql, ownerUserId) => ownerUserId === 'owner-admitted',
      sql,
    })
    expect(owners).toEqual([
      { ownerUserId: 'owner-admitted', threadRef: 'thread.sarah.aaaaaaaaaaaaaaaaaaaaaaaa' },
    ])
  })

  test('is bounded by the owner cap', async () => {
    const sql = makeCandidatesSql(
      Array.from({ length: 10 }, (_v, index) => ({
        owner_user_id: `owner-${index}`,
        thread_ref: `thread.sarah.${String(index).repeat(24).slice(0, 24)}`,
      })),
    )
    const owners = await resolveSarahAutonomousTickOwners({
      hasThreadAuthority: async () => true,
      maxOwners: 2,
      sql,
    })
    expect(owners).toHaveLength(2)
  })
})

// ---------------------------------------------------------------------------
// Interval claim (atomic, cross-instance-safe)
// ---------------------------------------------------------------------------

type TickRow = { tick_ref: string; interval_bucket: number; outcome: string | null; receipt_ref: string | null }

const makeTickRunsSql = (rows: Map<string, TickRow>): SyncSql => {
  const sql = (strings: TemplateStringsArray, ...values: ReadonlyArray<unknown>) => {
    const text = strings.join(' ')
    if (text.includes('INSERT INTO sarah_autonomous_tick_runs')) {
      const [tickRef, , , bucket] = values as [string, string, string, number, string]
      if (rows.has(tickRef)) return Promise.resolve([]) // ON CONFLICT DO NOTHING
      rows.set(tickRef, { interval_bucket: bucket, outcome: null, receipt_ref: null, tick_ref: tickRef })
      return Promise.resolve([{ tick_ref: tickRef }])
    }
    if (text.includes('UPDATE sarah_autonomous_tick_runs')) {
      const [outcome, receiptRef, , tickRef] = values as [string, string | null, string, string]
      const row = rows.get(tickRef)
      if (row !== undefined) {
        row.outcome = outcome
        row.receipt_ref = receiptRef
      }
      return Promise.resolve([])
    }
    throw new Error(`unexpected query: ${text}`)
  }
  return sql as unknown as SyncSql
}

describe('claimSarahAutonomousTickInterval', () => {
  test('the first claim wins the bucket; a second claim for the same bucket is a safe no-op', async () => {
    const rows = new Map<string, TickRow>()
    const sql = makeTickRunsSql(rows)
    const first = await claimSarahAutonomousTickInterval(sql, {
      bucket: 7,
      nowIso: '2026-07-22T00:00:00.000Z',
      ownerUserId: OWNER,
      threadRef: THREAD,
    })
    expect(first).toBe(sarahAutonomousTickRef(THREAD, 7))
    const second = await claimSarahAutonomousTickInterval(sql, {
      bucket: 7,
      nowIso: '2026-07-22T00:01:00.000Z',
      ownerUserId: OWNER,
      threadRef: THREAD,
    })
    expect(second).toBeUndefined()
    // A later interval bucket is a fresh claim.
    const nextBucket = await claimSarahAutonomousTickInterval(sql, {
      bucket: 8,
      nowIso: '2026-07-22T00:15:00.000Z',
      ownerUserId: OWNER,
      threadRef: THREAD,
    })
    expect(nextBucket).toBe(sarahAutonomousTickRef(THREAD, 8))
  })
})

// ---------------------------------------------------------------------------
// Owner-thread proactive update synthesis
// ---------------------------------------------------------------------------

type RecordedMutation = Readonly<{ name: string; mutationId: number; args: Record<string, unknown> }>

const makeRecordingExecutePush = (statuses?: ReadonlyArray<'applied' | 'rejected'>) => {
  const recorded: Array<RecordedMutation> = []
  const requests: Array<Readonly<{ userId: string }>> = []
  const executePush = (input: {
    request: { mutations: ReadonlyArray<{ argsJson: string; mutationId: number; name: string }> }
    userId: string
  }): Promise<PushResponse> => {
    requests.push({ userId: input.userId })
    for (const envelope of input.request.mutations) {
      recorded.push({
        args: JSON.parse(envelope.argsJson) as Record<string, unknown>,
        mutationId: envelope.mutationId,
        name: envelope.name,
      })
    }
    return Promise.resolve({
      lastMutationId: input.request.mutations.at(-1)?.mutationId ?? 0,
      protocolVersion: 1,
      results: input.request.mutations.map((envelope, index) => ({
        mutationId: envelope.mutationId,
        status: statuses?.[index] ?? 'applied',
      })),
    } as unknown as PushResponse)
  }
  return { executePush: executePush as never, recorded, requests }
}

describe('appendSarahAutonomousUpdateToThread', () => {
  test('synthesizes one owner-scoped hosted-runtime turn carrying the update text', async () => {
    const push = makeRecordingExecutePush()
    const applied = await appendSarahAutonomousUpdateToThread(
      {
        executePush: push.executePush,
        nowIso: () => '2026-07-22T00:10:00.000Z',
        sql: (() => Promise.resolve([])) as never,
        uuid: () => 'uuid-fixed',
      },
      {
        ownerUserId: OWNER,
        text: 'I reviewed the fleet and Full Auto, delegated one bounded task, and here is the update.',
        threadRef: THREAD,
        tickRef: sarahAutonomousTickRef(THREAD, 5),
      },
    )
    expect(applied).toBe(true)
    expect(push.recorded.map(entry => entry.name)).toEqual([
      'runtime.startTurn',
      'runtime.recordEvent',
      'runtime.recordEvent',
      'runtime.recordEvent',
      'runtime.recordEvent',
    ])
    const delta = push.recorded.find(
      entry => entry.name === 'runtime.recordEvent' && entry.args.kind === 'text.delta',
    )
    expect(delta?.args.text).toContain('delegated one bounded task')
    expect(push.requests).toEqual([{ userId: OWNER }])
  })

  test('returns false when the push engine rejects any mutation in the turn', async () => {
    const push = makeRecordingExecutePush(['applied', 'rejected'])
    const applied = await appendSarahAutonomousUpdateToThread(
      { executePush: push.executePush, sql: (() => Promise.resolve([])) as never },
      { ownerUserId: OWNER, text: 'update', threadRef: THREAD, tickRef: sarahAutonomousTickRef(THREAD, 1) },
    )
    expect(applied).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// Per-owner orchestration
// ---------------------------------------------------------------------------

const allowedDecision = (tickRef: string): SarahOperationAuthorityOutcome => ({
  allowed: true,
  receiptRef: `receipt.authority.sarah.tool.${tickRef}`,
})

const makeHarness = () => {
  const rows = new Map<string, TickRow>()
  const calls = {
    append: 0,
    authorize: 0,
    push: 0,
    turn: 0,
  }
  let lastPrompt: string | undefined
  let lastAppendText: string | undefined
  const deps = {
    appendUpdate: async (input: { text: string }) => {
      calls.append += 1
      lastAppendText = input.text
      return true
    },
    authorize: async ({ tickRef }: { tickRef: string }) => {
      calls.authorize += 1
      return allowedDecision(tickRef)
    },
    intervalMinutes: 15,
    now: () => new Date('2026-07-22T00:00:00.000Z'),
    push: async () => {
      calls.push += 1
      return { ok: true }
    },
    runTurn: async ({ prompt }: { prompt: string }): Promise<SarahAutonomousTurnResult> => {
      calls.turn += 1
      lastPrompt = prompt
      return { ok: true, text: 'observed and acted', toolCallCount: 2 }
    },
    sql: makeTickRunsSql(rows),
  }
  return { calls, deps, lastAppendText: () => lastAppendText, lastPrompt: () => lastPrompt, rows }
}

describe('runSarahAutonomousTickForOwner', () => {
  test('flag-ON happy path: claims, receipts the trigger, runs ONE gated turn with the autonomous objective, appends an owner update, and pushes', async () => {
    const h = makeHarness()
    const result = await runSarahAutonomousTickForOwner(h.deps, {
      ownerUserId: OWNER,
      threadRef: THREAD,
    })
    expect(result.outcome).toBe('acted')
    if (result.outcome !== 'acted') throw new Error('unreachable')
    expect(result.threadUpdateApplied).toBe(true)
    expect(result.toolCallCount).toBe(2)
    expect(result.receiptRef).toContain('receipt.authority.sarah.tool')
    // Exactly one of each downstream call — one action per tick.
    expect(h.calls).toEqual({ append: 1, authorize: 1, push: 1, turn: 1 })
    // The turn is driven by the fixed autonomous objective, not an owner message.
    expect(h.lastPrompt()).toBe(SARAH_AUTONOMOUS_TICK_OBJECTIVE)
    expect(h.lastAppendText()).toBe('observed and acted')
    // The audit row was claimed and then settled to `acted` with the receipt.
    const [row] = [...h.rows.values()]
    expect(row?.outcome).toBe('acted')
    expect(row?.receipt_ref).toBe(result.receiptRef)
  })

  test('interval bound: a second tick in the same interval is skipped and NEVER runs a turn', async () => {
    const h = makeHarness()
    const first = await runSarahAutonomousTickForOwner(h.deps, { ownerUserId: OWNER, threadRef: THREAD })
    const second = await runSarahAutonomousTickForOwner(h.deps, { ownerUserId: OWNER, threadRef: THREAD })
    expect(first.outcome).toBe('acted')
    expect(second.outcome).toBe('interval_skip')
    // The turn, authorize, append, and push ran exactly once across both ticks.
    expect(h.calls).toEqual({ append: 1, authorize: 1, push: 1, turn: 1 })
  })

  test('a refused trigger stops before any turn and settles the audit row', async () => {
    const h = makeHarness()
    h.deps.authorize = async () => ({
      allowed: false,
      receiptRef: 'receipt.authority.sarah.tool.refused',
      refusalReason: 'owner_scope_absent',
    })
    const result = await runSarahAutonomousTickForOwner(h.deps, { ownerUserId: OWNER, threadRef: THREAD })
    expect(result.outcome).toBe('refused')
    if (result.outcome !== 'refused') throw new Error('unreachable')
    expect(result.refusalReason).toBe('owner_scope_absent')
    expect(h.calls.turn).toBe(0)
    expect(h.calls.append).toBe(0)
    expect(h.calls.push).toBe(0)
  })

  test('a turn failure reports turn_failed and writes no owner update', async () => {
    const h = makeHarness()
    h.deps.runTurn = async () => ({ detail: 'sarah_autonomous_tick_agent_failed', ok: false })
    const result = await runSarahAutonomousTickForOwner(h.deps, { ownerUserId: OWNER, threadRef: THREAD })
    expect(result.outcome).toBe('turn_failed')
    expect(h.calls.append).toBe(0)
    expect(h.calls.push).toBe(0)
  })

  test('fail-soft: an appendUpdate throw never breaks the tick — push still fires, outcome stays honest', async () => {
    const h = makeHarness()
    h.deps.appendUpdate = async () => {
      throw new Error('push engine down')
    }
    const result = await runSarahAutonomousTickForOwner(h.deps, { ownerUserId: OWNER, threadRef: THREAD })
    expect(result.outcome).toBe('acted')
    if (result.outcome !== 'acted') throw new Error('unreachable')
    expect(result.threadUpdateApplied).toBe(false)
    expect(h.calls.push).toBe(1)
  })

  test('fail-soft: a claim throw is reported as a failed outcome, never thrown', async () => {
    const h = makeHarness()
    h.deps.sql = (() => {
      throw new Error('cannot reach khala sync postgres')
    }) as unknown as SyncSql
    const result = await runSarahAutonomousTickForOwner(h.deps, { ownerUserId: OWNER, threadRef: THREAD })
    expect(result.outcome).toBe('failed')
  })
})

// ---------------------------------------------------------------------------
// Authority: reserved actions still refuse; the tick trigger action is granted.
// This proves the autonomous trigger adds NO new power — it is bound by the
// SAME admitted Sarah authority profile as every owner-triggered action.
// ---------------------------------------------------------------------------

const decideSarah = (action: string, resource: string, conditionRefs: ReadonlyArray<string>) =>
  Effect.runPromise(
    resolveAuthorityDecision(SARAH_RUNTIME_AUTHORITY_PROFILE, {
      action,
      actorRef: 'principal.sarah',
      actorRole: 'sarah_orchestrator',
      conditionResults: conditionRefs.map(conditionRef => ({
        conditionRef,
        evidenceRefs: [`evidence:${conditionRef}`],
        passed: true,
      })),
      programRef: 'program.sarah_company_operations',
      requestRef: 'request.sarah.autonomous_tick.test',
      resource,
      startedAt: '2026-07-22T00:00:00.000Z',
      triggerRef: 'autonomous_tick.test',
    }),
  )

describe('autonomous trigger is bound by the admitted Sarah authority profile', () => {
  test('the tick trigger action (read_business_context) is ALLOWED for an owner-scoped tick', async () => {
    const decision = await decideSarah('read_business_context', 'owner_business_context', [
      'condition.owner_scope',
      'condition.redaction',
      'condition.citations',
    ])
    expect(decision._tag).toBe('Allowed')
  })

  test('a reserved action (move_financial_value) is DENIED regardless of the autonomous trigger', async () => {
    const decision = await decideSarah('move_financial_value', 'owner_business_context', [
      'condition.owner_scope',
      'condition.redaction',
    ])
    expect(decision._tag).toBe('Denied')
  })

  test('a stable release without direction is a reserved, DENIED action', async () => {
    const decision = await decideSarah('publish_stable_release_without_direction', 'openagents_stable_release_channel', [
      'condition.owner_scope',
    ])
    expect(decision._tag).toBe('Denied')
  })
})
