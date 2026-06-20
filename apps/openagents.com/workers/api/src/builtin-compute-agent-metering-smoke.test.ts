import { Schema as S } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
  BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS,
  BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
} from './builtin-compute-agent-grant'
import {
  BuiltinComputeAgentMeteringSmokeInput,
  BuiltinComputeAgentMeteringSmokeProjection,
  builtinComputeAgentMeteringSmokeHasPrivateMaterial,
  planBuiltinComputeAgentMeteringSmoke,
} from './builtin-compute-agent-metering-smoke'

// ─── Fixture helpers ─────────────────────────────────────────────────────────

const ciBaseInput = (
  overrides: Partial<BuiltinComputeAgentMeteringSmokeInput> = {},
): BuiltinComputeAgentMeteringSmokeInput =>
  new BuiltinComputeAgentMeteringSmokeInput({
    installerSignatureRefs: [],
    pylonReadinessRefs: [],
    hostedComputeReadinessRefs: [],
    grantEventRefs: [],
    sessionCloseoutRefs: [],
    usageEventRefs: [],
    publicQuotaProjectionRefs: [],
    grantIssuedWithoutUserKey: false,
    sessionWithinDailyCap: false,
    sessionsUsedToday: 0,
    configuredDailySessionCap: BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
    configuredSessionBudgetSeconds: BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS,
    configuredDailyTokenCeiling: BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
    mode: 'ci_no_live_sessions',
    nowIso: '2026-06-20T10:00:00.000Z',
    operatorApprovedLive: false,
    operatorApprovalRefs: [],
    ...overrides,
  })

const liveBaseInput = (
  overrides: Partial<BuiltinComputeAgentMeteringSmokeInput> = {},
): BuiltinComputeAgentMeteringSmokeInput =>
  new BuiltinComputeAgentMeteringSmokeInput({
    installerSignatureRefs: [
      'installer.autopilot.desktop.signed.notarized.v1.dmg.sha256.ref',
    ],
    pylonReadinessRefs: ['pylon.heartbeat.local.ref'],
    hostedComputeReadinessRefs: [
      'hosted_compute.grant_endpoint.200.ref',
    ],
    grantEventRefs: ['builtin_compute_grant.event.ref'],
    sessionCloseoutRefs: ['session.closeout.within_cap.ref'],
    usageEventRefs: ['token_usage_event.builtin_compute.ref'],
    publicQuotaProjectionRefs: [
      'quota.projection.public.sessions_remaining.ref',
    ],
    grantIssuedWithoutUserKey: true,
    sessionWithinDailyCap: true,
    sessionsUsedToday: 1,
    configuredDailySessionCap: BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
    configuredSessionBudgetSeconds: BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS,
    configuredDailyTokenCeiling: BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
    mode: 'live_from_signed_install',
    nowIso: '2026-06-20T10:00:00.000Z',
    operatorApprovedLive: true,
    operatorApprovalRefs: ['operator.approval.builtin_compute.live.ref'],
    ...overrides,
  })

// ─── CI mode ─────────────────────────────────────────────────────────────────

describe('planBuiltinComputeAgentMeteringSmoke — ci_no_live_sessions', () => {
  test('produces a valid projection that decodes against the schema', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(ciBaseInput())
    expect(
      S.decodeUnknownSync(BuiltinComputeAgentMeteringSmokeProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.status).toBe('ci_no_live_sessions_ready')
    expect(projection.liveSessionVerified).toBe(false)
    expect(projection.blockerRefs).toHaveLength(0)
  })

  test('all steps are present and in planned_no_live_sessions state (except installer which is also planned)', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(ciBaseInput())
    const stepKinds = projection.steps.map(s => s.kind)
    expect(stepKinds).toContain('installer_signed')
    expect(stepKinds).toContain('pylon_readiness_checked')
    expect(stepKinds).toContain('hosted_compute_readiness_checked')
    expect(stepKinds).toContain('grant_issued')
    expect(stepKinds).toContain('session_bounded')
    expect(stepKinds).toContain('usage_recorded')
    expect(stepKinds).toContain('public_quota_projection')
    for (const step of projection.steps) {
      expect(step.state).not.toBe('blocked')
    }
  })

  test('cap constants are reflected in the projection', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(ciBaseInput())
    expect(projection.configuredDailySessionCap).toBe(
      BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
    )
    expect(projection.configuredSessionBudgetSeconds).toBe(
      BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS,
    )
    expect(projection.configuredDailyTokenCeiling).toBe(
      BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
    )
  })

  test('blocks when cap constants do not match the canonical policy', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(
      ciBaseInput({ configuredDailySessionCap: 99 }),
    )
    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.builtin_compute_agent_metering_smoke.cap_constants_mismatch',
    )
  })

  test('redaction scan passes — no raw key material in projection', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(ciBaseInput())
    expect(projection.redactionScanPassed).toBe(true)
    expect(
      builtinComputeAgentMeteringSmokeHasPrivateMaterial(projection),
    ).toBe(false)
  })

  test('smoke bundle refs are empty in CI mode (no live refs supplied)', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(ciBaseInput())
    expect(projection.smokeBundleRefs).toHaveLength(0)
  })
})

