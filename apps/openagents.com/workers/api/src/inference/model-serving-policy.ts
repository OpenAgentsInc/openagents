// Provider serving policy for the OpenAgents inference gateway
// (blocker.product_promises.public_paid_model_gateway_missing on
// api.hosted_gemini.v1).
//
// THE GAP this closes: the public catalog (`model-catalog.ts`, served at
// `/v1/models`) publishes EVERY model in the pricing table, regardless of
// whether the gateway can actually serve that model's supply lane right now. A
// supply lane is only servable when its upstream credential/binding is
// provisioned — the Vertex lanes need `VERTEX_SA_KEY`, the Fireworks lane needs
// `FIREWORKS_API_KEY`, and the OpenAgents serving fabric needs an explicit
// route-ready flag plus public-safe Pylon evidence refs. A PAID gateway must
// not advertise (and let a credits customer fund a balance toward) a model it
// cannot serve: a request for an unarmed lane can only fail `model_unavailable`
// at dispatch time. This module is the SINGLE provider policy that maps which
// lanes are armed to which catalog models the gateway may publish.
//
// PUBLIC-SAFE + NO SECRETS: the policy reads credential PRESENCE only (a boolean
// "is this env var a non-empty string"), never the secret value, so it neither
// handles nor can leak a credential. PURE: no D1, no clock, no network. It moves
// no money and changes no promise state — it only narrows what the public
// catalog advertises to what is genuinely servable.
import type { ModelCatalogEntry } from './model-catalog'
import { DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF } from './owned-inference-cost'
import {
  HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  KHALA_MODEL_ID,
  lookupModel,
  normalizeKhalaModelId,
} from './pricing'
import type { SupplyLane } from './pricing'

// Which supply lanes the gateway can ACTUALLY serve right now. A lane is "armed"
// only when its upstream credential/binding is provisioned.
export type HydraliskModelId =
  | typeof HYDRALISK_GLM_52_REAP_504B_MODEL_ID
  | typeof HYDRALISK_GPT_OSS_20B_MODEL_ID
  | typeof HYDRALISK_GPT_OSS_120B_MODEL_ID

export type HydraliskModelArming = Readonly<Record<HydraliskModelId, boolean>>

export const KHALA_BACKING_HYDRALISK_GPT_OSS = 'hydralisk-gpt-oss'
export const KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH =
  'fireworks-deepseek-v4-flash'

export type KhalaBackingModel =
  | typeof KHALA_BACKING_HYDRALISK_GPT_OSS
  | typeof KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH

export const KHALA_FIREWORKS_DEEPSEEK_V4_FLASH_PRICE_MODEL = 'deepseek-v4-flash'

export type SupplyLaneArming = Readonly<
  Record<SupplyLane, boolean> & {
    // Which internal supply policy backs the single public Khala model.
    // Undefined is treated as the historical Hydralisk GPT-OSS policy for
    // backward-compatible tests and callers that only know lane booleans.
    khalaBacking?: KhalaBackingModel | undefined
    // Optional for backward-compatible tests/callers that still model arming by
    // lane only. Real Worker env arming supplies this map so one Hydralisk
    // model cannot accidentally advertise another.
    hydraliskModels?: HydraliskModelArming
  }
>

// Safe default: nothing servable. A gateway with no provisioned lane advertises
// no paid models rather than advertising models it cannot serve.
export const ALL_LANES_UNARMED: SupplyLaneArming = {
  fireworks: false,
  hydraliskModels: {
    [HYDRALISK_GLM_52_REAP_504B_MODEL_ID]: false,
    [HYDRALISK_GPT_OSS_120B_MODEL_ID]: false,
    [HYDRALISK_GPT_OSS_20B_MODEL_ID]: false,
  },
  hydralisk: false,
  khalaBacking: KHALA_BACKING_HYDRALISK_GPT_OSS,
  openrouter: false,
  'openagents-network': false,
  'vertex-anthropic': false,
  'vertex-gemini': false,
}

