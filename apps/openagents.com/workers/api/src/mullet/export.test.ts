import { decodeMulletSimulationRun } from '@openagentsinc/mullet-schema'
import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  MulletExportRedactionError,
  assertSafeMulletExportPayload,
  buildMulletRunExport,
} from './export'
import type {
  MulletProvenanceSummary,
  MulletSimulationRunRecord,
} from './repository'
import {
  mulletFixtureTimestamp,
  simulationRunFixture,
} from './test-fixtures.test-support'

const provenanceSummary: MulletProvenanceSummary = {
  acceptedValueCount: 1,
  measuredValueCount: 1,
  modeledValueCount: 1,
  needsDiligenceCount: 0,
  paidValueCount: 0,
  settledValueCount: 1,
  sourceRefCount: 1,
}

const runRecord = (): MulletSimulationRunRecord => {
  const run = simulationRunFixture()
  const attachedRun = decodeMulletSimulationRun({
    ...run,
    dispatchResults: run.dispatchResults.map(result => ({
      ...result,
      energyTelemetryRecordIds: ['energy_telemetry_1'],
      marketMemoryUpdateIds: ['market_memory_1'],
      proofPacketIds: ['proof_packet_1'],
    })),
    energyTelemetry: [
      {
        id: 'energy_telemetry_1',
        timestamp: mulletFixtureTimestamp,
        siteId: 'site_repo_fixture',
        nodeId: 'node_repo_fixture',
        workId: 'work_1',
        powerKw: 1.6,
        energyKwh: 1.6,
        powerDataState: 'measured',
        gridSignal: 'none',
        curtailmentOrShiftAction: 'none',
        priceCounterfactual: 'modeled',
        emissionsCounterfactual: 'modeled',
        customerImpact: 'none',
        payoutUsd: 6,
        marginUsd: 4.5,
        provenance: 'measured',
      },
    ],
    marketMemory: [
      {
        id: 'market_memory_1',
        nodeId: 'node_repo_fixture',
        siteId: 'site_repo_fixture',
        workClassId: 'work_class_repo_fixture',
        acceptedCount: 5,
        rejectedCount: 1,
        acceptanceProbability: 0.83,
        medianRuntimeSeconds: 300,
        medianPayoutSeconds: 120,
        payoutSuccessRate: 1,
        repeatProviderScore: 0.8,
        repeatBuyerScore: 0.7,
        validatorReliabilityScore: 0.9,
        commonFailureModes: [],
        lastUpdated: mulletFixtureTimestamp,
      },
    ],
    powerDataState: 'measured',
    proofPackets: [
      {
        id: 'proof_packet_1',
        workId: 'work_1',
        workClassId: 'work_class_repo_fixture',
        nodeId: 'node_repo_fixture',
        nodeCapabilitySnapshotRef: 'node_capability_snapshot_1',
        assignmentId: 'assignment_1',
        executionArtifactRef: 'execution_artifact_ref_1',
        validatorVerdictRef: 'validator_verdict_ref_1',
        acceptedCloseoutRef: 'accepted_closeout_ref_1',
        buyerPriceUsd: 2.5,
        providerPayoutUsd: 1.2,
        settlementReceiptRef: 'settlement_receipt_1',
        routingConsequence: 'attached_from_existing_proof',
        provenance: 'accepted',
      },
    ],
    providerSettlementState: 'settled_bitcoin',
  })

  return {
    id: attachedRun.id,
    ownerEmail: attachedRun.ownerEmail,
    ownerUserId: attachedRun.ownerUserId,
    run: attachedRun,
    scenarioId: attachedRun.scenarioId,
    schemaVersion: attachedRun.scenario.schemaVersion,
    sourceRefs: attachedRun.scenario.sourceRefs,
    provenanceSummary,
    visibility: 'private',
    exportRedactionState: 'not_checked',
    createdAt: attachedRun.createdAt,
    updatedAt: attachedRun.updatedAt,
    completedAt: attachedRun.completedAt ?? null,
  }
}

describe('mullet export generation', () => {
  test('builds private JSON and Markdown exports without public claim authority', async () => {
    const record = runRecord()
    const json = await Effect.runPromise(
      buildMulletRunExport({
        exportId: 'mullet_export_json_1',
        format: 'json',
        generatedAt: mulletFixtureTimestamp,
        runRecord: record,
      }),
    )
    const markdown = await Effect.runPromise(
      buildMulletRunExport({
        exportId: 'mullet_export_markdown_1',
        format: 'markdown',
        generatedAt: mulletFixtureTimestamp,
        runRecord: record,
      }),
    )

    expect(json.runExport.privateVisibility).toBe(true)
    expect(json.runExport.redactionStatus).toBe('passed')
    expect(json.content).toMatchObject({
      authority: {
        publicClaimProjection: false,
        simulationOnly: true,
      },
      proofRefs: {
        acceptedWorkProofPacketIds: ['proof_packet_1'],
        energyTelemetryRecordIds: ['energy_telemetry_1'],
        marketMemoryIds: ['market_memory_1'],
        settlementReceiptRefs: ['settlement_receipt_1'],
      },
      valueStates: {
        accepted: 1,
        measured: 1,
        paid: 0,
        settled: 0,
        verified: 0,
      },
    })
    expect(markdown.content).toContain('Public claim projection: no')
    expect(markdown.content).toContain('- Modeled:')
    expect(markdown.content).toContain('- Measured: 1')
    expect(markdown.content).toContain('- Verified: 0')
    expect(markdown.content).toContain('- Accepted: 1')
    expect(markdown.content).toContain('proof_packet_1')
    expect(markdown.content).toContain('settlement_receipt_1')
    expect(markdown.content).toContain(
      'Market-memory updates are modeled separately from runtime truth',
    )
  })

  test.each([
    ['raw prompts', { rawPrompt: 'do not export' }],
    ['raw traces', { rawTrace: 'do not export' }],
    ['customer data', { customerData: 'private buyer record' }],
    ['private artifacts', { privateArtifactRef: 'artifact.private.raw' }],
    ['private repo refs', { privateRepoRef: 'OpenAgentsInc/private' }],
    ['wallet material', { walletMaterial: 'wallet mnemonic phrase' }],
    ['payment preimages', { paymentPreimage: 'payment preimage' }],
    ['invoices', { invoice: 'lnbc1example' }],
    ['provider secrets', { providerSecret: 'xoxb-secret' }],
    ['raw logs', { rawLog: 'raw log line' }],
    ['raw timestamps', { rawTimestamp: '2026-06-08T00:00:00.000Z' }],
  ])('rejects %s in export payloads', async (_label, payload) => {
    await expect(
      Effect.runPromise(assertSafeMulletExportPayload(payload)),
    ).rejects.toBeInstanceOf(MulletExportRedactionError)
  })
})
