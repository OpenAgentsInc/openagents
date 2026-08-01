#!/usr/bin/env -S pnpm exec tsx

import { makeSarahRealtimeVoiceStore } from '@openagentsinc/khala-sync-server'
import { createHash } from 'node:crypto'
import { realpathSync, writeFileSync } from 'node:fs'
import { basename, dirname, isAbsolute, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { defaultMakeKhalaSyncSqlClient } from '../src/khala-sync-push-routes'

export const OWNER_GATE = 'I_APPROVE_SARAH_UNMETERED_ACCOUNTING_WAIVER'
export const OPERATOR_ACTOR_REF = 'operator:owner_sarah_unmetered_waiver'
export const REPOSITORY_ROOT = realpathSync(
  fileURLToPath(new URL('../../../../../', import.meta.url)),
)

export type Arguments = Readonly<{
  apply: boolean
  environment: 'production'
  expectedTargetCount: number | undefined
  expectedTargetSetDigest: string | undefined
  privateOutput: string
  reason: string
  providerEvidenceRefs: ReadonlyArray<string>
}>

export type WaiverTarget = Readonly<{
  sessionRef: string
  generation: number
  providerSessionRefDigest: string
  reservedMsat: number
  recordedChargeMsat: number
}>

const usage =
  'usage: waive-sarah-accounting-uncertain.ts [--apply] --environment production ' +
  '--private-output /absolute/outside/repo.json --reason TEXT ' +
  '--evidence-ref REF [--evidence-ref REF ...] ' +
  '[--expected-target-count INTEGER --expected-target-set-digest SHA256]'

const sha256 = (value: string): string =>
  createHash('sha256').update(value).digest('hex')

const requiredEnv = (name: string): string => {
  const value = process.env[name]?.trim()
  if (value === undefined || value === '') {
    throw new Error(`missing required environment variable ${name}`)
  }
  return value
}

export const parseArguments = (
  argv: ReadonlyArray<string>,
  repositoryRoot = REPOSITORY_ROOT,
): Arguments => {
  let apply = false
  let environment: string | undefined
  let expectedTargetCountText: string | undefined
  let expectedTargetSetDigest: string | undefined
  let privateOutput: string | undefined
  let reason: string | undefined
  const providerEvidenceRefs: string[] = []
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (argument === '--apply') {
      apply = true
      continue
    }
    const next = argv[index + 1]
    if (next === undefined) throw new Error(usage)
    index += 1
    switch (argument) {
      case '--environment':
        environment = next
        break
      case '--expected-target-count':
        expectedTargetCountText = next
        break
      case '--expected-target-set-digest':
        expectedTargetSetDigest = next
        break
      case '--private-output':
        privateOutput = next
        break
      case '--reason':
        reason = next
        break
      case '--evidence-ref':
        providerEvidenceRefs.push(next)
        break
      default:
        throw new Error(usage)
    }
  }
  const expectedTargetCount =
    expectedTargetCountText === undefined
      ? undefined
      : Number(expectedTargetCountText)
  const sortedEvidence = [...providerEvidenceRefs].sort()
  const canonicalRepositoryRoot = realpathSync(repositoryRoot)
  const resolvedPrivateOutput =
    privateOutput === undefined
      ? undefined
      : join(
          realpathSync(dirname(resolve(privateOutput))),
          basename(privateOutput),
        )
  if (
    environment !== 'production' ||
    resolvedPrivateOutput === undefined ||
    !isAbsolute(privateOutput ?? '') ||
    (resolvedPrivateOutput !== undefined &&
      (resolvedPrivateOutput === canonicalRepositoryRoot ||
        resolvedPrivateOutput.startsWith(`${canonicalRepositoryRoot}/`))) ||
    reason === undefined ||
    reason.length < 1 ||
    reason.length > 1_024 ||
    sortedEvidence.length < 1 ||
    sortedEvidence.length > 16 ||
    new Set(sortedEvidence).size !== sortedEvidence.length ||
    sortedEvidence.some(
      reference => reference.length < 1 || reference.length > 512,
    ) ||
    (expectedTargetCount !== undefined &&
      (!Number.isSafeInteger(expectedTargetCount) ||
        expectedTargetCount < 0)) ||
    (expectedTargetSetDigest !== undefined &&
      !/^[0-9a-f]{64}$/u.test(expectedTargetSetDigest)) ||
    (apply &&
      (expectedTargetCount === undefined ||
        expectedTargetSetDigest === undefined))
  ) {
    throw new Error(usage)
  }
  return {
    apply,
    environment,
    expectedTargetCount,
    expectedTargetSetDigest,
    privateOutput: resolvedPrivateOutput,
    reason,
    providerEvidenceRefs: sortedEvidence,
  }
}

export const targetSetDigest = (targets: ReadonlyArray<WaiverTarget>): string =>
  sha256(
    JSON.stringify(
      [...targets]
        .sort((left, right) => left.sessionRef.localeCompare(right.sessionRef))
        .map(target => ({
          sessionRef: target.sessionRef,
          generation: target.generation,
          providerSessionRefDigest: target.providerSessionRefDigest,
        })),
    ),
  )

export const assertExpectedTargets = (
  targets: ReadonlyArray<WaiverTarget>,
  expectedCount: number,
  expectedDigest: string,
): void => {
  const digest = targetSetDigest(targets)
  if (targets.length !== expectedCount || digest !== expectedDigest) {
    throw new Error(
      `target set changed: observed count=${targets.length} digest=${digest}`,
    )
  }
}

export const assertOwnerGate = (
  apply: boolean,
  gate: string | undefined,
): void => {
  if (apply && gate?.trim() !== OWNER_GATE) {
    throw new Error(
      `set SARAH_ACCOUNTING_WAIVER_OWNER_GATE=${OWNER_GATE} to release uncertain holds`,
    )
  }
}

