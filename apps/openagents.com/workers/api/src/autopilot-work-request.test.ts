import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES,
  OPENAGENTS_AUTOPILOT_WORK_RESPONSE_FIXTURES,
  OpenAgentsAutopilotWorkRequest,
  OpenAgentsAutopilotWorkRequestUnsafe,
  OpenAgentsAutopilotWorkResponseFixture,
  autopilotGithubRepoRefForFullName,
  decodeOpenAgentsAutopilotWorkRequest,
} from './autopilot-work-request'
import { OPENAGENTS_UNSAFE_REDACTION_FIXTURES } from './redaction-regression-fixtures'

describe('OpenAgents Autopilot work request contract', () => {
  test('decodes public free-slice and paid L402 request fixtures', () => {
    const freeSlice = decodeOpenAgentsAutopilotWorkRequest(
      OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
    )
    const paid = decodeOpenAgentsAutopilotWorkRequest(
      OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[1],
    )

    expect(
      S.decodeUnknownSync(OpenAgentsAutopilotWorkRequest)(freeSlice),
    ).toEqual(freeSlice)
    expect(S.decodeUnknownSync(OpenAgentsAutopilotWorkRequest)(paid)).toEqual(
      paid,
    )
    expect(freeSlice).toMatchObject({
      mode: 'free_slice_or_paid_quote_or_l402',
      paymentPolicy: {
        buyerPaymentMode: 'free_slice',
        settlementMode: 'no_worker_payout',
      },
      placementPolicy: {
        preferredRunnerKinds: ['requester_pylon'],
        privacyTier: 'public_beta',
      },
    })
    expect(paid).toMatchObject({
      mode: 'l402',
      paymentPolicy: {
        buyerPaymentMode: 'l402',
        quoteRef: 'quote.autopilot_coder.public_patch.1',
        quotedAmountCents: 2500,
      },
      placementPolicy: {
        preferredRunnerKinds: ['openagents_shc'],
      },
    })
  })

  test('decodes response fixtures for accepted and payment-required states', () => {
    const decoded = OPENAGENTS_AUTOPILOT_WORK_RESPONSE_FIXTURES.map(fixture =>
      S.decodeUnknownSync(OpenAgentsAutopilotWorkResponseFixture)(fixture),
    )

    expect(decoded.map(fixture => fixture.state)).toEqual([
      'accepted_free_slice',
      'payment_required',
    ])
    expect(decoded[1]?.paymentChallengeRef).toBe(
      'challenge.l402.autopilot_work.public_patch.1',
    )
  })

  test('rejects prompt-only payloads without typed tasks', () => {
    expect(() =>
      decodeOpenAgentsAutopilotWorkRequest({
        prompt: 'Please just do the thing on autopilot.',
        schema: 'openagents.autopilot_work_request.v1',
      }),
    ).toThrow()
  })

  test('rejects empty task batches even when the shape is otherwise valid', () => {
    expect(() =>
      decodeOpenAgentsAutopilotWorkRequest({
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [],
      }),
    ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
  })

  test('decodes per-mission data scope from request fixtures', () => {
    const decoded = decodeOpenAgentsAutopilotWorkRequest(
      OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
    )

    expect(decoded.dataScope).toMatchObject({
      pathPrefixes: ['docs/', 'apps/openagents.com/'],
      repoRefs: ['repo.github.OpenAgentsInc.openagents'],
      toolRefs: ['tool.git_checkout', 'tool.bun_test'],
    })
    expect(autopilotGithubRepoRefForFullName('OpenAgentsInc/openagents')).toBe(
      'repo.github.OpenAgentsInc.openagents',
    )
  })

  test('decodes requested adapter intent on a typed task', () => {
    const decoded = decodeOpenAgentsAutopilotWorkRequest({
      ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
      tasks: [
        {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
          requestedAdapter: 'codex',
        },
      ],
    })

    expect(decoded.tasks[0]?.requestedAdapter).toBe('codex')
  })

  test('rejects placeholder git checkout commits', () => {
    expect(() =>
      decodeOpenAgentsAutopilotWorkRequest({
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            checkout: {
              ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0]
                .checkout,
              commitSha: '1111111111111111111111111111111111111111',
            },
          },
        ],
      }),
    ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
  })

  test('rejects data scopes that do not cover every task repository', () => {
    expect(() =>
      decodeOpenAgentsAutopilotWorkRequest({
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        dataScope: {
          pathPrefixes: [],
          repoRefs: ['repo.github.OpenAgentsInc.other_repo'],
          toolRefs: [],
        },
      }),
    ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
  })

  test('rejects unsafe data-scope paths and non-tool refs', () => {
    for (const pathPrefix of [
      '../secrets',
      '/etc/passwd',
      'docs//',
      'wallet_private_key',
    ]) {
      expect(() =>
        decodeOpenAgentsAutopilotWorkRequest({
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
          dataScope: {
            pathPrefixes: [pathPrefix],
            repoRefs: ['repo.github.OpenAgentsInc.openagents'],
            toolRefs: ['tool.git_checkout'],
          },
        }),
      ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
    }

    expect(() =>
      decodeOpenAgentsAutopilotWorkRequest({
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        dataScope: {
          pathPrefixes: ['docs/'],
          repoRefs: ['repo.github.OpenAgentsInc.openagents'],
          toolRefs: ['bash'],
        },
      }),
    ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
  })

  test('rejects private repositories until access and secret-broker states are modeled', () => {
    expect(() =>
      decodeOpenAgentsAutopilotWorkRequest({
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        tasks: [
          {
            ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
            repository: {
              ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0]
                .repository,
              fullName: 'customer/private-product',
              visibility: 'private',
            },
          },
        ],
      }),
    ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
  })

  test('rejects unsafe secret, wallet, payment, raw prompt, and runner material anywhere in the request', () => {
    for (const fixture of OPENAGENTS_UNSAFE_REDACTION_FIXTURES) {
      expect(() =>
        decodeOpenAgentsAutopilotWorkRequest({
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
          tasks: [
            {
              ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].tasks[0],
              objective: `Repair public docs. ${fixture.value}`,
            },
          ],
        }),
      ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
    }
  })

  test('rejects private or premium privacy tiers that request public traces', () => {
    expect(() =>
      decodeOpenAgentsAutopilotWorkRequest({
        ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0],
        placementPolicy: {
          ...OPENAGENTS_AUTOPILOT_WORK_REQUEST_FIXTURES[0].placementPolicy,
          privacyTier: 'tee',
          publicTraceAllowed: true,
        },
      }),
    ).toThrow(OpenAgentsAutopilotWorkRequestUnsafe)
  })
})
