// Builtin compute agent metering smoke fixture.
//
// Models the steps needed to demonstrate a metered, bounded, no-user-key
// go-online session from a signed Autopilot Desktop installer — the evidence
// form for the `openagents_compute_metering_live_smoke_missing` blocker.
//
// Follows the same structural pattern as pylon-install-to-bitcoin-smoke.ts:
// a pure, referentially-transparent projection over public refs that asserts
// caps are enforced, no secrets leak, and the smoke bundle is complete. A live
// smoke run fills the refs; a CI run runs in ci_no_live_sessions mode.
//
// SECURITY: this module never materializes, logs, or returns raw keys.
// The grant it projects carries only a secret-REF (resolved by the runner).

import { Schema as S } from 'effect'

import {
  BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS,
  BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS,
  BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING,
} from './builtin-compute-agent-grant'

export const BUILTIN_COMPUTE_AGENT_METERING_SMOKE_SCHEMA_VERSION =
  'openagents.builtin_compute_agent_metering_smoke.v1' as const

// ─── Enums ───────────────────────────────────────────────────────────────────

export const BuiltinComputeAgentMeteringSmokeMode = S.Literals([
  // CI: validates projection logic only, no live sessions or real grants.
  'ci_no_live_sessions',
  // Full: validates against a real signed installer + live go-online session.
  'live_from_signed_install',
])
export type BuiltinComputeAgentMeteringSmokeMode =
  typeof BuiltinComputeAgentMeteringSmokeMode.Type

export const BuiltinComputeAgentMeteringSmokeStepKind = S.Literals([
  // Signed + notarized installer verified (hash/signature ref captured).
  'installer_signed',
  // Local Pylon reachability confirmed (heartbeat or status ref).
  'pylon_readiness_checked',
  // OpenAgents hosted-compute endpoint reachable (grant endpoint 200 ref).
  'hosted_compute_readiness_checked',
  // Builtin compute grant issued (no user key; metered; secret-ref only).
  'grant_issued',
  // Session bounded: did not exceed dailyCap × sessionBudgetSeconds.
  'session_bounded',
  // Usage event recorded in the quota ledger (grant event ID ref).
  'usage_recorded',
  // Public quota projection is readable (shows sessionsRemaining + resetsAt).
  'public_quota_projection',
])
export type BuiltinComputeAgentMeteringSmokeStepKind =
  typeof BuiltinComputeAgentMeteringSmokeStepKind.Type

export const BuiltinComputeAgentMeteringSmokeStepState = S.Literals([
  'blocked',
  'not_applicable',
  'passed',
  'planned_no_live_sessions',
])
export type BuiltinComputeAgentMeteringSmokeStepState =
  typeof BuiltinComputeAgentMeteringSmokeStepState.Type

export const BuiltinComputeAgentMeteringSmokeStatus = S.Literals([
  // One or more required steps are missing refs.
  'blocked',
  // CI mode: logic validated, no live session.
  'ci_no_live_sessions_ready',
  // Full live smoke: all steps passed with real refs.
  'live_metered_session_verified',
])
export type BuiltinComputeAgentMeteringSmokeStatus =
  typeof BuiltinComputeAgentMeteringSmokeStatus.Type

// ─── Input ───────────────────────────────────────────────────────────────────