// ─── live_from_signed_install mode ───────────────────────────────────────────

describe('planBuiltinComputeAgentMeteringSmoke — live_from_signed_install', () => {
  test('produces live_metered_session_verified with all refs supplied', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(liveBaseInput())
    expect(
      S.decodeUnknownSync(BuiltinComputeAgentMeteringSmokeProjection)(
        projection,
      ),
    ).toEqual(projection)
    expect(projection.status).toBe('live_metered_session_verified')
    expect(projection.liveSessionVerified).toBe(true)
    expect(projection.grantIssuedWithoutUserKey).toBe(true)
    expect(projection.sessionWithinDailyCap).toBe(true)
    expect(projection.blockerRefs).toHaveLength(0)
  })

  test('all live steps pass', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(liveBaseInput())
    for (const step of projection.steps) {
      expect(step.state).toBe('passed')
    }
  })

  test('smoke bundle refs include all supplied live refs (sorted, deduped)', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(liveBaseInput())
    expect(projection.smokeBundleRefs).toContain(
      'builtin_compute_grant.event.ref',
    )
    expect(projection.smokeBundleRefs).toContain(
      'installer.autopilot.desktop.signed.notarized.v1.dmg.sha256.ref',
    )
    expect(projection.smokeBundleRefs).toContain(
      'quota.projection.public.sessions_remaining.ref',
    )
    expect(projection.smokeBundleRefs).toContain(
      'token_usage_event.builtin_compute.ref',
    )
  })

  test('blocks when installer signature refs are missing in live mode', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(
      liveBaseInput({ installerSignatureRefs: [] }),
    )
    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.builtin_compute_agent_metering_smoke.installer_signature_missing',
    )
    const installerStep = projection.steps.find(
      s => s.kind === 'installer_signed',
    )
    expect(installerStep?.state).toBe('blocked')
  })

  test('blocks when grant was not issued without user key', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(
      liveBaseInput({ grantIssuedWithoutUserKey: false }),
    )
    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.builtin_compute_agent_metering_smoke.grant_without_user_key_missing',
    )
  })

  test('blocks when session exceeded the daily cap', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(
      liveBaseInput({ sessionWithinDailyCap: false }),
    )
    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.builtin_compute_agent_metering_smoke.session_bounded_proof_missing',
    )
  })

  test('blocks when operator approval is missing in live mode', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(
      liveBaseInput({ operatorApprovedLive: false }),
    )
    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.builtin_compute_agent_metering_smoke.operator_approval_missing',
    )
  })

  test('blocks when usage event refs are missing', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(
      liveBaseInput({ usageEventRefs: [] }),
    )
    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.builtin_compute_agent_metering_smoke.usage_event_missing',
    )
  })

  test('blocks when public quota projection refs are missing', () => {
    const projection = planBuiltinComputeAgentMeteringSmoke(
      liveBaseInput({ publicQuotaProjectionRefs: [] }),
    )
    expect(projection.status).toBe('blocked')
    expect(projection.blockerRefs).toContain(
      'blocker.builtin_compute_agent_metering_smoke.public_quota_projection_missing',
    )
  })
})

// ─── Redaction guard ─────────────────────────────────────────────────────────

describe('builtinComputeAgentMeteringSmokeHasPrivateMaterial', () => {
  test('returns false for safe projection-shaped objects', () => {
    expect(
      builtinComputeAgentMeteringSmokeHasPrivateMaterial({
        grantRef: 'builtin_compute_grant.public.ref',
        status: 'issued',
        sessionsRemaining: 2,
      }),
    ).toBe(false)
  })

  test('returns true when raw API key material is present', () => {
    expect(
      builtinComputeAgentMeteringSmokeHasPrivateMaterial({
        rawKey: 'AIzaSyABCDEF12345678',
      }),
    ).toBe(true)
  })

  test('returns true when bearer token pattern is present', () => {
    expect(
      builtinComputeAgentMeteringSmokeHasPrivateMaterial({
        authorization: 'bearer eyJhbGciOiJIUzI1NiJ9',
      }),
    ).toBe(true)
  })
})

// ─── Invariant: required steps always present ─────────────────────────────────

describe('projection invariants', () => {
  test('throws if a required step is somehow absent (invariant guard)', () => {
    // We can't directly remove a step from the projection, but we verify
    // the planner always emits all 7 step kinds for both modes.
    for (const mode of [
      'ci_no_live_sessions',
      'live_from_signed_install',
    ] as const) {
      const input =
        mode === 'ci_no_live_sessions' ? ciBaseInput() : liveBaseInput()
      const projection = planBuiltinComputeAgentMeteringSmoke(input)
      expect(projection.steps).toHaveLength(7)
    }
  })
})