export const publicReceiptFor = (
  input: Pick<Arguments, 'apply' | 'environment'>,
  targets: ReadonlyArray<WaiverTarget>,
  results: ReadonlyArray<Readonly<{ waiverReceiptRef: string }>>,
  privateReceipt: unknown,
) => ({
  schema: 'openagents.sarah.accounting-waiver-batch.public.v1' as const,
  environment: input.environment,
  mode: input.apply ? ('applied' as const) : ('preview' as const),
  targetCount: targets.length,
  targetSetDigest: targetSetDigest(targets),
  reservedMsat: targets.reduce((sum, target) => sum + target.reservedMsat, 0),
  recordedChargeMsat: targets.reduce(
    (sum, target) => sum + target.recordedChargeMsat,
    0,
  ),
  receiptDigests: results.map(result => sha256(result.waiverReceiptRef)),
  privateReceiptDigest: sha256(JSON.stringify(privateReceipt)),
})

export const waiverPayloadDigest = (
  target: WaiverTarget,
  input: Pick<Arguments, 'reason' | 'providerEvidenceRefs'>,
): string =>
  sha256(
    JSON.stringify({
      authority: 'owner_waived_unmetered_v1',
      operatorActorRef: OPERATOR_ACTOR_REF,
      sessionRef: target.sessionRef,
      generation: target.generation,
      providerSessionRefDigest: target.providerSessionRefDigest,
      reason: input.reason,
      providerEvidenceRefs: input.providerEvidenceRefs,
    }),
  )

const main = async (): Promise<void> => {
  const input = parseArguments(process.argv.slice(2))
  const client = await defaultMakeKhalaSyncSqlClient(
    requiredEnv('KHALA_SYNC_DATABASE_URL'),
  )
  try {
    const expectedDatabase = requiredEnv(
      'SARAH_ACCOUNTING_WAIVER_EXPECTED_PRODUCTION_DATABASE',
    )
    const databaseRows = await client.sql<
      ReadonlyArray<{ database_name: string }>
    >`
      SELECT current_database() AS database_name
    `
    if (databaseRows[0]?.database_name !== expectedDatabase) {
      throw new Error(
        'refusing Sarah accounting waiver against the wrong database',
      )
    }
    const rows = await client.sql<
      ReadonlyArray<{
        session_ref: string
        generation: number | string
        provider_session_ref_digest: string
        reserved_msat: number | string
        charged_msat: number | string
      }>
    >`
      SELECT session.session_ref, session.generation,
        binding.provider_session_ref_digest, session.reserved_msat,
        session.charged_msat
      FROM sarah_realtime_voice_sessions AS session
      INNER JOIN sarah_livekit_room_bindings AS binding
        ON binding.session_ref = session.session_ref
        AND binding.generation = session.generation
      LEFT JOIN sarah_voice_accounting_waivers AS waiver
        ON waiver.session_ref = session.session_ref
      WHERE session.state = 'accounting_uncertain'
        AND session.transport_kind = 'livekit_room_v1'
        AND session.credit_mode = 'metered'
        AND binding.provider_accounting_status = 'uncertain'
        AND binding.provider_session_ref_digest IS NOT NULL
        AND waiver.session_ref IS NULL
      ORDER BY session.session_ref
      FOR UPDATE OF session
    `
    const targets = rows.map(row => ({
      sessionRef: row.session_ref,
      generation: Number(row.generation),
      providerSessionRefDigest: row.provider_session_ref_digest,
      reservedMsat: Number(row.reserved_msat),
      recordedChargeMsat: Number(row.charged_msat),
    }))
    const digest = targetSetDigest(targets)
    if (
      input.expectedTargetCount !== undefined &&
      input.expectedTargetSetDigest !== undefined
    ) {
      assertExpectedTargets(
        targets,
        input.expectedTargetCount,
        input.expectedTargetSetDigest,
      )
    }
    assertOwnerGate(input.apply, process.env.SARAH_ACCOUNTING_WAIVER_OWNER_GATE)
    const results = []
    if (input.apply) {
      const store = makeSarahRealtimeVoiceStore(client.sql)
      for (const target of targets) {
        const payloadDigest = waiverPayloadDigest(target, input)
        // Deliberately sequential: each durable receipt commits independently.
        // eslint-disable-next-line no-await-in-loop
        results.push(
          // eslint-disable-next-line no-await-in-loop
          await store.waiveLiveKitAccounting({
            waiverRef: `sarah_voice_owner_waiver:${payloadDigest}`,
            waiverPayloadDigest: payloadDigest,
            sessionRef: target.sessionRef,
            generation: target.generation,
            providerSessionRefDigest: target.providerSessionRefDigest,
            operatorActorRef: OPERATOR_ACTOR_REF,
            reason: input.reason,
            providerEvidenceRefs: input.providerEvidenceRefs,
            nowIso: new Date().toISOString(),
          }),
        )
      }
    }
    const privateReceipt = {
      schema: 'openagents.sarah.accounting-waiver-batch.private.v1',
      environment: input.environment,
      mode: input.apply ? 'applied' : 'preview',
      targetSetDigest: digest,
      targets,
      results,
    }
    writeFileSync(
      input.privateOutput,
      `${JSON.stringify(privateReceipt, null, 2)}\n`,
      {
        encoding: 'utf8',
        flag: 'wx',
        mode: 0o600,
      },
    )
    process.stdout.write(
      `${JSON.stringify(publicReceiptFor(input, targets, results, privateReceipt))}\n`,
    )
  } finally {
    await client.close()
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  await main()
}
