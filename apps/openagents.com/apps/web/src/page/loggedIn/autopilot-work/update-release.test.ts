import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeUpdateReleaseInput,
  projectForgeUpdateRelease,
} from './update-release'

const baseInput = {
  generatedAt: '2026-06-18T03:40:00.000Z',
  manifestRefs: ['release-manifest.public.v1_3_0'],
  policyRefs: ['release-policy.public.managed'],
  snapshotRef: 'update-release-snapshot.public.work_1',
  versionRef: 'update-release-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

const readyEntry = {
  artifactRefs: ['release-artifact.public.darwin_arm64'],
  channel: 'stable' as const,
  channelRefs: ['release-channel.public.stable'],
  checksumRefs: ['release-checksum.public.v1_3_0'],
  compatibilityRefs: ['release-compat.public.runtime_v1'],
  freshness: 'fresh' as const,
  manifestRefs: ['release-manifest.public.v1_3_0'],
  platformRefs: ['release-platform.public.darwin_arm64'],
  policyRefs: ['release-policy.public.managed'],
  releaseNoteRefs: ['release-notes.public.v1_3_0'],
  releaseRef: 'release.public.v1_3_0',
  rollbackRefs: ['release-rollback.public.v1_2_9'],
  rolloutRefs: ['release-rollout.public.stable_10pct'],
  runtimeRequirementRefs: ['release-runtime.public.node_bun'],
  signatureRefs: ['release-signature.public.v1_3_0'],
  smokeReceiptRefs: ['release-smoke.public.v1_3_0'],
  status: 'recommended' as const,
  supportRefs: ['release-support.public.v1_3_0'],
  versionRef: 'release-version.public.v1_3_0',
}

describe('Forge update and release projection', () => {
  test('projects release evidence as refs-only non-authoritative state', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [readyEntry],
    })

    expect(view.status).toBe('update_available')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      available: 1,
      blocked: 0,
      current: 0,
      entries: 1,
      managed: 0,
      required: 0,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      channelPinMutationAuthority: false,
      deploymentAuthority: false,
      installerAuthority: false,
      managedPolicyMutationAuthority: false,
      manifestFetchAuthority: false,
      manifestVerificationAuthority: false,
      migrationAuthority: false,
      publicClaimAuthority: false,
      rollbackAuthority: false,
      settlementAuthority: false,
      smokeExecutionAuthority: false,
      updateCheckNetworkAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing update/release state as empty', () => {
    const view = projectForgeUpdateRelease({
      generatedAt: '2026-06-18T03:40:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.entries).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks update claims without manifest, integrity, platform, compatibility, and smoke refs', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [
        {
          channel: 'stable',
          releaseRef: 'release.public.incomplete',
          status: 'required',
          versionRef: 'release-version.public.incomplete',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toEqual(
      expect.arrayContaining([
        'forge-update-release-blocker:work.public.work_1:update-claim-missing-manifest:release.public.incomplete',
        'forge-update-release-blocker:work.public.work_1:update-claim-missing-integrity:release.public.incomplete',
        'forge-update-release-blocker:work.public.work_1:update-claim-missing-platform:release.public.incomplete',
        'forge-update-release-blocker:work.public.work_1:update-claim-missing-compatibility:release.public.incomplete',
        'forge-update-release-blocker:work.public.work_1:update-claim-missing-smoke:release.public.incomplete',
      ]),
    )
  })

  test('blocks stale release evidence', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          freshness: 'stale',
          releaseRef: 'release.public.stale',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-update-release-blocker:work.public.work_1:stale-release-evidence:release.public.stale',
    )
  })

  test('blocks migration-required updates without restore or rollback refs', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          migrationRequired: true,
          releaseRef: 'release.public.migration',
          rollbackRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-update-release-blocker:work.public.work_1:migration-without-restore-or-rollback:release.public.migration',
    )
  })

  test('blocks active-run update claims without safe update window refs', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          activeRunRefs: ['run.public.active'],
          releaseRef: 'release.public.active_run',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-update-release-blocker:work.public.work_1:active-run-update-without-safe-window:release.public.active_run',
    )
  })

  test('blocks managed pins without policy refs', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          managedPinRefs: ['release-pin.public.v1_2_9'],
          policyRefs: [],
          releaseRef: 'release.public.pinned',
          status: 'current',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.managedOverride).toBe(true)
    expect(view.blockerRefs).toContain(
      'forge-update-release-blocker:work.public.work_1:managed-pin-policy-missing:release.public.pinned',
    )
  })

  test('shows managed override when a pin has policy evidence', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          channel: 'managed',
          managedPinRefs: ['release-pin.public.v1_2_9'],
          releaseRef: 'release.public.managed',
          status: 'current',
        },
      ],
    })

    expect(view.status).toBe('current')
    expect(view.entries[0]?.managedOverride).toBe(true)
    expect(view.counts.managed).toBe(1)
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks release notes without receipt refs', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      entries: [
        {
          ...readyEntry,
          releaseRef: 'release.public.notes',
          smokeReceiptRefs: [],
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-update-release-blocker:work.public.work_1:release-notes-without-receipts:release.public.notes',
    )
  })

  test('blocks populated release entries without snapshot refs', () => {
    const view = projectForgeUpdateRelease({
      entries: [readyEntry],
      generatedAt: '2026-06-18T03:40:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-update-release-blocker:work.public.no_snapshot:missing-update-release-snapshot-ref',
    )
  })

  test('omits unsafe private update/release material before projection', () => {
    const view = projectForgeUpdateRelease({
      ...baseInput,
      blockerRefs: [
        'release-blocker.public.safe',
        'raw manifest /Users/christopher/release.json',
      ],
      entries: [
        {
          ...readyEntry,
          artifactRefs: ['release-artifact.public.safe', 'artifact payload /Users/christopher/pkg.tgz'],
          manifestRefs: ['release-manifest.public.safe', 'manifest body bearer token private'],
          platformRefs: ['release-platform.public.safe'],
          policyRefs: ['release-policy.public.safe', 'credential private'],
          releaseNoteRefs: ['release-note.public.safe', 'release note body /Users/christopher/notes.md'],
          releaseRef: 'release.public.safe',
          signatureRefs: ['release-signature.public.safe', 'provider payload sk-private'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.entries[0]?.artifactRefs).toEqual(['release-artifact.public.safe'])
    expect(view.entries[0]?.manifestRefs).toEqual(['release-manifest.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-update-release-blocker:work.public.work_1:unsafe-update-release-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw manifest')
    expect(payload).not.toContain('artifact payload')
    expect(payload).not.toContain('manifest body')
    expect(payload).not.toContain('release note body')
    expect(payload).not.toContain('credential private')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      generatedAt: '2026-06-18T03:40:00.000Z',
      updateRelease: {
        entries: [readyEntry],
        generatedAt: '2026-06-18T03:41:00.000Z',
        manifestRefs: ['release-manifest.public.v1_3_0'],
        policyRefs: ['release-policy.public.managed'],
        snapshotRef: 'update-release-snapshot.public.work_2',
        versionRef: 'update-release-version.public.v2',
      },
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeUpdateReleaseInput(work)).toEqual({
      entries: [readyEntry],
      generatedAt: '2026-06-18T03:41:00.000Z',
      manifestRefs: ['release-manifest.public.v1_3_0'],
      policyRefs: ['release-policy.public.managed'],
      snapshotRef: 'update-release-snapshot.public.work_2',
      versionRef: 'update-release-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