export class BuiltinComputeAgentMeteringSmokeInput extends S.Class<BuiltinComputeAgentMeteringSmokeInput>(
  'BuiltinComputeAgentMeteringSmokeInput',
)({
  // Signed + notarized installer evidence refs (e.g. notarization ticket hash).
  installerSignatureRefs: S.Array(S.String),
  // Pylon readiness refs (heartbeat/status refs from local Pylon).
  pylonReadinessRefs: S.Array(S.String),
  // Hosted-compute readiness refs (grant-endpoint 200 / health-check refs).
  hostedComputeReadinessRefs: S.Array(S.String),
  // Grant refs: the grant event ID (not the secret; only the ref).
  grantEventRefs: S.Array(S.String),
  // Session closeout refs proving the session stayed within caps.
  sessionCloseoutRefs: S.Array(S.String),
  // Usage event refs from the quota ledger (token_usage_events row IDs).
  usageEventRefs: S.Array(S.String),
  // Public quota projection refs (sessionsRemaining + resetsAt captured).
  publicQuotaProjectionRefs: S.Array(S.String),
  // Whether the grant was issued with no user-supplied API key.
  grantIssuedWithoutUserKey: S.Boolean,
  // Whether the session stayed within the daily session cap.
  sessionWithinDailyCap: S.Boolean,
  // Sessions used today (at time of smoke).
  sessionsUsedToday: S.Number,
  // Configured daily session cap (from policy; must match the constant).
  configuredDailySessionCap: S.Number,
  // Configured session budget in seconds.
  configuredSessionBudgetSeconds: S.Number,
  // Configured daily token ceiling.
  configuredDailyTokenCeiling: S.Number,
  // Smoke mode.
  mode: BuiltinComputeAgentMeteringSmokeMode,
  // ISO timestamp at smoke time.
  nowIso: S.String,
  // Operator-approved live go-online (required for live_from_signed_install).
  operatorApprovedLive: S.Boolean,
  // Operator approval refs.
  operatorApprovalRefs: S.Array(S.String),
}) {}

// ─── Step ────────────────────────────────────────────────────────────────────

export class BuiltinComputeAgentMeteringSmokeStep extends S.Class<BuiltinComputeAgentMeteringSmokeStep>(
  'BuiltinComputeAgentMeteringSmokeStep',
)({
  blockerRefs: S.Array(S.String),
  evidenceRefs: S.Array(S.String),
  guardRefs: S.Array(S.String),
  kind: BuiltinComputeAgentMeteringSmokeStepKind,
  state: BuiltinComputeAgentMeteringSmokeStepState,
}) {}

// ─── Projection ──────────────────────────────────────────────────────────────

export class BuiltinComputeAgentMeteringSmokeProjection extends S.Class<BuiltinComputeAgentMeteringSmokeProjection>(
  'BuiltinComputeAgentMeteringSmokeProjection',
)({
  blockerRefs: S.Array(S.String),
  configuredDailySessionCap: S.Number,
  configuredDailyTokenCeiling: S.Number,
  configuredSessionBudgetSeconds: S.Number,
  grantIssuedWithoutUserKey: S.Boolean,
  liveSessionVerified: S.Boolean,
  mode: BuiltinComputeAgentMeteringSmokeMode,
  publicQuotaProjectionRefs: S.Array(S.String),
  redactionScanPassed: S.Boolean,
  schemaVersion: S.Literal(
    BUILTIN_COMPUTE_AGENT_METERING_SMOKE_SCHEMA_VERSION,
  ),
  sessionWithinDailyCap: S.Boolean,
  sessionsUsedToday: S.Number,
  smokeBundleRefs: S.Array(S.String),
  status: BuiltinComputeAgentMeteringSmokeStatus,
  steps: S.Array(BuiltinComputeAgentMeteringSmokeStep),
}) {}

export class BuiltinComputeAgentMeteringSmokeUnsafe extends S.TaggedErrorClass<BuiltinComputeAgentMeteringSmokeUnsafe>()(
  'BuiltinComputeAgentMeteringSmokeUnsafe',
  { reason: S.String },
) {}

// ─── Helpers ─────────────────────────────────────────────────────────────────

const requiredStepKinds: ReadonlyArray<BuiltinComputeAgentMeteringSmokeStepKind> =
  [
    'installer_signed',
    'pylon_readiness_checked',
    'hosted_compute_readiness_checked',
    'grant_issued',
    'session_bounded',
    'usage_recorded',
    'public_quota_projection',
  ]

// Patterns that must NOT appear in any projection output.
const unsafeMaterialPattern =
  /(@|AIza[0-9A-Za-z_-]{10,}|api[_-]?key|bearer|raw[_-]?key|secret[^_-]|sk-[a-z0-9]|provider[_-]?(key|credential|grant[^_-]ref)|token[^_-]?(=|:))/i

const hasRefs = (refs: ReadonlyArray<string>): boolean =>
  refs.filter(r => r.trim() !== '').length > 0

const uniqueRefs = (refs: ReadonlyArray<string>): ReadonlyArray<string> =>
  [...new Set(refs.map(r => r.trim()).filter(r => r !== ''))].sort()

