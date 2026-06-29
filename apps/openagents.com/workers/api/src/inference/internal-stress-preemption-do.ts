import type { DispatchSchedulerPreemptionEvidence } from './model-router'
import { currentEpochMillis } from '../runtime-primitives'

type ActiveStressRow = Readonly<{
  evidence_ref: string | null
  expires_at_ms: number
  preempted_at_ms: number | null
  preemption_reason: string | null
  registered_at_ms: number
  request_id: string
}>

const json = (value: unknown, init: ResponseInit = {}) =>
  (() => {
    const headers = new Headers(init.headers)
    headers.set('cache-control', 'no-store')
    return Response.json(value, { ...init, headers })
  })()

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null

const stringField = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : undefined

const numberField = (value: unknown): number | undefined =>
  typeof value === 'number' && Number.isFinite(value) ? value : undefined

const evidenceFor = (
  row: Pick<ActiveStressRow, 'evidence_ref' | 'preemption_reason' | 'request_id'>,
): DispatchSchedulerPreemptionEvidence | undefined => {
  if (row.evidence_ref === null || row.preemption_reason === null) {
    return undefined
  }
  return {
    evidenceRef: row.evidence_ref,
    reason: row.preemption_reason,
    targetDemandClass: 'internal_stress',
    targetOutcome: 'preempted_yielded',
  }
}

export class GlmStressSchedulerDurableObject {
  constructor(private readonly state: DurableObjectState) {
    state.blockConcurrencyWhile(async () => {
      this.state.storage.sql.exec(`
        CREATE TABLE IF NOT EXISTS active_internal_stress (
          request_id TEXT PRIMARY KEY,
          registered_at_ms INTEGER NOT NULL,
          expires_at_ms INTEGER NOT NULL,
          preempted_at_ms INTEGER,
          preemption_reason TEXT,
          evidence_ref TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_active_internal_stress_live
          ON active_internal_stress (expires_at_ms, preempted_at_ms, registered_at_ms);
      `)
    })
  }

  async fetch(request: Request) {
    const url = new URL(request.url)
    if (
      request.method === 'POST' &&
      url.pathname === '/v1/internal-stress/register'
    ) {
      return this.register(request)
    }
    if (
      request.method === 'POST' &&
      url.pathname === '/v1/internal-stress/release'
    ) {
      return this.release(request)
    }
    if (
      request.method === 'POST' &&
      url.pathname === '/v1/internal-stress/preempt'
    ) {
      return this.preempt(request)
    }
    if (
      request.method === 'POST' &&
      url.pathname === '/v1/internal-stress/snapshot'
    ) {
      return this.snapshot(request)
    }
    if (
      request.method === 'GET' &&
      url.pathname === '/v1/internal-stress/preempted'
    ) {
      return this.preempted(url)
    }
    return json({ error: 'not_found' }, { status: 404 })
  }

  async alarm(): Promise<void> {
    const nowMs = currentEpochMillis()
    this.prune(nowMs)
    await this.scheduleNextAlarm()
  }

  private async body(request: Request): Promise<Record<string, unknown>> {
    const body = await request.json().catch(() => undefined)
    return isRecord(body) ? body : {}
  }

  private prune(nowMs: number): void {
    this.state.storage.sql
      .exec('DELETE FROM active_internal_stress WHERE expires_at_ms <= ?', nowMs)
  }

  private liveCount(nowMs: number): number {
    return this.state.storage.sql
      .exec<{ count: number }>(
        `SELECT COUNT(*) AS count
           FROM active_internal_stress
          WHERE expires_at_ms > ?
            AND preempted_at_ms IS NULL`,
        nowMs,
      )
      .one().count
  }

  private nextExpiryMs(): number | undefined {
    const row = this.state.storage.sql
      .exec<{ expires_at_ms: number }>(
        `SELECT MIN(expires_at_ms) AS expires_at_ms
           FROM active_internal_stress`,
      )
      .one()
    return Number.isFinite(row.expires_at_ms) ? row.expires_at_ms : undefined
  }

