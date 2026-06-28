import {
  ProviderAccountId,
  ProviderAccountRef,
  ProviderConnectionAttemptId,
  IsoTimestamp as ProviderIsoTimestamp,
} from '@openagentsinc/provider-account-schema'
import type { Html } from 'foldkit/html'
import { describe, expect, test } from 'vitest'

import {
  type ProviderDeviceLoginStatusResponse,
  authBootstrapFromSession,
} from '../../../domain/session'
import { SettingsRoute, SettingsSectionRoute } from '../../../route'
import {
  FailedLoadProviderAccountPool,
  RequestedLoadProviderAccountPool,
  SucceededLoadProviderAccountPool,
  SucceededPollProviderDeviceLogin,
} from '../message'
import { type ProviderAccountPoolResponse, init } from '../model'
import {
  formatRateLimitCountdown,
  rateLimitCountdownTitle,
  rateLimitCountdownView,
} from '../page/settings'
import { initialCommands, update } from '../update'

type VNodeLike = Readonly<{
  children?: ReadonlyArray<VNodeLike | string | null>
  data?: {
    attrs?: Record<string, unknown>
    class?: Record<string, boolean>
    props?: Record<string, unknown>
  }
  sel?: string
  text?: string
}>

const isVNodeLike = (value: unknown): value is VNodeLike =>
  typeof value === 'object' && value !== null

const attrsToString = (node: VNodeLike): string => {
  const attrs = node.data?.attrs ?? {}
  const props = node.data?.props ?? {}
  const classes = Object.entries(node.data?.class ?? {})
    .filter(([, enabled]) => enabled)
    .map(([className]) => className)
    .join(' ')
  const pairs = [
    ...Object.entries(attrs),
    ...Object.entries(props),
    ...(classes.length === 0 ? [] : [['class', classes] as const]),
  ]

  return pairs
    .filter(
      ([, value]) => value !== false && value !== undefined && value !== null,
    )
    .map(([name, value]) =>
      value === true ? ` ${name}` : ` ${name}="${String(value)}"`,
    )
    .join('')
}

const renderHtml = (html: Html): string => {
  if (html === null || !isVNodeLike(html)) {
    return ''
  }

  const tag = html.sel ?? 'node'
  const children = (html.children ?? [])
    .map(child =>
      typeof child === 'string'
        ? child
        : child === null
          ? ''
          : renderHtml(child),
    )
    .join('')
  const text = html.text ?? ''

  return `<${tag}${attrsToString(html)}>${text}${children}</${tag}>`
}

const auth = authBootstrapFromSession({
  email: 'chris@openagents.com',
  name: 'Christopher David',
  userId: 'github:14167547',
})

const poolResponse = {
  generatedAt: '2026-06-11T12:00:00.000Z',
  provider: 'all_connected_provider_accounts',
  policyVersion: 'provider-account-lease-policy:v2',
  accounts: [
    {
      providerAccountRef: 'provider-account_1',
      provider: 'chatgpt_codex',
      accountLabel: 'chris@openagents.com',
      status: 'connected',
      health: 'healthy',
      eligibility: 'eligible',
      eligibilityReasons: [],
      operatorPriority: 100,
      activeLeaseCount: 1,
      leaseLimit: 2,
      cooldownUntil: null,
      cooldownRemainingSeconds: null,
      lowCredit: false,
      recentFailureClass: null,
      lastSelectedAt: '2026-06-11T11:30:00.000Z',
      lastSanityCheckAt: null,
      lastSanityCheckResult: null,
      lastParallelProbeAt: null,
      lastParallelProbeResult: null,
      lastSuccessfulLaunchAt: null,
      lastFailedLaunchAt: null,
      connectedAt: '2026-06-10T00:00:00.000Z',
      reconnect: { needed: false, reason: null },
    },
    {
      providerAccountRef: 'provider-account_2',
      provider: 'anthropic_claude',
      accountLabel: 'team@openagents.com',
      status: 'connected',
      health: 'healthy',
      eligibility: 'ineligible',
      eligibilityReasons: ['cooldown'],
      operatorPriority: 100,
      activeLeaseCount: 0,
      leaseLimit: 2,
      cooldownUntil: '2026-06-11T12:05:00.000Z',
      cooldownRemainingSeconds: 300,
      lowCredit: false,
      recentFailureClass: 'rate_limited',
      lastSelectedAt: null,
      lastSanityCheckAt: null,
      lastSanityCheckResult: null,
      lastParallelProbeAt: null,
      lastParallelProbeResult: null,
      lastSuccessfulLaunchAt: null,
      lastFailedLaunchAt: '2026-06-11T11:50:00.000Z',
      connectedAt: '2026-06-09T00:00:00.000Z',
      reconnect: { needed: false, reason: null },
    },
  ],
  activeLeases: [
    {
      leaseRef: 'provider-account-lease_ref_1',
      providerAccountRef: 'provider-account_1',
      provider: 'chatgpt_codex',
      accountLabel: 'chris@openagents.com',
      requestedAction: 'autopilot_coder_run',
      runId: 'run_1',
      assignmentId: null,
      orderId: null,
      startedAt: '2026-06-11T11:30:00.000Z',
      expiresAt: '2026-06-11T12:30:00.000Z',
      lastTouchedAt: null,
      status: 'active',
    },
  ],
  nextSelection: {
    status: 'selected',
    providerAccountRef: 'provider-account_1',
    provider: 'chatgpt_codex',
    accountLabel: 'chris@openagents.com',
    selectionReason:
      'Selected connected healthy account with 1 active lease(s), priority 100, and no cooldown, reconnect marker, or low-credit flag.',
    activeLeaseCount: 1,
    leaseLimit: 2,
  },
  summary: {
    total: 2,
    eligible: 1,
    activeLeaseCount: 1,
    lowCredit: 0,
    requiresReauth: 0,
    cooldown: 1,
    unhealthy: 0,
  },
} satisfies ProviderAccountPoolResponse

