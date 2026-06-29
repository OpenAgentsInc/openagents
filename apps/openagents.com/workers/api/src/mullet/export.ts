import {
  MulletSimulationRunExport,
  type MulletSimulationRunExport as MulletSimulationRunExportType,
} from '@openagentsinc/mullet-schema'
import { Effect, Schema as S } from 'effect'

import type { MulletSimulationRunRecord } from './repository'

export const MulletExportFormat = S.Literals(['json', 'markdown'])
export type MulletExportFormat = typeof MulletExportFormat.Type

export class MulletExportRedactionError extends S.TaggedErrorClass<MulletExportRedactionError>()(
  'MulletExportRedactionError',
  {
    path: S.String,
    reason: S.String,
  },
) {}

export type MulletRunExportBuild = Readonly<{
  content: string | Record<string, unknown>
  runExport: MulletSimulationRunExportType
}>

type ValueStateCounts = Readonly<{
  accepted: number
  measured: number
  modeled: number
  paid: number
  settled: number
  verified: number
}>

const unsafeKeyFragments = [
  'customer_data',
  'customerdata',
  'invoice',
  'payment_preimage',
  'paymentpreimage',
  'preimage',
  'private_artifact',
  'privateartifact',
  'private_repo',
  'privaterepo',
  'provider_secret',
  'providersecret',
  'raw_log',
  'rawlog',
  'raw_prompt',
  'rawprompt',
  'raw_timestamp',
  'rawtimestamp',
  'raw_trace',
  'rawtrace',
  'secret',
  'wallet',
  'wallet_material',
  'walletmaterial',
]

const unsafeStringFragments = [
  'bolt11',
  'customer data',
  'github_pat_',
  'gho_',
  'invoice secret',
  'lnbc',
  'payment preimage',
  'private artifact',
  'private repo',
  'provider secret',
  'raw log',
  'raw prompt',
  'raw timestamp',
  'raw trace',
  'sk_live_',
  'wallet material',
  'wallet mnemonic',
  'xoxb-',
]

const normalizeScanText = (value: string): string =>
  value.toLowerCase().replaceAll('-', '_').replaceAll(' ', '_')

const unsafePayloadReason = (
  value: unknown,
  path: ReadonlyArray<string>,
): MulletExportRedactionError | undefined => {
  const pathKey = normalizeScanText(path.at(-1) ?? '')

  if (
    pathKey !== '' &&
    unsafeKeyFragments.some(fragment => pathKey.includes(fragment))
  ) {
    return new MulletExportRedactionError({
      path: path.join('.'),
      reason: 'unsafe_key',
    })
  }

  if (typeof value === 'string') {
    const text = value.toLowerCase()
    const normalizedText = normalizeScanText(value)

    if (
      unsafeStringFragments.some(
        fragment =>
          text.includes(fragment) || normalizedText.includes(fragment),
      )
    ) {
      return new MulletExportRedactionError({
        path: path.join('.'),
        reason: 'unsafe_string',
      })
    }
  }

  return undefined
}

export const findUnsafeMulletExportPayload = (
  value: unknown,
  path: ReadonlyArray<string> = [],
): MulletExportRedactionError | undefined => {
  const directReason = unsafePayloadReason(value, path)

  if (directReason !== undefined) {
    return directReason
  }

  if (Array.isArray(value)) {
    return value
      .map((item, index) =>
        findUnsafeMulletExportPayload(item, [...path, String(index)]),
      )
      .find(reason => reason !== undefined)
  }

  if (typeof value === 'object' && value !== null) {
    return Object.entries(value)
      .map(([key, item]) => findUnsafeMulletExportPayload(item, [...path, key]))
      .find(reason => reason !== undefined)
  }

  return undefined
}

export const assertSafeMulletExportPayload = (
  value: unknown,
): Effect.Effect<void, MulletExportRedactionError> => {
  const unsafe = findUnsafeMulletExportPayload(value)

  return unsafe === undefined ? Effect.void : Effect.fail(unsafe)
}

export const buildMulletRunExport = (input: {
  readonly exportId: string
  readonly format: MulletExportFormat
  readonly generatedAt: string
  readonly runRecord: MulletSimulationRunRecord
}): Effect.Effect<MulletRunExportBuild, MulletExportRedactionError> =>
  Effect.gen(function* () {
    const valueStates = valueStateCounts(input.runRecord.run)
    const content =
      input.format === 'json'
        ? jsonExportContent(input.runRecord, valueStates)
        : markdownExportContent(input.runRecord, valueStates)

    yield* assertSafeMulletExportPayload(content)

    const runExport = S.decodeUnknownSync(MulletSimulationRunExport)({
      runId: input.runRecord.id,
      scenarioId: input.runRecord.scenarioId,
      generatedAt: input.generatedAt,
      format: input.format,
      privateVisibility: true,
      redactionStatus: 'passed',
      modeledValueCount: valueStates.modeled,
      measuredValueCount: valueStates.measured,
      acceptedValueCount: valueStates.accepted,
      paidValueCount: valueStates.paid,
      settledValueCount: valueStates.settled,
      contentRef: `mullet://private/runs/${input.runRecord.id}/exports/${input.exportId}.${input.format}`,
    })

    return { content, runExport }
  })

