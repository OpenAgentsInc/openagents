import { describe, expect, test } from 'vitest'

import type { AutopilotWorkProjection } from '../model'
import {
  buildForgeArtifactReceiptIndexInput,
  projectForgeArtifactReceiptIndex,
} from './artifact-receipt-index'

const baseInput = {
  generatedAt: '2026-06-18T02:00:00.000Z',
  snapshotRef: 'artifact-receipt-index-snapshot.public.work_1',
  versionRef: 'artifact-receipt-index-version.public.v1',
  workOrderRef: 'work.public.work_1',
}

describe('Forge artifact and receipt index projection', () => {
  test('projects public artifact and receipt evidence as refs-only non-authoritative state', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      artifacts: [
        {
          artifactRef: 'artifact.public.work_1.diff_summary',
          digestRefs: ['digest.public.artifact.sha256'],
          freshness: 'fresh',
          kind: 'diff',
          mediaTypeRefs: ['media-type.public.text_markdown'],
          policyRefs: ['policy.public.artifact.redacted'],
          producerRefs: ['producer.public.pylon.local'],
          redactionClass: 'public_safe',
          relatedReceiptRefs: ['receipt.public.delivery.work_1'],
          retentionRefs: ['retention.public.receipt_index'],
          runRefs: ['run.public.work_1'],
          sizeRefs: ['size.public.artifact.2048'],
          summaryRefs: ['summary.public.diff'],
          visibility: 'public',
          workOrderRefs: ['work-order.public.work_1'],
        },
      ],
      receipts: [
        {
          actorRefs: ['actor.public.agent'],
          caveatRefs: ['caveat.public.summary_only'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.delivery.work_1'],
          inputRefs: ['artifact.public.work_1.diff_summary'],
          outputRefs: ['receipt-output.public.delivery'],
          policyRefs: ['policy.public.delivery'],
          receiptRef: 'receipt.public.delivery.work_1',
          serviceRefs: ['service.public.autopilot'],
          subjectRefs: ['artifact.public.work_1.diff_summary'],
          transitionKind: 'delivery',
          verificationRefs: ['verification.public.bun_test'],
        },
      ],
    })

    expect(view.status).toBe('ready')
    expect(view.publicSafe).toBe(true)
    expect(view.counts).toEqual({
      artifacts: 1,
      publicArtifacts: 1,
      receipts: 1,
      stale: 0,
    })
    expect(view.blockerRefs).toEqual([])
    expect(view.authority).toEqual({
      acceptedOutcomeAuthority: false,
      artifactDeleteAuthority: false,
      artifactDownloadAuthority: false,
      artifactStoreAuthority: false,
      claimSatisfactionAuthority: false,
      deploymentAuthority: false,
      publicClaimAuthority: false,
      receiptAppendAuthority: false,
      receiptRevokeAuthority: false,
      settlementAuthority: false,
      visibilityWidenAuthority: false,
      workerPayoutAuthority: false,
    })
  })

  test('treats missing artifact and receipt state as empty', () => {
    const view = projectForgeArtifactReceiptIndex({
      generatedAt: '2026-06-18T02:00:00.000Z',
      workOrderRef: 'work.public.empty',
    })

    expect(view.status).toBe('empty')
    expect(view.artifacts).toEqual([])
    expect(view.receipts).toEqual([])
    expect(view.blockerRefs).toEqual([])
  })

  test('blocks stale artifact and receipt evidence', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      artifacts: [
        {
          artifactRef: 'artifact.public.stale',
          freshness: 'stale',
          kind: 'test_result',
          relatedReceiptRefs: ['receipt.public.stale'],
          visibility: 'team',
        },
      ],
      receipts: [
        {
          freshness: 'stale',
          idempotencyRefs: ['idempotency.public.stale'],
          policyRefs: ['policy.public.stale'],
          receiptRef: 'receipt.public.stale',
          subjectRefs: ['artifact.public.stale'],
          transitionKind: 'verification',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:stale-artifact-evidence:artifact.public.stale',
    )
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:stale-receipt-evidence:receipt.public.stale',
    )
  })

  test('blocks public artifacts without digest redaction and policy refs', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      artifacts: [
        {
          artifactRef: 'artifact.public.unverified',
          freshness: 'fresh',
          kind: 'patch',
          redactionClass: 'private_ref',
          relatedReceiptRefs: ['receipt.public.delivery'],
          visibility: 'public',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:public-artifact-evidence-missing:artifact.public.unverified',
    )
  })

  test('blocks artifacts without related receipt refs', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      artifacts: [
        {
          artifactRef: 'artifact.team.unlinked',
          freshness: 'fresh',
          kind: 'preview',
          visibility: 'team',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:artifact-receipt-link-missing:artifact.team.unlinked',
    )
  })

  test('blocks receipts without subject idempotency and policy refs', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      receipts: [
        {
          freshness: 'fresh',
          receiptRef: 'receipt.public.incomplete',
          transitionKind: 'execution',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:receipt-contract-incomplete:receipt.public.incomplete',
    )
  })

  test('blocks payment receipts that claim accepted outcome satisfaction', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      receipts: [
        {
          claimRequirementRefs: ['claim.public.accepted_outcome'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.payment'],
          policyRefs: ['policy.public.payment'],
          receiptRef: 'receipt.public.payment',
          satisfyingReceiptRefs: ['receipt.public.acceptance'],
          subjectRefs: ['settlement.public.work_1'],
          transitionKind: 'payment',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:payment-receipt-cannot-satisfy-acceptance:receipt.public.payment',
    )
  })

  test('blocks PR draft receipts that claim final delivery satisfaction', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      receipts: [
        {
          claimRequirementRefs: ['claim.public.customer_acceptance'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.pr_draft'],
          policyRefs: ['policy.public.pr_draft'],
          receiptRef: 'receipt.public.pr_draft',
          satisfyingReceiptRefs: ['receipt.public.merge'],
          subjectRefs: ['pr-draft.public.work_1'],
          transitionKind: 'pr_draft',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:pr-draft-receipt-cannot-satisfy-final-claim:receipt.public.pr_draft',
    )
  })

  test('blocks unsatisfied claim requirements', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      receipts: [
        {
          claimRequirementRefs: ['claim.public.delivery_verified'],
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.delivery'],
          policyRefs: ['policy.public.delivery'],
          receiptRef: 'receipt.public.delivery',
          subjectRefs: ['artifact.public.delivery'],
          transitionKind: 'delivery',
        },
      ],
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:claim-requirement-unsatisfied:receipt.public.delivery',
    )
  })

  test('blocks populated artifact and receipt entries without snapshot refs', () => {
    const view = projectForgeArtifactReceiptIndex({
      artifacts: [
        {
          artifactRef: 'artifact.team.no_snapshot',
          freshness: 'fresh',
          kind: 'preview',
          relatedReceiptRefs: ['receipt.team.no_snapshot'],
          visibility: 'team',
        },
      ],
      generatedAt: '2026-06-18T02:00:00.000Z',
      workOrderRef: 'work.public.no_snapshot',
    })

    expect(view.status).toBe('blocked')
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.no_snapshot:missing-artifact-receipt-index-snapshot-ref',
    )
  })

  test('omits unsafe private artifact and receipt material before projection', () => {
    const view = projectForgeArtifactReceiptIndex({
      ...baseInput,
      artifacts: [
        {
          artifactRef: 'artifact.public.safe',
          digestRefs: ['digest.public.safe'],
          freshness: 'fresh',
          kind: 'diff',
          mediaTypeRefs: [
            'media-type.public.text_markdown',
            'raw artifact /Users/christopher/a.bin',
          ],
          policyRefs: ['policy.public.safe'],
          producerRefs: ['producer.public.safe', 'provider payload sk-private'],
          redactionClass: 'public_safe',
          relatedReceiptRefs: ['receipt.public.safe'],
          summaryRefs: ['summary.public.safe', 'raw patch /Users/christopher/a.diff'],
          visibility: 'public',
        },
      ],
      blockerRefs: [
        'artifact-index-blocker.public.safe',
        'raw artifact /Users/christopher/artifact.log',
      ],
      receipts: [
        {
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.safe'],
          inputRefs: [
            'artifact.public.safe',
            'raw receipt /Users/christopher/receipt.json',
          ],
          outputRefs: ['receipt-output.public.safe'],
          policyRefs: ['policy.public.safe', 'bearer token private'],
          receiptRef: 'receipt.public.safe',
          serviceRefs: ['service.public.safe'],
          subjectRefs: ['artifact.public.safe'],
          transitionKind: 'delivery',
          verificationRefs: ['verification.public.safe'],
        },
      ],
    })
    const payload = JSON.stringify(view)

    expect(view.status).toBe('blocked')
    expect(view.artifacts[0]?.mediaTypeRefs).toEqual([
      'media-type.public.text_markdown',
    ])
    expect(view.receipts[0]?.policyRefs).toEqual(['policy.public.safe'])
    expect(view.blockerRefs).toContain(
      'forge-artifact-receipt-index-blocker:work.public.work_1:unsafe-artifact-receipt-material-omitted',
    )
    expect(payload).not.toContain('/Users/christopher')
    expect(payload).not.toContain('raw artifact')
    expect(payload).not.toContain('raw patch')
    expect(payload).not.toContain('raw receipt')
    expect(payload).not.toContain('provider payload')
    expect(payload).not.toContain('bearer token')
    expect(payload).not.toContain('sk-private')
  })

  test('builds input from optional Run projection fields', () => {
    const work = {
      artifactReceiptIndex: {
        artifacts: [
          {
            artifactRef: 'artifact.public.work_2.diff',
            digestRefs: ['digest.public.work_2'],
            freshness: 'fresh',
            kind: 'diff',
            policyRefs: ['policy.public.work_2'],
            redactionClass: 'public_safe',
            relatedReceiptRefs: ['receipt.public.work_2.delivery'],
            visibility: 'public',
          },
        ],
        generatedAt: '2026-06-18T02:01:00.000Z',
        receipts: [
          {
            freshness: 'fresh',
            idempotencyRefs: ['idempotency.public.work_2'],
            policyRefs: ['policy.public.work_2'],
            receiptRef: 'receipt.public.work_2.delivery',
            subjectRefs: ['artifact.public.work_2.diff'],
            transitionKind: 'delivery',
          },
        ],
        snapshotRef: 'artifact-receipt-index-snapshot.public.work_2',
        versionRef: 'artifact-receipt-index-version.public.v2',
      },
      generatedAt: '2026-06-18T02:00:00.000Z',
      workOrderRef: 'work.public.work_2',
    } as unknown as AutopilotWorkProjection

    expect(buildForgeArtifactReceiptIndexInput(work)).toEqual({
      artifacts: [
        {
          artifactRef: 'artifact.public.work_2.diff',
          digestRefs: ['digest.public.work_2'],
          freshness: 'fresh',
          kind: 'diff',
          policyRefs: ['policy.public.work_2'],
          redactionClass: 'public_safe',
          relatedReceiptRefs: ['receipt.public.work_2.delivery'],
          visibility: 'public',
        },
      ],
      generatedAt: '2026-06-18T02:01:00.000Z',
      receipts: [
        {
          freshness: 'fresh',
          idempotencyRefs: ['idempotency.public.work_2'],
          policyRefs: ['policy.public.work_2'],
          receiptRef: 'receipt.public.work_2.delivery',
          subjectRefs: ['artifact.public.work_2.diff'],
          transitionKind: 'delivery',
        },
      ],
      snapshotRef: 'artifact-receipt-index-snapshot.public.work_2',
      versionRef: 'artifact-receipt-index-version.public.v2',
      workOrderRef: 'work.public.work_2',
    })
  })
})
