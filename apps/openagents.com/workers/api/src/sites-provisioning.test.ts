import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import { AutopilotSiteUnsafePayload } from './sites'
import {
  recordSiteProvisioningPlan,
  type SiteProvisioningRuntime,
} from './sites-provisioning'

type Row = Record<string, unknown>

class ProvisioningStore {
  site_projects: Array<Row> = [
    {
      archived_at: null,
      id: 'site_project_1',
    },
  ]
  site_provisioning_plans: Array<Row> = []
}

const active = (row: Row): boolean => row.archived_at === null

class ProvisioningStatement implements D1PreparedStatement {
  private values: ReadonlyArray<unknown> = []

  constructor(
    private readonly query: string,
    private readonly store: ProvisioningStore,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.values = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM site_projects')) {
      return Promise.resolve(
        (this.store.site_projects.find(
          row => row.id === this.values[0] && active(row),
        ) ?? null) as T | null,
      )
    }

    if (this.query.includes('FROM site_provisioning_plans')) {
      return Promise.resolve(
        (this.store.site_provisioning_plans.find(
          row => row.idempotency_key === this.values[0] && active(row),
        ) ?? null) as T | null,
      )
    }

    return Promise.reject(new Error(`Unexpected first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT OR IGNORE INTO site_provisioning_plans')) {
      if (
        this.store.site_provisioning_plans.some(
          row => row.idempotency_key === this.values[1] && active(row),
        )
      ) {
        return Promise.resolve({ success: true } as D1Result<T>)
      }

      this.store.site_provisioning_plans.push({
        archived_at: null,
        created_at: String(this.values[8]),
        id: String(this.values[0]),
        idempotency_key: String(this.values[1]),
        receipt_json: String(this.values[7]),
        requested_by_user_id: this.values[4] as string | null,
        resource_manifest_json: String(this.values[6]),
        reviewed_at: this.values[9] as string | null,
        reviewed_by_user_id: this.values[5] as string | null,
        site_id: String(this.values[2]),
        status: String(this.values[3]),
        updated_at: String(this.values[10]),
      })

      return Promise.resolve({ success: true } as D1Result<T>)
    }

    return Promise.reject(new Error(`Unexpected run: ${this.query}`))
  }

  all<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.resolve({
      results: [] as ReadonlyArray<T>,
      success: true,
    } as D1Result<T>)
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[Array<string>, ...Array<T>]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<Array<T>>
  raw<T = unknown[]>(): Promise<[Array<string>, ...Array<T>] | Array<T>> {
    return Promise.resolve([])
  }
}

const db = (store: ProvisioningStore): D1Database => ({
  batch: () => Promise.reject(new Error('batch not used')),
  dump: () => Promise.reject(new Error('dump not used')),
  exec: () => Promise.reject(new Error('exec not used')),
  prepare: query => new ProvisioningStatement(query, store),
  withSession: () => {
    throw new Error('session not used')
  },
})

const runtime = {
  nowIso: () => '2026-06-05T21:00:00.000Z',
  randomId: prefix => `${prefix}_test`,
} satisfies SiteProvisioningRuntime

describe('recordSiteProvisioningPlan', () => {
  test('records an approved D1/R2/KV/env provisioning receipt without secrets', async () => {
    const store = new ProvisioningStore()
    const plan = await Effect.runPromise(
      recordSiteProvisioningPlan(
        db(store),
        {
          idempotencyKey: 'site-provisioning:1',
          receipt: {
            d1: [{ bindingName: 'SITE_DB', migrationRef: 'migration:001' }],
            r2: [{ bindingName: 'SITE_UPLOADS', prefix: 'sites/site_project_1' }],
          },
          requestedByUserId: 'user_operator',
          resourceManifest: {
            d1: [{ bindingName: 'SITE_DB', retentionPolicy: 'standard' }],
            env: [
              {
                key: 'PUBLIC_SITE_NAME',
                kind: 'plain',
                plainValue: 'OTEC',
              },
              {
                key: 'PAYMENT_WEBHOOK_SECRET',
                kind: 'secret',
                secretRef: 'cf-secret:sites/otec/payment-webhook',
              },
            ],
            kv: [{ bindingName: 'SITE_CACHE' }],
            r2: [{ bindingName: 'SITE_UPLOADS', retentionPolicy: 'standard' }],
          },
          reviewedByUserId: 'user_operator',
          siteId: 'site_project_1',
        },
        runtime,
      ),
    )

    expect(plan.status).toBe('approved')
    expect(plan.reviewedAt).toBe('2026-06-05T21:00:00.000Z')
    expect(plan.resourceManifest.d1?.[0]?.bindingName).toBe('SITE_DB')
    expect(plan.resourceManifest.env?.[1]).toMatchObject({
      kind: 'secret',
      secretRef: 'cf-secret:sites/otec/payment-webhook',
    })
    expect(store.site_provisioning_plans).toHaveLength(1)
  })

  test('reuses plans by idempotency key', async () => {
    const store = new ProvisioningStore()
    const input = {
      idempotencyKey: 'site-provisioning:2',
      resourceManifest: {
        d1: [{ bindingName: 'SITE_DB' }],
      },
      siteId: 'site_project_1',
    }

    const first = await Effect.runPromise(
      recordSiteProvisioningPlan(db(store), input, runtime),
    )
    const second = await Effect.runPromise(
      recordSiteProvisioningPlan(db(store), input, runtime),
    )

    expect(first.id).toBe(second.id)
    expect(first.status).toBe('review_required')
    expect(store.site_provisioning_plans).toHaveLength(1)
  })

  test('rejects secret-shaped plain environment values', async () => {
    const store = new ProvisioningStore()
    const result = await Effect.runPromiseExit(
      recordSiteProvisioningPlan(
        db(store),
        {
          idempotencyKey: 'site-provisioning:secret',
          resourceManifest: {
            env: [
              {
                key: 'OPENAI_API_KEY',
                kind: 'plain',
                plainValue: 'sk-testabcdefghijklmnopqrstuvwxyz',
              },
            ],
          },
          siteId: 'site_project_1',
        },
        runtime,
      ),
    )

    expect(result._tag).toBe('Failure')
    if (result._tag === 'Failure') {
      expect(String(result.cause)).toContain(AutopilotSiteUnsafePayload.name)
    }
    expect(store.site_provisioning_plans).toHaveLength(0)
  })
})