const RAW_GPT_OSS_MODEL_IDS = new Set<string>([
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  'gpt-oss-20b',
  'gpt-oss-120b',
])

export const isRawGptOssModelId = (modelId: string): boolean =>
  RAW_GPT_OSS_MODEL_IDS.has(modelId.trim().toLowerCase())

export const isPublicModelId = (modelId: string): boolean =>
  normalizeKhalaModelId(modelId) === KHALA_MODEL_ID

// The presence-only env shape the arming is derived from. Every field is the
// SAME worker secret/flag name the corresponding adapter already reads; we only
// ever test for a non-empty value, never read the secret itself.
export type SupplyLaneCredentialEnv = Readonly<{
  // Mints the GCP token for both Vertex lanes (Claude + Gemini). See config.ts.
  VERTEX_SA_KEY?: string | undefined
  // Fireworks open-model lane key. See config.ts.
  FIREWORKS_API_KEY?: string | undefined
  // OpenRouter hidden Khala lane. Presence-only: the Worker secret arms this
  // supply lane; the upstream model is fixed in code to
  // `ibm-granite/granite-4.1-8b` at adapter registration so env cannot silently
  // switch it to a paid or Khala-facing alias.
  OPENROUTER_API_KEY?: string | undefined
  OPENROUTER_KHALA_FALLBACK_MODEL?: string | undefined
  // Operator-only backing selector for the single public Khala model. Supported
  // values are bounded below; raw customer model selection remains closed.
  KHALA_BACKING_MODEL?: string | undefined
  // Hydralisk GLM-5.2 504B REAP private G4 lane. This uses the admitted
  // OpenAI-compatible private proxy and remains a backing worker for the single
  // public Khala model, not a public model selector.
  HYDRALISK_GLM_52_REAP_504B_ENABLED?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_BASE_URL?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_PROFILE_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_COST_PROFILE_REF?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_MAX_INFLIGHT?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_BENCHMARK_RESERVED?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_DRAINING?: string | undefined
  HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS?: string | undefined
  // Hydralisk GPT-OSS 20B lane. The URL/token are secret-backed transport
  // presence only; the public-safe preflight/receipt refs are the evidence that
  // the owned L4/vLLM route is ready for Khala traffic.
  HYDRALISK_GPT_OSS_20B_ENABLED?: string | undefined
  HYDRALISK_BASE_URL?: string | undefined
  HYDRALISK_BEARER_TOKEN?: string | undefined
  HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF?: string | undefined
  HYDRALISK_GPT_OSS_20B_RECEIPT_REF?: string | undefined
  // Hydralisk GPT-OSS 120B high-memory lane. This intentionally requires its
  // own URL/token and evidence refs so the live L4 20B host cannot arm 120B by
  // accident.
  HYDRALISK_GPT_OSS_120B_ENABLED?: string | undefined
  HYDRALISK_GPT_OSS_120B_BASE_URL?: string | undefined
  HYDRALISK_GPT_OSS_120B_BEARER_TOKEN?: string | undefined
  HYDRALISK_GPT_OSS_120B_PREFLIGHT_REF?: string | undefined
  HYDRALISK_GPT_OSS_120B_RECEIPT_REF?: string | undefined
  // Explicit public-gateway route arming for the OpenAgents/Pylon serving
  // fabric. `ready` is the only accepted on-token; public-safe refs below carry
  // the evidence. No endpoint URL, API key, raw prompt, or private host appears
  // in this policy.
  OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY?: string | undefined
  OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF?: string | undefined
  OPENAGENTS_NETWORK_SERVING_PREFLIGHT_REF?: string | undefined
  OPENAGENTS_NETWORK_SERVING_RECEIPT_REF?: string | undefined
  OPENAGENTS_NETWORK_REPLAY_CHALLENGE_REF?: string | undefined
  OPENAGENTS_NETWORK_ADMITTED_PYLON_REF?: string | undefined
  // Secret-backed transport presence for the real serving route. Presence-only:
  // the URL/token values are never returned from this policy and must not appear
  // in public readiness/catalog payloads.
  OPENAGENTS_NETWORK_FABRIC_SERVE_URL?: string | undefined
  OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN?: string | undefined
}>

