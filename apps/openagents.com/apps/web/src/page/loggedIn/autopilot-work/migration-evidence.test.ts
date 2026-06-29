import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeMigrationEvidenceInput,
  projectForgeMigrationEvidence,
} from './migration-evidence'

const baseInput = {
  generatedAt: '2026-06-18T04:00:00.000Z',
  registryRefs: ['migration-registry.public.v1'],
  snapshotRef: 'migration-evidence-snapshot.public.work_1',
  versionRef: 'migration-evidence-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const completedEntry = {
  domain: 'settings' as const,
  domainRef: 'migration-domain.public.settings',
  freshness: 'fresh' as const,
  idempotencyRefs: ['migration-idempotency.public.settings'],
  migrationRefs: ['migration-step.public.settings.v1_to_v2'],
  policyRefs: ['migration-policy.public.redacted'],
  receiptRefs: ['migration-receipt.public.settings.v2'],
  redactionRefs: ['migration-redaction.public.settings'],
  registryRefs: ['migration-registry.public.v1'],
  required: true,
  restorePointRefs: ['migration-restore.public.settings.v1'],
  rollbackBoundaryRefs: ['migration-rollback-boundary.public.settings.v1'],
  schemaFromRef: 'schema.public.settings.v1',
  schemaToRef: 'schema.public.settings.v2',
  status: 'completed' as const,
  validationRefs: ['migration-validation.public.settings.v2'],
}

describe('Forge migration evidence projection', () => {
  test('projects migration evidence as refs-only non-authoritative state', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [completedEntry],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      completed: 1,
      domains: 1,
      failed: 0,
      rebuildable: 0,
      required: 1,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      cacheRebuildAuthority: false,
      deploymentAuthority: false,
      downgradeExecutionAuthority: false,
      exportGenerationAuthority: false,
      migrationExecutionAuthority: false,
      publicClaimAuthority: false,
      registryMutationAuthority: false,
      restoreAuthority: false,
      rollbackAuthority: false,
      settlementAuthority: false,
      snapshotCreationAuthority: false,
      startupRecoveryTransitionAuthority: false,
      validationExecutionAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing migration evidence as empty', () => {
    const view = projectForgeMigrationEvidence({
      generatedAt: '2026-06-18T04:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks required migrations without restore or rollback boundaries', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [
        {
          ...completedEntry,
          domainRef: 'migration-domain.public.no_restore',
          rollbackBoundaryRefs: [],
          restorePointRefs: [],
          status: 'required',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:required-migration-missing-restore-boundary:migration-domain.public.no_restore',
    )
  })

  test('blocks completed migrations without validation refs', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [
        {
          ...completedEntry,
          domainRef: 'migration-domain.public.no_validation',
          validationRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:completed-migration-missing-validation:migration-domain.public.no_validation',
    )
  })

  test('blocks completed migrations without receipt refs', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [
        {
          ...completedEntry,
          domainRef: 'migration-domain.public.no_receipt',
          receiptRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:completed-migration-missing-receipt:migration-domain.public.no_receipt',
    )
  })

  test('blocks optional cache rebuilds without rebuild and policy refs', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [
        {
          ...completedEntry,
          domain: 'artifact_indexes',
          domainRef: 'migration-domain.public.artifact_cache',
          optionalCache: true,
          optionalCacheRebuildRefs: [],
          policyRefs: [],
          required: false,
          status: 'rebuildable',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:optional-cache-rebuild-policy-missing:migration-domain.public.artifact_cache',
    )
  })

  test('blocks downgrade evidence without downgrade and policy refs', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [
        {
          ...completedEntry,
          domainRef: 'migration-domain.public.downgrade',
          downgradeRefs: [],
          downgradeRequired: true,
          policyRefs: [],
          status: 'rolled_back',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:downgrade-policy-missing:migration-domain.public.downgrade',
    )
  })

  test('blocks failed required migrations', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [
        {
          ...completedEntry,
          domainRef: 'migration-domain.public.failed',
          status: 'failed',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:failed-required-migration:migration-domain.public.failed',
    )
  })

  test('blocks stale migration evidence', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      entries: [
        {
          ...completedEntry,
          domainRef: 'migration-domain.public.stale',
          freshness: 'stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:stale-migration-evidence:migration-domain.public.stale',
    )
  })

  test('blocks populated migration evidence without snapshot refs', () => {
    const view = projectForgeMigrationEvidence({
      entries: [completedEntry],
      generatedAt: '2026-06-18T04:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.no_snapshot:missing-migration-evidence-snapshot-ref',
    )
  })

  test('omits unsafe private migration material before projection', () => {
    const view = projectForgeMigrationEvidence({
      ...baseInput,
      blockerRefs: [
        'migration-blocker.public.safe',
        'raw fixture /Users/christopher/state.json',
      ],
      entries: [
        {
          ...completedEntry,
          domainRef: 'migration-domain.public.safe',
          idempotencyRefs: [
            'migration-idempotency.public.safe',
            'credential value password private',
          ],
          migrationRefs: ['migration-step.public.safe', 'state payload /Users/christopher/state.json'],
          policyRefs: ['migration-policy.public.safe', 'bearer token private'],
          receiptRefs: ['migration-receipt.public.safe'],
          redactionRefs: ['migration-redaction.public.safe'],
          registryRefs: ['migration-registry.public.safe', 'provider payload sk-private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.domainRef).toBe('migration-domain.public.safe')
    expect(view.entries[0]?.migrationRefs).toEqual(['migration-step.public.safe'])
    expect(view.entries[0]?.policyRefs).toEqual(['migration-policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-migration-evidence-blocker:work.public.work_1:unsafe-migration-evidence-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw fixture')
    expect(payload).not.toContain('credential value')
    expect(payload).not.toContain('state payload')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T04:00:00.000Z',
      migrationEvidence: {
        entries: [completedEntry],
        generatedAt: '2026-06-18T04:01:00.000Z',
        registryRefs: ['migration-registry.public.v2'],
        snapshotRef: 'migration-evidence-snapshot.public.work_2',
        versionRef: 'migration-evidence-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeMigrationEvidenceInput(work)).toEqual({
      entries: [completedEntry],
      generatedAt: '2026-06-18T04:01:00.000Z',
      registryRefs: ['migration-registry.public.v2'],
      snapshotRef: 'migration-evidence-snapshot.public.work_2',
      versionRef: 'migration-evidence-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
