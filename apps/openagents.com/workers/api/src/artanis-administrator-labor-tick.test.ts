import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  runArtanisAdminLaborRequestTickScheduled,
} from './artanis-administrator-tick'
import { makeInMemoryArtanisLaborUnattendedReceiptStore } from './artanis-labor-receipt-store'
import type {
  ArtanisLaborRequestProposal,
  ArtanisLaborRequesterDeps,
} from './artanis-labor-requester'

const proposal = (): ArtanisLaborRequestProposal => ({
  budgetSats: 2_000,
  deadlineRef: 'deadline.public.artanis_labor.issue_6870',
  objectiveRef: 'objective.public.artanis_labor.issue_6870',
  repositoryRefs: ['repo.public.github.OpenAgentsInc.openagents'],
  requiredCapabilityRefs: ['capability.pylon.local_codex'],
  title: 'Artanis unattended labor request',
  verificationCommandRef: 'command.public.pylon_khala.verify.5ae2dab38b186855ec74c348',
})

const requesterDeps = (
  overrides?: Partial<Omit<ArtanisLaborRequesterDeps, 'enabled'>>,
): Omit<ArtanisLaborRequesterDeps, 'enabled'> => ({
  alreadyReservedThisTickMsat: 0,
  artanisActorRef: 'agent:artanis',
  nowIso: '2026-06-28T12:00:00.000Z',
  perTickBudgetMsat: 10_000_000,
  propose: async () => proposal(),
  recordTickReceipt: async () => {},
  reserveEscrow: async () => ({
    ok: true,
    reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_issue_6870',
  }),
  seedBalanceAvailableMsat: 10_000_000,
  submitWorkRequest: async () => ({
    jobEventId: 'b'.repeat(64),
    topicId: 'topic_artanis_labor_issue_6870',
    workRequestId: 'work_request_artanis_labor_issue_6870',
  }),
  ...overrides,
})

describe('Artanis administrator request_labor scheduled action', () => {
  test('enabled tick publishes one bounded request, reserves escrow, and stores a placed receipt', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    const calls = { propose: 0, reserve: 0, submit: 0 }
    const outcome = await Effect.runPromise(
      runArtanisAdminLaborRequestTickScheduled({
        artanisActorRef: 'agent:artanis',
        enabled: true,
        requesterDeps: requesterDeps({
          propose: async () => {
            calls.propose += 1
            return proposal()
          },
          reserveEscrow: async () => {
            calls.reserve += 1
            return {
              ok: true,
              reserveReceiptRef: 'receipt.labor_escrow.reserve.artanis_issue_6870',
            }
          },
          submitWorkRequest: async () => {
            calls.submit += 1
            return {
              jobEventId: 'b'.repeat(64),
              topicId: 'topic_artanis_labor_issue_6870',
              workRequestId: 'work_request_artanis_labor_issue_6870',
            }
          },
        }),
        store,
        tickRef: 'tick.public.artanis.issue_6870.enabled',
      }),
    )

    expect(calls).toEqual({ propose: 1, reserve: 1, submit: 1 })
    expect(outcome.state).toBe('placed')
    expect(outcome.terminalState).toBe('requested_pending_delivery')
    expect(outcome.workRequestId).toBe('work_request_artanis_labor_issue_6870')
    expect(outcome.receiptRef).toMatch(/^receipt\.artanis_labor\.unattended_request\.[a-f0-9]{16}$/)
    expect(await store.list()).toHaveLength(1)
  })

  test('disabled labor gate seals skipped_config_disabled and never counts as placed work', async () => {
    const store = makeInMemoryArtanisLaborUnattendedReceiptStore()
    let proposed = false
    const outcome = await Effect.runPromise(
      runArtanisAdminLaborRequestTickScheduled({
        artanisActorRef: 'agent:artanis',
        enabled: false,
        requesterDeps: requesterDeps({
          propose: async () => {
            proposed = true
            return proposal()
          },
        }),
        store,
        tickRef: 'tick.public.artanis.issue_6870.disabled',
      }),
    )

    const stored = await store.list()
    expect(proposed).toBe(false)
    expect(outcome.state).toBe('skipped')
    expect(outcome.reason).toBe('config_disabled')
    expect(outcome.terminalState).toBe('skipped_config_disabled')
    expect(outcome.workRequestId).toBeNull()
    expect(stored).toHaveLength(1)
    expect(stored[0]?.receipt.terminalState).toBe('skipped_config_disabled')
    expect(stored[0]?.receipt.budgetMsat).toBeNull()
  })
})