const valueStateCounts = (
  value: unknown,
  counts: ValueStateCounts = {
    accepted: 0,
    measured: 0,
    modeled: 0,
    paid: 0,
    settled: 0,
    verified: 0,
  },
): ValueStateCounts => {
  if (Array.isArray(value)) {
    return value.reduce<ValueStateCounts>(
      (nextCounts, item) => valueStateCounts(item, nextCounts),
      counts,
    )
  }

  if (typeof value !== 'object' || value === null) {
    return counts
  }

  const record = value as Record<string, unknown>
  const provenance = record.provenance
  const nextCounts =
    provenance === 'modeled'
      ? { ...counts, modeled: counts.modeled + 1 }
      : provenance === 'measured'
        ? { ...counts, measured: counts.measured + 1 }
        : provenance === 'verified'
          ? { ...counts, verified: counts.verified + 1 }
          : provenance === 'accepted'
            ? { ...counts, accepted: counts.accepted + 1 }
            : provenance === 'paid'
              ? { ...counts, paid: counts.paid + 1 }
              : provenance === 'settled'
                ? { ...counts, settled: counts.settled + 1 }
                : counts

  return Object.values(record).reduce<ValueStateCounts>(
    (next, item) => valueStateCounts(item, next),
    nextCounts,
  )
}

const jsonExportContent = (
  runRecord: MulletSimulationRunRecord,
  valueStates: ValueStateCounts,
): Record<string, unknown> => ({
  authority: {
    publicClaimProjection: false,
    simulationOnly: true,
  },
  dispatch: runRecord.run.dispatchResults.map(result => ({
    acceptedOutcomes: result.acceptedOutcomes,
    acceptedOutcomesPerMwh: result.acceptedOutcomesPerMwh,
    candidates: result.candidates.map(candidate => ({
      mode: candidate.mode,
      reasonCode: candidate.reasonCode,
      riskAdjustedNetUsdPerMwh: candidate.riskAdjustedNetUsdPerMwh,
    })),
    reasonCode: result.reasonCode,
    selectedMode: result.selectedMode,
  })),
  privateVisibility: true,
  proofRefs: {
    acceptedWorkProofPacketIds: runRecord.run.proofPackets.map(
      packet => packet.id,
    ),
    energyTelemetryRecordIds: runRecord.run.energyTelemetry.map(
      record => record.id,
    ),
    marketMemoryIds: runRecord.run.marketMemory.map(record => record.id),
    settlementReceiptRefs: runRecord.run.proofPackets.flatMap(packet =>
      packet.settlementReceiptRef === undefined
        ? []
        : [packet.settlementReceiptRef],
    ),
  },
  providerSettlementState: runRecord.run.providerSettlementState,
  runId: runRecord.id,
  scenarioId: runRecord.scenarioId,
  scenarioName: runRecord.run.scenario.name,
  valueStates,
})

const markdownExportContent = (
  runRecord: MulletSimulationRunRecord,
  valueStates: ValueStateCounts,
): string => {
  const firstDispatch = runRecord.run.dispatchResults[0]
  const acceptedProofIds = runRecord.run.proofPackets
    .map(packet => `- ${packet.id}`)
    .join('\n')
  const energyTelemetryIds = runRecord.run.energyTelemetry
    .map(record => `- ${record.id}`)
    .join('\n')
  const marketMemoryIds = runRecord.run.marketMemory
    .map(record => `- ${record.id}`)
    .join('\n')
  const settlementRefs = runRecord.run.proofPackets
    .flatMap(packet =>
      packet.settlementReceiptRef === undefined
        ? []
        : [`- ${packet.settlementReceiptRef}`],
    )
    .join('\n')

  return [
    '# Private Mullet Simulation Export',
    '',
    'Visibility: private',
    'Public claim projection: no',
    'Authority: simulation-only; no live work assignment, provider mutation, wallet spend, payout settlement, or public promotion is authorized by this packet.',
    '',
    `Run: ${runRecord.id}`,
    `Scenario: ${runRecord.run.scenario.name}`,
    `Selected dispatch: ${firstDispatch?.selectedMode ?? 'none'}`,
    `Reason: ${firstDispatch?.reasonCode ?? 'none'}`,
    '',
    '## Value states',
    `- Modeled: ${valueStates.modeled}`,
    `- Measured: ${valueStates.measured}`,
    `- Verified: ${valueStates.verified}`,
    `- Accepted: ${valueStates.accepted}`,
    `- Paid: ${valueStates.paid}`,
    `- Settled: ${valueStates.settled}`,
    '',
    '## Attached refs',
    'Accepted-work proof packet refs:',
    acceptedProofIds === '' ? '- none attached' : acceptedProofIds,
    '',
    'Energy telemetry refs:',
    energyTelemetryIds === '' ? '- none attached' : energyTelemetryIds,
    '',
    'Settlement refs:',
    settlementRefs === '' ? '- none attached' : settlementRefs,
    '',
    'Market-memory refs:',
    marketMemoryIds === '' ? '- none attached' : marketMemoryIds,
    '',
    'Market-memory updates are modeled separately from runtime truth and must not be treated as settlement evidence.',
  ].join('\n')
}