  private async scheduleNextAlarm(): Promise<void> {
    const next = this.nextExpiryMs()
    if (next === undefined) {
      await this.state.storage.deleteAlarm()
      return
    }
    await this.state.storage.setAlarm(next)
  }

  private async register(request: Request) {
    const body = await this.body(request)
    const requestId = stringField(body.requestId)
    const nowMs = numberField(body.nowMs) ?? currentEpochMillis()
    const expiresAtMs = numberField(body.expiresAtMs)

    if (requestId === undefined || expiresAtMs === undefined) {
      return json({ error: 'invalid_internal_stress_registration' }, { status: 400 })
    }

    this.prune(nowMs)
    this.state.storage.sql.exec(
      `INSERT OR REPLACE INTO active_internal_stress
        (request_id, registered_at_ms, expires_at_ms, preempted_at_ms, preemption_reason, evidence_ref)
       VALUES (?, ?, ?, NULL, NULL, NULL)`,
      requestId,
      nowMs,
      expiresAtMs,
    )
    await this.scheduleNextAlarm()

    return json({ ok: true })
  }

  private async release(request: Request) {
    const body = await this.body(request)
    const requestId = stringField(body.requestId)
    const nowMs = numberField(body.nowMs) ?? currentEpochMillis()

    if (requestId === undefined) {
      return json({ error: 'invalid_internal_stress_release' }, { status: 400 })
    }

    this.prune(nowMs)
    this.state.storage.sql.exec(
      'DELETE FROM active_internal_stress WHERE request_id = ?',
      requestId,
    )
    await this.scheduleNextAlarm()

    return json({ ok: true })
  }

  private async snapshot(request: Request) {
    const body = await this.body(request)
    const nowMs = numberField(body.nowMs) ?? currentEpochMillis()
    this.prune(nowMs)
    return json({ activeStressCount: this.liveCount(nowMs) })
  }

  private async preempt(request: Request) {
    const body = await this.body(request)
    const nowMs = numberField(body.nowMs) ?? currentEpochMillis()
    const reason =
      stringField(body.reason) ?? 'external_reserved_headroom_unavailable'
    this.prune(nowMs)

    const target = this.state.storage.sql
      .exec<ActiveStressRow>(
        `SELECT request_id, registered_at_ms, expires_at_ms, preempted_at_ms,
                preemption_reason, evidence_ref
           FROM active_internal_stress
          WHERE expires_at_ms > ?
            AND preempted_at_ms IS NULL
          ORDER BY registered_at_ms ASC
          LIMIT 1`,
        nowMs,
      )
      .toArray()[0]

    if (target === undefined) {
      return json({ preempted: false })
    }

    const evidenceRef = `scheduler.preemption.internal_stress.${target.request_id}`
    this.state.storage.sql.exec(
      `UPDATE active_internal_stress
          SET preempted_at_ms = ?,
              preemption_reason = ?,
              evidence_ref = ?
        WHERE request_id = ?`,
      nowMs,
      reason,
      evidenceRef,
      target.request_id,
    )

    return json({
      evidence: {
        evidenceRef,
        reason,
        targetDemandClass: 'internal_stress',
        targetOutcome: 'preempted_yielded',
      },
      preempted: true,
    })
  }

  private preempted(url: URL) {
    const requestId = stringField(url.searchParams.get('requestId'))
    if (requestId === undefined) {
      return json({ error: 'invalid_internal_stress_request_id' }, { status: 400 })
    }
    const nowMs = currentEpochMillis()
    this.prune(nowMs)
    const row = this.state.storage.sql
      .exec<Pick<ActiveStressRow, 'evidence_ref' | 'preemption_reason' | 'request_id'>>(
        `SELECT request_id, preemption_reason, evidence_ref
           FROM active_internal_stress
          WHERE request_id = ?
            AND preempted_at_ms IS NOT NULL
          LIMIT 1`,
        requestId,
      )
      .toArray()[0]
    const evidence = row === undefined ? undefined : evidenceFor(row)
    return json({
      ...(evidence === undefined
        ? {}
        : { evidenceRef: evidence.evidenceRef, reason: evidence.reason }),
      preempted: evidence !== undefined,
    })
  }
}
