import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  ArtanisGepaScheduledRunnerProofProjection,
  ArtanisGepaScheduledRunnerProofUnsafe,
  artanisProductionLaunchGateCheckInputFromGepaScheduledRunnerProof,
  exampleArtanisGepaScheduledRunnerProofRecord,
  projectArtanisGepaScheduledRunnerProof,
} from './artanis-gepa-scheduled-runner-proof'

const nowIso = '2026-06-08T06:20:00.000Z'

describe('Artanis bounded GEPA scheduled runner proof', () => {
  test('projects enabled bounded runner evidence without risky authority', () => {
    const projection = projectArtanisGepaScheduledRunnerProof(
      exampleArtanisGepaScheduledRunnerProofRecord(),
      nowIso,
    )

    expect(S.decodeUnknownSync(ArtanisGepaScheduledRunnerProofProjection)(
      projection,
    )).toEqual(projection)
    expect(projection).toMatchObject({
      assignmentDispatchAllowed: false,
      budgetMode: 'unpaid_smoke_no_spend',
      cadence: 'minute_cron_status_projection',
      enabled: true,
      forumAutoPublishAllowed: false,
      mutationAuthorityAllowed: false,
      productionSmokeCheckPassed: true,
      state: 'retained',
      stateLabel: 'Retained bounded GEPA scheduled runner proof',
    })
    expect(projection.idempotencyRefs).toEqual(
      expect.arrayContaining([
        'idempotency.public.artanis.scheduled_tick.schedule_ref',
        'idempotency.public.artanis.forum_intent.schedule_ref',
      ]),
    )
    expect(JSON.stringify(projection)).not.toMatch(
      /raw_|provider|wallet|payment_hash|lnbc|\/Users\/|2026-06-08T/,
    )
  })

  test('keeps the launch-gate check blocked until production smoke has passed', () => {
    const record = exampleArtanisGepaScheduledRunnerProofRecord()
    const passed = artanisProductionLaunchGateCheckInputFromGepaScheduledRunnerProof(
      record,
      nowIso,
      true,
    )
    const blocked = artanisProductionLaunchGateCheckInputFromGepaScheduledRunnerProof(
      record,
      nowIso,
      false,
    )

    expect(passed).toMatchObject({
      category: 'scheduled_runner',
      status: 'passed',
    })
    expect(blocked).toMatchObject({
      category: 'scheduled_runner',
      status: 'blocked',
    })
  })

  test('requires cadence, idempotency, pause, disable, freshness, and rollback refs', () => {
    const base = exampleArtanisGepaScheduledRunnerProofRecord()

    for (const partial of [
      { idempotencyRefs: [] },
      { operatorPauseRefs: [] },
      { disableCommandRefs: [] },
      { freshnessSignalRefs: [] },
      { rollbackRefs: [] },
      { forumIntentRefs: [] },
      { closeoutReceiptRefs: [] },
    ]) {
      expect(() =>
        projectArtanisGepaScheduledRunnerProof(
          {
            ...base,
            ...partial,
          },
          nowIso,
        )
      ).toThrow(ArtanisGepaScheduledRunnerProofUnsafe)
    }
  })

  test('rejects risky authority and unsafe refs', () => {
    const base = exampleArtanisGepaScheduledRunnerProofRecord()

    expect(() =>
      projectArtanisGepaScheduledRunnerProof(
        {
          ...base,
          authority: {
            ...base.authority,
            walletSpendAllowed: true,
          },
        },
        nowIso,
      )
    ).toThrow(ArtanisGepaScheduledRunnerProofUnsafe)

    expect(() =>
      projectArtanisGepaScheduledRunnerProof(
        {
          ...base,
          tickRefs: ['raw_runner_log.hidden_tick'],
        },
        nowIso,
      )
    ).toThrow(ArtanisGepaScheduledRunnerProofUnsafe)
  })
})