const blockerIfMissing = (
  condition: boolean,
  ref: string,
): ReadonlyArray<string> => (condition ? [] : [ref])

const stepFor = (
  kind: BuiltinComputeAgentMeteringSmokeStepKind,
  evidenceRefs: ReadonlyArray<string>,
  guardRefs: ReadonlyArray<string>,
  blockerRefs: ReadonlyArray<string>,
  state: BuiltinComputeAgentMeteringSmokeStepState,
): BuiltinComputeAgentMeteringSmokeStep =>
  new BuiltinComputeAgentMeteringSmokeStep({
    blockerRefs: uniqueRefs(blockerRefs),
    evidenceRefs: uniqueRefs(evidenceRefs),
    guardRefs: uniqueRefs(guardRefs),
    kind,
    state,
  })

const liveState = (
  mode: BuiltinComputeAgentMeteringSmokeMode,
  blockers: ReadonlyArray<string>,
  evidencePresent: boolean,
): BuiltinComputeAgentMeteringSmokeStepState => {
  if (blockers.length > 0) return 'blocked'
  if (mode === 'ci_no_live_sessions') return 'planned_no_live_sessions'
  if (!evidencePresent) return 'blocked'
  return 'passed'
}

const capConstraintsMet = (
  input: BuiltinComputeAgentMeteringSmokeInput,
): boolean =>
  input.configuredDailySessionCap ===
    BUILTIN_COMPUTE_AGENT_FREE_DAILY_SESSIONS &&
  input.configuredSessionBudgetSeconds ===
    BUILTIN_COMPUTE_AGENT_SESSION_BUDGET_SECONDS &&
  input.configuredDailyTokenCeiling === BUILTIN_COMPUTE_AGENT_DAILY_TOKEN_CEILING

const commonBlockerRefs = (
  input: BuiltinComputeAgentMeteringSmokeInput,
): ReadonlyArray<string> => [
  ...blockerIfMissing(
    input.mode === 'ci_no_live_sessions' ||
      hasRefs(input.installerSignatureRefs),
    'blocker.builtin_compute_agent_metering_smoke.installer_signature_missing',
  ),
  ...blockerIfMissing(
    capConstraintsMet(input),
    'blocker.builtin_compute_agent_metering_smoke.cap_constants_mismatch',
  ),
  ...blockerIfMissing(
    input.mode === 'ci_no_live_sessions' ||
      hasRefs(input.pylonReadinessRefs),
    'blocker.builtin_compute_agent_metering_smoke.pylon_readiness_missing',
  ),
  ...blockerIfMissing(
    input.mode === 'ci_no_live_sessions' ||
      hasRefs(input.hostedComputeReadinessRefs),
    'blocker.builtin_compute_agent_metering_smoke.hosted_compute_readiness_missing',
  ),
  ...blockerIfMissing(
    input.mode === 'ci_no_live_sessions' ||
      (hasRefs(input.grantEventRefs) && input.grantIssuedWithoutUserKey),
    'blocker.builtin_compute_agent_metering_smoke.grant_without_user_key_missing',
  ),
  ...blockerIfMissing(
    input.mode === 'ci_no_live_sessions' ||
      (hasRefs(input.sessionCloseoutRefs) && input.sessionWithinDailyCap),
    'blocker.builtin_compute_agent_metering_smoke.session_bounded_proof_missing',
  ),
  ...blockerIfMissing(
    input.mode === 'ci_no_live_sessions' || hasRefs(input.usageEventRefs),
    'blocker.builtin_compute_agent_metering_smoke.usage_event_missing',
  ),
  ...blockerIfMissing(
    input.mode === 'ci_no_live_sessions' ||
      hasRefs(input.publicQuotaProjectionRefs),
    'blocker.builtin_compute_agent_metering_smoke.public_quota_projection_missing',
  ),
  ...(input.mode === 'live_from_signed_install'
    ? [
        ...blockerIfMissing(
          input.operatorApprovedLive,
          'blocker.builtin_compute_agent_metering_smoke.operator_approval_missing',
        ),
      ]
    : []),
]

