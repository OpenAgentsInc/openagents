import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ProbeGepaUnpaidPylonLeaseProof,
  ProbeGepaUnpaidPylonLeaseProofUnsafe,
  assertProbeGepaUnpaidPylonLeaseProofSafe,
  buildProbeGepaUnpaidPylonLeaseProof,
} from './probe-gepa-unpaid-pylon-lease-proof'

describe('Probe GEPA unpaid Pylon lease proof', () => {
  test('builds demo Pylon worker leases with accepted and rejected closeouts', () => {
    const proof = buildProbeGepaUnpaidPylonLeaseProof()

    expect(S.decodeUnknownSync(ProbeGepaUnpaidPylonLeaseProof)(proof)).toEqual(
      proof,
    )
    expect(proof.workerRefs).toEqual([
      'pylon.public.demo.alpha',
      'pylon.public.demo.beta',
      'pylon.public.demo.gamma',
    ])
    expect(proof.assignmentRecords).toHaveLength(3)
    expect(proof.coordinatorImports).toHaveLength(3)
    expect(proof.acceptedCloseoutRefs).toHaveLength(2)
    expect(proof.rejectedCloseoutRefs).toEqual([
      'probe_closeout.shc_harbor.db_wal_recovery.20260608',
    ])
    expect(proof.progressRefs.length).toBeGreaterThanOrEqual(6)
    expect(proof.artifactRefs).toContain(
      'artifact_manifest.probe.shc_harbor.db_wal_recovery.20260608',
    )
    expect(proof.proofBundleRefs).toContain(
      'proof_bundle.probe.shc_harbor.db_wal_recovery.20260608',
    )
    expect(proof.resourceUsageRefs).toContain(
      'resource_usage_unavailable.probe.benchmark_run_probe_shc_harbor_db_wal_recovery_20260608',
    )
    expect(proof.verifierResultRefs).toContain(
      'verifier_result.terminal_bench.db_wal_recovery.shc_harbor.20260608.reward_0',
    )
    expect(proof.paymentModes).toEqual(['rejected_no_pay', 'unpaid_smoke'])
    expect(proof.noPaidWorkClaim).toBe(true)
    expect(proof.noSettlementClaim).toBe(true)
    expect(proof.noAutomaticPromotionClaim).toBe(true)
    expect(
      proof.coordinatorImports.every(
        coordinatorImport =>
          !coordinatorImport.payableWorkClaimAllowed &&
          !coordinatorImport.settledBitcoinPayoutClaimAllowed,
      ),
    ).toBe(true)
  })

  test('rejects payable or settled overclaims in the unpaid proof', () => {
    const proof = buildProbeGepaUnpaidPylonLeaseProof()

    expect(() =>
      assertProbeGepaUnpaidPylonLeaseProofSafe(
        new ProbeGepaUnpaidPylonLeaseProof({
          ...proof,
          paymentModes: ['payable_pending_settlement', 'unpaid_smoke'],
        }),
      ),
    ).toThrow(ProbeGepaUnpaidPylonLeaseProofUnsafe)

    expect(() =>
      assertProbeGepaUnpaidPylonLeaseProofSafe(
        new ProbeGepaUnpaidPylonLeaseProof({
          ...proof,
          noSettlementClaim: false,
        }),
      ),
    ).toThrow(ProbeGepaUnpaidPylonLeaseProofUnsafe)
  })

  test('rejects private refs in demo worker lease proof projections', () => {
    const proof = buildProbeGepaUnpaidPylonLeaseProof()

    expect(() =>
      assertProbeGepaUnpaidPylonLeaseProofSafe(
        new ProbeGepaUnpaidPylonLeaseProof({
          ...proof,
          artifactRefs: ['raw_runner_log.private'],
        }),
      ),
    ).toThrow(ProbeGepaUnpaidPylonLeaseProofUnsafe)
  })
})