export type OpenAgentsNetworkGatewayArming = Readonly<{
  armed: boolean
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

export type HydraliskGptOss20bArming = Readonly<{
  armed: boolean
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

export type HydraliskGlm52Reap504bArming = Readonly<{
  armed: boolean
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  replicas: ReadonlyArray<HydraliskGlm52Replica>
}>

export type HydraliskGlm52Replica = Readonly<{
  replicaId: string
  baseUrl: string
  bearerToken: string
  baseUrlSecretRef: string
  bearerSecretRef: string
  profileRef: string
  evidenceRefs: ReadonlyArray<string>
  costProfileRef: string
  maxInflight: number
  benchmarkReserved: boolean
  draining: boolean
}>

export type HydraliskGlm52ReplicaArming = Readonly<{
  replicaId: string
  armed: boolean
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  replica?: HydraliskGlm52Replica | undefined
}>

export type HydraliskGptOss120bArming = Readonly<{
  armed: boolean
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}>

// Is an env credential present (a non-blank string)? Presence-only; the value is
// never returned or logged.
const isPresent = (value: string | undefined): boolean =>
  typeof value === 'string' && value.trim() !== ''

const GATEWAY_ROUTE_READY_ON_TOKEN = 'ready'
const HYDRALISK_ROUTE_READY_ON_TOKEN = 'ready'

const PUBLIC_SAFE_REF = /^[a-z0-9][a-z0-9._:-]{1,199}$/iu

const isPublicSafeRef = (value: string | undefined): value is string => {
  if (typeof value !== 'string' || value.trim() === '') {
    return false
  }
  const trimmed = value.trim()
  return (
    trimmed === value &&
    PUBLIC_SAFE_REF.test(trimmed) &&
    !trimmed.includes('://') &&
    !trimmed.toLowerCase().startsWith('sk-')
  )
}

const LEGACY_GLM_REPLICA_ID = 'primary'
const DEFAULT_GLM_PROFILE_REF =
  'profile.hydralisk.glm_52_reap_504b.g4_tp4_minp.v1'
const DEFAULT_GLM_COST_PROFILE_REF =
  DEFAULT_GLM_52_REAP_504B_OWNED_COST_PROFILE_REF
const REPLICA_ID = /^[a-z0-9][a-z0-9-]{0,63}$/u

const isEnabledFlag = (value: string | undefined): boolean => {
  const normalized = value?.trim().toLowerCase()
  return (
    normalized === 'true' ||
    normalized === '1' ||
    normalized === 'yes' ||
    normalized === 'on'
  )
}

const parsePositiveInt = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number(value?.trim())
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback
}

const parseGlmReplicaIds = (
  value: string | undefined,
): Readonly<{
  replicaIds: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
}> => {
  const rawValue = value?.trim()
  if (rawValue === undefined || rawValue === '') {
    return { blockerRefs: [], replicaIds: [LEGACY_GLM_REPLICA_ID] }
  }

  const blockerRefs: Array<string> = []
  const seen = new Set<string>()
  const replicaIds: Array<string> = []
  for (const raw of rawValue.split(',')) {
    const replicaId = raw.trim().toLowerCase()
    if (!REPLICA_ID.test(replicaId)) {
      blockerRefs.push('blocker.hydralisk_glm_52_reap_504b.replica_id_invalid')
      continue
    }
    if (seen.has(replicaId)) {
      blockerRefs.push(
        `blocker.hydralisk_glm_52_reap_504b.${replicaId}.replica_id_duplicate`,
      )
      continue
    }
    seen.add(replicaId)
    replicaIds.push(replicaId)
  }

  if (replicaIds.length === 0) {
    blockerRefs.push('blocker.hydralisk_glm_52_reap_504b.replica_ids_empty')
  }

  return { blockerRefs, replicaIds }
}

const replicaEnvToken = (replicaId: string): string =>
  replicaId.replace(/-/gu, '_').toUpperCase()

const glmReplicaEnvKey = (replicaId: string, suffix: string): string =>
  `HYDRALISK_GLM_52_REAP_504B_${replicaEnvToken(replicaId)}_${suffix}`

const envStringValue = (
  env: SupplyLaneCredentialEnv,
  key: string,
): string | undefined => {
  const value = (env as Readonly<Record<string, unknown>>)[key]
  return typeof value === 'string' ? value : undefined
}

const glmReplicaEnvValue = (
  env: SupplyLaneCredentialEnv,
  replicaId: string,
  suffix: string,
  legacyKey: keyof SupplyLaneCredentialEnv,
): Readonly<{ key: string; value: string | undefined }> => {
  const key = glmReplicaEnvKey(replicaId, suffix)
  const namedValue = envStringValue(env, key)
  if (replicaId === LEGACY_GLM_REPLICA_ID && !isPresent(namedValue)) {
    return {
      key: String(legacyKey),
      value: envStringValue(env, String(legacyKey)),
    }
  }
  return { key, value: namedValue }
}

const resolveHydraliskGlm52ReplicaArming = (
  env: SupplyLaneCredentialEnv,
  replicaId: string,
): HydraliskGlm52ReplicaArming => {
  const enabled = glmReplicaEnvValue(
    env,
    replicaId,
    'ENABLED',
    'HYDRALISK_GLM_52_REAP_504B_ENABLED',
  )
  const baseUrl = glmReplicaEnvValue(
    env,
    replicaId,
    'BASE_URL',
    'HYDRALISK_GLM_52_REAP_504B_BASE_URL',
  )
  const bearerToken = glmReplicaEnvValue(
    env,
    replicaId,
    'BEARER_TOKEN',
    'HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN',
  )
  const preflightRef = glmReplicaEnvValue(
    env,
    replicaId,
    'PREFLIGHT_REF',
    'HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF',
  )
  const receiptRef = glmReplicaEnvValue(
    env,
    replicaId,
    'RECEIPT_REF',
    'HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF',
  )
  const profileRef = glmReplicaEnvValue(
    env,
    replicaId,
    'PROFILE_REF',
    'HYDRALISK_GLM_52_REAP_504B_PROFILE_REF',
  )
  const costProfileRef = glmReplicaEnvValue(
    env,
    replicaId,
    'COST_PROFILE_REF',
    'HYDRALISK_GLM_52_REAP_504B_COST_PROFILE_REF',
  )
  const maxInflight = glmReplicaEnvValue(
    env,
    replicaId,
    'MAX_INFLIGHT',
    'HYDRALISK_GLM_52_REAP_504B_MAX_INFLIGHT',
  )
  const benchmarkReserved = glmReplicaEnvValue(
    env,
    replicaId,
    'BENCHMARK_RESERVED',
    'HYDRALISK_GLM_52_REAP_504B_BENCHMARK_RESERVED',
  )
  const draining = glmReplicaEnvValue(
    env,
    replicaId,
    'DRAINING',
    'HYDRALISK_GLM_52_REAP_504B_DRAINING',
  )

  const blockerPrefix =
    replicaId === LEGACY_GLM_REPLICA_ID &&
    !isPresent(env.HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS)
      ? 'blocker.hydralisk_glm_52_reap_504b'
      : `blocker.hydralisk_glm_52_reap_504b.${replicaId}`
  const blockerRefs: Array<string> = []
  if (enabled.value?.trim() !== HYDRALISK_ROUTE_READY_ON_TOKEN) {
    blockerRefs.push(`${blockerPrefix}.route_not_ready`)
  }
  if (!isPresent(baseUrl.value)) {
    blockerRefs.push(`${blockerPrefix}.base_url_missing`)
  }
  if (!isPresent(bearerToken.value)) {
    blockerRefs.push(`${blockerPrefix}.bearer_missing`)
  }

  const evidenceRefs: Array<string> = []
  const evidence: Array<[string, string | undefined]> = [
    [`${blockerPrefix}.preflight_ref_missing`, preflightRef.value],
    [`${blockerPrefix}.receipt_ref_missing`, receiptRef.value],
  ]
  for (const [blockerRef, value] of evidence) {
    if (isPublicSafeRef(value)) {
      evidenceRefs.push(value)
    } else {
      blockerRefs.push(blockerRef)
    }
  }

  const profileCandidate = profileRef.value?.trim()
  const costProfileCandidate = costProfileRef.value?.trim()
  const resolvedProfileRef =
    profileCandidate === undefined || profileCandidate === ''
      ? DEFAULT_GLM_PROFILE_REF
      : profileCandidate
  const resolvedCostProfileRef =
    costProfileCandidate === undefined || costProfileCandidate === ''
      ? DEFAULT_GLM_COST_PROFILE_REF
      : costProfileCandidate
  if (!isPublicSafeRef(resolvedProfileRef)) {
    blockerRefs.push(`${blockerPrefix}.profile_ref_invalid`)
  }
  if (!isPublicSafeRef(resolvedCostProfileRef)) {
    blockerRefs.push(`${blockerPrefix}.cost_profile_ref_invalid`)
  }

  const armed = blockerRefs.length === 0
  return {
    armed,
    blockerRefs,
    evidenceRefs,
    replicaId,
    ...(armed
      ? {
          replica: {
            baseUrl: baseUrl.value!.trim(),
            baseUrlSecretRef: baseUrl.key,
            bearerSecretRef: bearerToken.key,
            bearerToken: bearerToken.value!.trim(),
            benchmarkReserved: isEnabledFlag(benchmarkReserved.value),
            costProfileRef: resolvedCostProfileRef,
            draining: isEnabledFlag(draining.value),
            evidenceRefs,
            maxInflight: parsePositiveInt(maxInflight.value, 1),
            profileRef: resolvedProfileRef,
            replicaId,
          },
        }
      : {}),
  }
}

export const resolveKhalaBackingModel = (
  value: string | undefined,
): KhalaBackingModel => {
  const normalized = value?.trim().toLowerCase()
  switch (normalized) {
    case 'deepseek-v4-flash':
    case 'fireworks/deepseek-v4-flash':
    case 'accounts/fireworks/models/deepseek-v4-flash':
      return KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH
    default:
      return KHALA_BACKING_HYDRALISK_GPT_OSS
  }
}

const khalaBackingFor = (arming: SupplyLaneArming): KhalaBackingModel =>
  arming.khalaBacking ?? KHALA_BACKING_HYDRALISK_GPT_OSS

export const khalaBackingSupplyLane = (arming: SupplyLaneArming): SupplyLane =>
  khalaBackingFor(arming) === KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH
    ? 'fireworks'
    : 'hydralisk'

export const khalaBackingPriceModel = (arming: SupplyLaneArming): string =>
  khalaBackingFor(arming) === KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH
    ? KHALA_FIREWORKS_DEEPSEEK_V4_FLASH_PRICE_MODEL
    : KHALA_MODEL_ID

export const resolveOpenAgentsNetworkGatewayArming = (
  env: SupplyLaneCredentialEnv,
): OpenAgentsNetworkGatewayArming => {
  const evidence: Array<[string, string | undefined]> = [
    [
      'blocker.openagents_network_gateway.approval_ref_missing',
      env.OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF,
    ],
    [
      'blocker.openagents_network_gateway.serving_preflight_ref_missing',
      env.OPENAGENTS_NETWORK_SERVING_PREFLIGHT_REF,
    ],
    [
      'blocker.openagents_network_gateway.serving_receipt_ref_missing',
      env.OPENAGENTS_NETWORK_SERVING_RECEIPT_REF,
    ],
    [
      'blocker.openagents_network_gateway.replay_challenge_ref_missing',
      env.OPENAGENTS_NETWORK_REPLAY_CHALLENGE_REF,
    ],
    [
      'blocker.openagents_network_gateway.admitted_pylon_ref_missing',
      env.OPENAGENTS_NETWORK_ADMITTED_PYLON_REF,
    ],
  ]

  const blockerRefs: Array<string> = []
  if (
    env.OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY?.trim() !==
    GATEWAY_ROUTE_READY_ON_TOKEN
  ) {
    blockerRefs.push('blocker.openagents_network_gateway.route_not_ready')
  }
  if (!isPresent(env.OPENAGENTS_NETWORK_FABRIC_SERVE_URL)) {
    blockerRefs.push('blocker.openagents_network_gateway.transport_url_missing')
  }
  if (!isPresent(env.OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN)) {
    blockerRefs.push(
      'blocker.openagents_network_gateway.transport_bearer_missing',
    )
  }

  const evidenceRefs: Array<string> = []
  for (const [blockerRef, value] of evidence) {
    if (isPublicSafeRef(value)) {
      evidenceRefs.push(value)
    } else {
      blockerRefs.push(blockerRef)
    }
  }

  return {
    armed: blockerRefs.length === 0,
    blockerRefs,
    evidenceRefs,
  }
}

export const resolveHydraliskGptOss20bArming = (
  env: SupplyLaneCredentialEnv,
): HydraliskGptOss20bArming => {
  return resolveHydraliskGptOssModelArming({
    baseUrl: env.HYDRALISK_BASE_URL,
    bearerToken: env.HYDRALISK_BEARER_TOKEN,
    blockerPrefix: 'blocker.hydralisk_gpt_oss_20b',
    enabled: env.HYDRALISK_GPT_OSS_20B_ENABLED,
    preflightRef: env.HYDRALISK_GPT_OSS_20B_PREFLIGHT_REF,
    receiptRef: env.HYDRALISK_GPT_OSS_20B_RECEIPT_REF,
  })
}

export const resolveHydraliskGlm52Reap504bArming = (
  env: SupplyLaneCredentialEnv,
): HydraliskGlm52Reap504bArming => {
  const replicaArmings = resolveHydraliskGlm52Reap504bReplicaArmings(env)
  const replicas = replicaArmings.flatMap(arming =>
    arming.replica === undefined ? [] : [arming.replica],
  )
  return {
    armed: replicas.length > 0,
    blockerRefs: replicaArmings.flatMap(arming => arming.blockerRefs),
    evidenceRefs: replicaArmings.flatMap(arming => arming.evidenceRefs),
    replicas,
  }
}

export const resolveHydraliskGlm52Reap504bReplicaArmings = (
  env: SupplyLaneCredentialEnv,
): ReadonlyArray<HydraliskGlm52ReplicaArming> => {
  const parsed = parseGlmReplicaIds(env.HYDRALISK_GLM_52_REAP_504B_REPLICA_IDS)
  const replicaArmings = parsed.replicaIds.map(replicaId =>
    resolveHydraliskGlm52ReplicaArming(env, replicaId),
  )
  return parsed.blockerRefs.length === 0
    ? replicaArmings
    : [
        ...replicaArmings,
        {
          armed: false,
          blockerRefs: parsed.blockerRefs,
          evidenceRefs: [],
          replicaId: 'configuration',
        },
      ]
}

export const resolveHydraliskGptOss120bArming = (
  env: SupplyLaneCredentialEnv,
): HydraliskGptOss120bArming => {
  return resolveHydraliskGptOssModelArming({
    baseUrl: env.HYDRALISK_GPT_OSS_120B_BASE_URL,
    bearerToken: env.HYDRALISK_GPT_OSS_120B_BEARER_TOKEN,
    blockerPrefix: 'blocker.hydralisk_gpt_oss_120b',
    enabled: env.HYDRALISK_GPT_OSS_120B_ENABLED,
    preflightRef: env.HYDRALISK_GPT_OSS_120B_PREFLIGHT_REF,
    receiptRef: env.HYDRALISK_GPT_OSS_120B_RECEIPT_REF,
  })
}

const resolveHydraliskGptOssModelArming = (
  input: Readonly<{
    enabled?: string | undefined
    baseUrl?: string | undefined
    bearerToken?: string | undefined
    preflightRef?: string | undefined
    receiptRef?: string | undefined
    blockerPrefix: string
  }>,
): HydraliskGptOss20bArming => {
  const blockerRefs: Array<string> = []
  if (input.enabled?.trim() !== HYDRALISK_ROUTE_READY_ON_TOKEN) {
    blockerRefs.push(`${input.blockerPrefix}.route_not_ready`)
  }
  if (!isPresent(input.baseUrl)) {
    blockerRefs.push(`${input.blockerPrefix}.base_url_missing`)
  }
  if (!isPresent(input.bearerToken)) {
    blockerRefs.push(`${input.blockerPrefix}.bearer_missing`)
  }

  const evidence: Array<[string, string | undefined]> = [
    [`${input.blockerPrefix}.preflight_ref_missing`, input.preflightRef],
    [`${input.blockerPrefix}.receipt_ref_missing`, input.receiptRef],
  ]

  const evidenceRefs: Array<string> = []
  for (const [blockerRef, value] of evidence) {
    if (isPublicSafeRef(value)) {
      evidenceRefs.push(value)
    } else {
      blockerRefs.push(blockerRef)
    }
  }

  return {
    armed: blockerRefs.length === 0,
    blockerRefs,
    evidenceRefs,
  }
}

// Derive which supply lanes are armed from credential PRESENCE. The OpenAgents
// serving-fabric lane is stricter than a credential presence check: it only arms
// when a deploy supplies route-ready plus public-safe evidence refs for an
// admitted Pylon, serving preflight, serving receipt, replay challenge, and
// owner approval.
export const resolveSupplyLaneArming = (
  env: SupplyLaneCredentialEnv,
): SupplyLaneArming => {
  const vertex = isPresent(env.VERTEX_SA_KEY)
  const khalaBacking = resolveKhalaBackingModel(env.KHALA_BACKING_MODEL)
  const openAgentsNetwork = resolveOpenAgentsNetworkGatewayArming(env)
  const hydraliskGlm52 = resolveHydraliskGlm52Reap504bArming(env)
  const hydralisk20b = resolveHydraliskGptOss20bArming(env)
  const hydralisk120b = resolveHydraliskGptOss120bArming(env)
  const hydraliskModels = {
    [HYDRALISK_GLM_52_REAP_504B_MODEL_ID]: hydraliskGlm52.armed,
    [HYDRALISK_GPT_OSS_120B_MODEL_ID]: hydralisk120b.armed,
    [HYDRALISK_GPT_OSS_20B_MODEL_ID]: hydralisk20b.armed,
  } as const
  return {
    fireworks: isPresent(env.FIREWORKS_API_KEY),
    hydralisk:
      hydraliskModels[HYDRALISK_GLM_52_REAP_504B_MODEL_ID] ||
      hydraliskModels[HYDRALISK_GPT_OSS_20B_MODEL_ID] ||
      hydraliskModels[HYDRALISK_GPT_OSS_120B_MODEL_ID],
    hydraliskModels,
    khalaBacking,
    openrouter: isPresent(env.OPENROUTER_API_KEY),
    'openagents-network': openAgentsNetwork.armed,
    'vertex-anthropic': vertex,
    'vertex-gemini': vertex,
  }
}

// Is a single lane armed?
export const isLaneArmed = (
  arming: SupplyLaneArming,
  lane: SupplyLane,
): boolean => arming[lane]

const hydraliskModelArmed = (
  arming: SupplyLaneArming,
  modelId: string,
): boolean => {
  const normalized = modelId.trim().toLowerCase()
  if (normalized === HYDRALISK_GLM_52_REAP_504B_MODEL_ID) {
    return (
      arming.hydraliskModels?.[HYDRALISK_GLM_52_REAP_504B_MODEL_ID] ??
      arming.hydralisk
    )
  }
  if (normalized === HYDRALISK_GPT_OSS_20B_MODEL_ID) {
    return (
      arming.hydraliskModels?.[HYDRALISK_GPT_OSS_20B_MODEL_ID] ??
      arming.hydralisk
    )
  }
  if (normalized === HYDRALISK_GPT_OSS_120B_MODEL_ID) {
    return (
      arming.hydraliskModels?.[HYDRALISK_GPT_OSS_120B_MODEL_ID] ??
      arming.hydralisk
    )
  }
  return arming.hydralisk
}

export const isKhalaBackingArmed = (arming: SupplyLaneArming): boolean =>
  khalaBackingSupplyLane(arming) === 'fireworks'
    ? arming.fireworks
    : arming.hydralisk

export const projectKhalaCatalogForArming = (
  catalog: ReadonlyArray<ModelCatalogEntry>,
  arming: SupplyLaneArming,
): ReadonlyArray<ModelCatalogEntry> => {
  if (khalaBackingFor(arming) !== KHALA_BACKING_FIREWORKS_DEEPSEEK_V4_FLASH) {
    return catalog
  }
  const backing = catalog.find(
    entry => entry.id === KHALA_FIREWORKS_DEEPSEEK_V4_FLASH_PRICE_MODEL,
  )
  if (backing === undefined) {
    return catalog
  }
  return catalog.map(entry =>
    entry.id === KHALA_MODEL_ID
      ? {
          ...entry,
          costBasis: backing.costBasis,
          lane: backing.lane,
          multiplier: backing.multiplier,
          ownedBy: backing.ownedBy,
          price: backing.price,
        }
      : entry,
  )
}

// Is a single catalog model servable under the given arming?
export const isModelServable = (
  entry: ModelCatalogEntry,
  arming: SupplyLaneArming,
): boolean => {
  if (!isPublicModelId(entry.id)) {
    return false
  }
  if (normalizeKhalaModelId(entry.id) === KHALA_MODEL_ID) {
    return isKhalaBackingArmed(arming)
  }
  return entry.lane === 'hydralisk'
    ? hydraliskModelArmed(arming, entry.id)
    : isLaneArmed(arming, entry.lane)
}

// Servability for a model the customer NAMES by id (vs a catalog entry already
// in hand). Resolves the id against the SAME pricing table the gateway bills
// from (`lookupModel`, case-insensitive), so a named quote cannot disagree with
// the catalog on which lane a model belongs to. Public serveability now has a
// second invariant: the only public named model is Khala. Raw GPT-OSS ids and
// old split ids are internal supply/legacy names and always return false here.
// Returns:
//   - true      : `khala` / `openagents/khala` is in the pricing table AND its
//                 backing lane is armed (servable right now)
//   - false     : the id is non-public OR Khala's backing lane is not armed
//   - undefined : retained only for the historical signature; current policy
//                 does not intentionally expose unknown public models.
export const resolveNamedModelServability = (
  modelId: string,
  arming: SupplyLaneArming,
): boolean | undefined => {
  const normalized = normalizeKhalaModelId(modelId)
  if (!isPublicModelId(normalized)) {
    return false
  }
  const entry = lookupModel(normalized)
  if (entry === undefined) {
    return false
  }
  if (normalized === KHALA_MODEL_ID) {
    return isKhalaBackingArmed(arming)
  }
  return entry.lane === 'hydralisk'
    ? hydraliskModelArmed(arming, entry.model)
    : isLaneArmed(arming, entry.lane)
}

export const filterPublicCatalog = (
  catalog: ReadonlyArray<ModelCatalogEntry>,
): ReadonlyArray<ModelCatalogEntry> =>
  catalog.filter(entry => isPublicModelId(entry.id))

// Narrow a published catalog to only the models the gateway can actually serve
// right now. Order is preserved. With every lane armed this is the identity
// filter (no model is dropped); with no lane armed it is empty.
export const filterServableCatalog = (
  catalog: ReadonlyArray<ModelCatalogEntry>,
  arming: SupplyLaneArming,
): ReadonlyArray<ModelCatalogEntry> =>
  projectKhalaCatalogForArming(catalog, arming).filter(entry =>
    isModelServable(entry, arming),
  )
