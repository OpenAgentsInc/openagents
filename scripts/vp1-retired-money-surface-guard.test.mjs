import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  isVp1RetirementException,
  scanVp1RetiredMoneySurfaces,
} from './vp1-retired-money-surface-guard.mjs'

const safeOpenApi = `
const retiredDiscoveryPathPattern = /billing|payments|sites/i
const activeOpenApiPaths = () => Object.fromEntries(
  Object.entries(paths()).filter(([path]) =>
    !retiredDiscoveryPathPattern.test(path),
  ),
)
const document = { paths: activeOpenApiPaths() }
`

const safeCapabilities = `
const retiredCapabilityEntryPattern = /billing|payments|sites/i
const advertisesRetiredCapability = entry => retiredCapabilityEntryPattern.test(entry.id)
const activeManifest = {
  actions: manifest.actions.filter(entry => !advertisesRetiredCapability(entry)),
  resources: manifest.resources.filter(entry => !advertisesRetiredCapability(entry)),
  rateLimits: {
    public: {
      recovery: manifest.rateLimits.public.recovery.filter(value =>
        !retiredCapabilityEntryPattern.test(value)),
    },
    authenticated: {
      recovery: manifest.rateLimits.authenticated.recovery.filter(value =>
        !retiredCapabilityEntryPattern.test(value)),
    },
  },
}
`

const scan = records =>
  scanVp1RetiredMoneySurfaces({
    files: Object.keys(records),
    readText: path => records[path],
  })

test('allows immutable history, typed 410, recovery docs, and no-spend fields', () => {
  assert.equal(
    isVp1RetirementException(
      'apps/openagents.com/workers/api/migrations/9999_retired_payments.sql',
    ),
    true,
  )
  assert.deepEqual(
    scan({
      'apps/openagents.com/workers/api/migrations/9999_retired_payments.sql':
        'CREATE TABLE historical_payments(id TEXT);',
      'apps/openagents.com/workers/api/src/money-surface-retirement.ts':
        "export const gone = { status: 410, code: 'money_surface_retired' }",
      'apps/pylon/src/no-spend.ts':
        "export const policy = { paymentMode: 'no-spend', payoutAllowed: false }",
      'docs/ops/2026-07-14-vp1-treasury-wallet-recovery-runbook.md':
        'Owner-only recovery reference for the retired treasury.',
      'docs/sol/receipts/vp1.md': 'Historical payment receipt.',
      'apps/openagents.com/workers/api/src/openagents-openapi.ts': safeOpenApi,
      'apps/openagents.com/workers/api/src/openagents-capability-manifest.ts':
        safeCapabilities,
    }),
    [],
  )
})

test('rejects retired service trees, service names, bindings, and secret mounts', () => {
  const findings = scan({
    'apps/openagents.com/services/mdk-treasury/src/server.mjs':
      'export const alive = true',
    'infra/prod/money.yaml': `
      service: oa-mdk-treasury
      env: MDK_TREASURY_MNEMONIC
    `,
    'apps/openagents.com/packages/sync-worker/src/index.ts':
      'export type Env = { MDK_TIPS_BUFFER: DurableObjectNamespace }',
    'apps/openagents.com/workers/api/src/index.ts':
      `
      isRetiredMoneySurfaceRequest(request.method, url.pathname)
      const makeBillingAwareOmniRunStore = env => makeOmniRunStoreForEnv(env)
      const reasonRef = 'continuation.skipped.paid_capacity_retired'
      export class MdkTreasuryContainer {}
      `,
    'apps/openagents.com/workers/api/src/config.ts':
      'export type Env = { MDK_TREASURY_SERVICE_TOKEN?: string }',
  })

  assert.deepEqual(
    new Set(findings.map(finding => finding.category)),
    new Set([
      'retired-service-tree',
      'retired-cloud-run-service',
      'retired-money-secret-mount',
      'retired-money-container-binding',
      'retired-money-runtime-authority',
    ]),
  )
})

test('rejects removal of OpenAPI or capability discovery filters', () => {
  const findings = scan({
    'apps/openagents.com/workers/api/src/openagents-openapi.ts':
      'export const document = { paths: paths() }',
    'apps/openagents.com/workers/api/src/openagents-capability-manifest.ts':
      'export const manifest = { actions: allActions, resources: allResources }',
  })

  assert.equal(
    findings.filter(finding =>
      finding.category.endsWith('retirement-filter-missing'),
    ).length,
    10,
  )
})

test('comments mentioning the retired topology do not restore authority', () => {
  assert.deepEqual(
    scan({
      'infra/prod/main.tf': `
        // oa-mdk-treasury and MDK_TREASURY were deleted by VP-1.
        /* STRIPE_API_KEY was never mounted here. */
        resource "google_project" "retained" {}
      `,
    }),
    [],
  )
})