const smokeBundleRefsFor = (
  input: BuiltinComputeAgentMeteringSmokeInput,
): ReadonlyArray<string> =>
  uniqueRefs([
    ...input.installerSignatureRefs,
    ...input.pylonReadinessRefs,
    ...input.hostedComputeReadinessRefs,
    ...input.grantEventRefs,
    ...input.sessionCloseoutRefs,
    ...input.usageEventRefs,
    ...input.publicQuotaProjectionRefs,
    ...input.operatorApprovalRefs,
  ])

const statusFor = (
  input: BuiltinComputeAgentMeteringSmokeInput,
  blockers: ReadonlyArray<string>,
): BuiltinComputeAgentMeteringSmokeStatus => {
  if (blockers.length > 0) return 'blocked'
  if (input.mode === 'live_from_signed_install') return 'live_metered_session_verified'
  return 'ci_no_live_sessions_ready'
}

// ─── Redaction guard ─────────────────────────────────────────────────────────

export const builtinComputeAgentMeteringSmokeHasPrivateMaterial = (
  value: unknown,
): boolean => {
  const json = JSON.stringify(value)
  return unsafeMaterialPattern.test(json)
}

// ─── Plan ────────────────────────────────────────────────────────────────────

export const planBuiltinComputeAgentMeteringSmoke = (
  input: BuiltinComputeAgentMeteringSmokeInput,
): BuiltinComputeAgentMeteringSmokeProjection => {
  const blockerRefs = uniqueRefs(commonBlockerRefs(input))
  const isLive = input.mode === 'live_from_signed_install'
  const liveAndUnblocked = isLive && blockerRefs.length === 0

  const steps = [
    stepFor(
      'installer_signed',
      input.installerSignatureRefs,
      [
        'guard.builtin_compute_agent_metering_smoke.notarized_installer_required',
      ],
      blockerIfMissing(
        !isLive || hasRefs(input.installerSignatureRefs),
        'blocker.builtin_compute_agent_metering_smoke.installer_signature_missing',
      ),
      isLive
        ? hasRefs(input.installerSignatureRefs)
          ? 'passed'
          : 'blocked'
        : 'planned_no_live_sessions',
    ),
    stepFor(
      'pylon_readiness_checked',
      input.pylonReadinessRefs,
      [
        'guard.builtin_compute_agent_metering_smoke.local_pylon_heartbeat_required',
      ],
      blockerIfMissing(
        !isLive || hasRefs(input.pylonReadinessRefs),
        'blocker.builtin_compute_agent_metering_smoke.pylon_readiness_missing',
      ),
      liveState(
        input.mode,
        blockerIfMissing(
          !isLive || hasRefs(input.pylonReadinessRefs),
          'blocker.builtin_compute_agent_metering_smoke.pylon_readiness_missing',
        ),
        hasRefs(input.pylonReadinessRefs),
      ),
    ),
    stepFor(
      'hosted_compute_readiness_checked',
      input.hostedComputeReadinessRefs,
      [
        'guard.builtin_compute_agent_metering_smoke.grant_endpoint_200_required',
      ],
      blockerIfMissing(
        !isLive || hasRefs(input.hostedComputeReadinessRefs),
        'blocker.builtin_compute_agent_metering_smoke.hosted_compute_readiness_missing',
      ),
      liveState(
        input.mode,
        blockerIfMissing(
          !isLive || hasRefs(input.hostedComputeReadinessRefs),
          'blocker.builtin_compute_agent_metering_smoke.hosted_compute_readiness_missing',
        ),
        hasRefs(input.hostedComputeReadinessRefs),
      ),
    ),
    stepFor(
      'grant_issued',
      input.grantEventRefs,
      [
        'guard.builtin_compute_agent_metering_smoke.no_user_key_in_grant',
        'guard.builtin_compute_agent_metering_smoke.secret_ref_only_in_output',
      ],
      blockerIfMissing(
        !isLive || (hasRefs(input.grantEventRefs) && input.grantIssuedWithoutUserKey),
        'blocker.builtin_compute_agent_metering_smoke.grant_without_user_key_missing',
      ),
      liveState(
        input.mode,
        blockerIfMissing(
          !isLive || (hasRefs(input.grantEventRefs) && input.grantIssuedWithoutUserKey),
          'blocker.builtin_compute_agent_metering_smoke.grant_without_user_key_missing',
        ),
        hasRefs(input.grantEventRefs) && input.grantIssuedWithoutUserKey,
      ),
    ),
    stepFor(
      'session_bounded',
      input.sessionCloseoutRefs,
      [
        'guard.builtin_compute_agent_metering_smoke.daily_cap_enforced',
        `guard.builtin_compute_agent_metering_smoke.session_cap.${input.configuredDailySessionCap}_per_day`,
        `guard.builtin_compute_agent_metering_smoke.session_budget.${input.configuredSessionBudgetSeconds}s`,
      ],
      blockerIfMissing(
        !isLive || (hasRefs(input.sessionCloseoutRefs) && input.sessionWithinDailyCap),
        'blocker.builtin_compute_agent_metering_smoke.session_bounded_proof_missing',
      ),
      liveState(
        input.mode,
        blockerIfMissing(
          !isLive || (hasRefs(input.sessionCloseoutRefs) && input.sessionWithinDailyCap),
          'blocker.builtin_compute_agent_metering_smoke.session_bounded_proof_missing',
        ),
        hasRefs(input.sessionCloseoutRefs) && input.sessionWithinDailyCap,
      ),
    ),
    stepFor(
      'usage_recorded',
      input.usageEventRefs,
      [
        'guard.builtin_compute_agent_metering_smoke.token_usage_event_in_ledger',
        'guard.builtin_compute_agent_metering_smoke.quota_event_in_ledger',
      ],
      blockerIfMissing(
        !isLive || hasRefs(input.usageEventRefs),
        'blocker.builtin_compute_agent_metering_smoke.usage_event_missing',
      ),
      liveState(
        input.mode,
        blockerIfMissing(
          !isLive || hasRefs(input.usageEventRefs),
          'blocker.builtin_compute_agent_metering_smoke.usage_event_missing',
        ),
        hasRefs(input.usageEventRefs),
      ),
    ),
    stepFor(
      'public_quota_projection',
      input.publicQuotaProjectionRefs,
      [
        'guard.builtin_compute_agent_metering_smoke.sessions_remaining_readable',
        'guard.builtin_compute_agent_metering_smoke.resets_at_readable',
      ],
      blockerIfMissing(
        !isLive || hasRefs(input.publicQuotaProjectionRefs),
        'blocker.builtin_compute_agent_metering_smoke.public_quota_projection_missing',
      ),
      liveState(
        input.mode,
        blockerIfMissing(
          !isLive || hasRefs(input.publicQuotaProjectionRefs),
          'blocker.builtin_compute_agent_metering_smoke.public_quota_projection_missing',
        ),
        hasRefs(input.publicQuotaProjectionRefs),
      ),
    ),
  ]

  const projection = new BuiltinComputeAgentMeteringSmokeProjection({
    blockerRefs,
    configuredDailySessionCap: input.configuredDailySessionCap,
    configuredDailyTokenCeiling: input.configuredDailyTokenCeiling,
    configuredSessionBudgetSeconds: input.configuredSessionBudgetSeconds,
    grantIssuedWithoutUserKey: isLive
      ? input.grantIssuedWithoutUserKey
      : true, // always true in CI (no session)
    liveSessionVerified: liveAndUnblocked,
    mode: input.mode,
    publicQuotaProjectionRefs: uniqueRefs(input.publicQuotaProjectionRefs),
    redactionScanPassed: true,
    schemaVersion: BUILTIN_COMPUTE_AGENT_METERING_SMOKE_SCHEMA_VERSION,
    sessionWithinDailyCap: isLive ? input.sessionWithinDailyCap : true,
    sessionsUsedToday: Math.max(0, Math.trunc(input.sessionsUsedToday)),
    smokeBundleRefs: smokeBundleRefsFor(input),
    status: statusFor(input, blockerRefs),
    steps,
  })

  if (
    requiredStepKinds.some(
      kind => !projection.steps.some(step => step.kind === kind),
    )
  ) {
    throw new BuiltinComputeAgentMeteringSmokeUnsafe({
      reason:
        'Builtin compute agent metering smoke projection is missing a required step.',
    })
  }

  if (builtinComputeAgentMeteringSmokeHasPrivateMaterial(projection)) {
    throw new BuiltinComputeAgentMeteringSmokeUnsafe({
      reason:
        'Builtin compute agent metering smoke projection contains private or raw key material.',
    })
  }

  return projection
}