const connectedDeviceLoginStatus = {
  account: {
    accountLabel: 'chris@openagents.com',
    authMode: 'chatgpt_device_code',
    createdAt: ProviderIsoTimestamp.make('2026-06-11T00:00:00.000Z'),
    hasSecretRef: true,
    health: 'healthy',
    id: ProviderAccountId.make('provider_account_1'),
    lastStatusAt: ProviderIsoTimestamp.make('2026-06-11T00:00:03.000Z'),
    provider: 'chatgpt_codex',
    providerAccountRef: ProviderAccountRef.make('provider-account_1'),
    publicStatus: 'connected',
    status: 'connected',
    updatedAt: ProviderIsoTimestamp.make('2026-06-11T00:00:03.000Z'),
  },
  attempt: {
    completedAt: ProviderIsoTimestamp.make('2026-06-11T00:00:03.000Z'),
    createdAt: ProviderIsoTimestamp.make('2026-06-11T00:00:00.000Z'),
    expiresAt: ProviderIsoTimestamp.make('2026-06-11T00:10:00.000Z'),
    id: ProviderConnectionAttemptId.make('provider_attempt_1'),
    method: 'chatgpt_device_code',
    provider: 'chatgpt_codex',
    providerAccountId: ProviderAccountId.make('provider_account_1'),
    providerAccountRef: ProviderAccountRef.make('provider-account_1'),
    source: 'worker_device_code',
    status: 'connected',
    updatedAt: ProviderIsoTimestamp.make('2026-06-11T00:00:03.000Z'),
  },
} satisfies ProviderDeviceLoginStatusResponse

describe('provider account pool', () => {
  test('formats rate-limit countdowns as stable timer text', () => {
    expect(formatRateLimitCountdown(300)).toBe('05:00')
    expect(formatRateLimitCountdown(65)).toBe('01:05')
    expect(formatRateLimitCountdown(3661)).toBe('01:01:01')
    expect(formatRateLimitCountdown(-4)).toBe('00:00')
    expect(formatRateLimitCountdown(null)).toBe('reset pending')
  })

  test('renders rate-limited accounts with a semantic countdown timer', () => {
    const rendered = renderHtml(
      rateLimitCountdownView({
        cooldownRemainingSeconds: 300,
        cooldownUntil: '2026-06-11T12:05:00.000Z',
      }),
    )

    expect(rendered).toContain('data-rate-limit-countdown="true"')
    expect(rendered).toContain('datetime="2026-06-11T12:05:00.000Z"')
    expect(rendered).toContain('05:00')
    expect(rateLimitCountdownTitle('2026-06-11T12:05:00.000Z')).toContain(
      'Rate limit resets at ',
    )
  })

  test('settings connections route loads the account pool', () => {
    const model = init(SettingsSectionRoute({ section: 'connections' }), auth)

    expect(initialCommands(model).map(command => command.name)).toContain(
      'LoadProviderAccountPool',
    )
  })

  test('general settings route does not load the account pool', () => {
    const model = init(SettingsRoute(), auth)

    expect(initialCommands(model).map(command => command.name)).not.toContain(
      'LoadProviderAccountPool',
    )
  })

  test('requesting a pool refresh marks loading and dispatches the load command', () => {
    const model = init(SettingsSectionRoute({ section: 'connections' }), auth)
    const [loadingModel, commands] = update(
      model,
      RequestedLoadProviderAccountPool(),
    )

    expect(loadingModel.providerAccountPool._tag).toBe(
      'ProviderAccountPoolLoading',
    )
    expect(commands.map(command => command.name)).toEqual([
      'LoadProviderAccountPool',
    ])
  })

  test('successful pool load lands lease load, cooldowns, and reconnect state in the model', () => {
    const model = init(SettingsSectionRoute({ section: 'connections' }), auth)
    const [loadedModel] = update(
      model,
      SucceededLoadProviderAccountPool({ response: poolResponse }),
    )

    expect(loadedModel.providerAccountPool).toEqual({
      _tag: 'ProviderAccountPoolLoaded',
      response: poolResponse,
    })

    if (loadedModel.providerAccountPool._tag !== 'ProviderAccountPoolLoaded') {
      throw new Error('expected loaded account pool')
    }

    const pool = loadedModel.providerAccountPool.response

    expect(pool.summary.cooldown).toBe(1)
    expect(pool.accounts[1]?.cooldownRemainingSeconds).toBe(300)
    expect(pool.accounts[1]?.recentFailureClass).toBe('rate_limited')
    expect(pool.activeLeases[0]?.leaseRef).toBe('provider-account-lease_ref_1')
    expect(pool.nextSelection.providerAccountRef).toBe('provider-account_1')
  })

  test('failed pool load records the error', () => {
    const model = init(SettingsSectionRoute({ section: 'connections' }), auth)
    const [failedModel] = update(
      model,
      FailedLoadProviderAccountPool({ error: 'pool unavailable' }),
    )

    expect(failedModel.providerAccountPool).toEqual({
      _tag: 'ProviderAccountPoolFailed',
      error: 'pool unavailable',
    })
  })

  test('completed device login reloads the account pool', () => {
    const model = init(SettingsSectionRoute({ section: 'connections' }), auth)
    const [, commands] = update(
      model,
      SucceededPollProviderDeviceLogin({
        response: connectedDeviceLoginStatus,
      }),
    )

    expect(commands.map(command => command.name)).toEqual([
      'LoadProviderAccountPool',
    ])
  })
})
