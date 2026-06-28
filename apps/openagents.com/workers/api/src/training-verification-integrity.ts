export type VerificationModelFamily =
  | 'anthropic_claude'
  | 'deepseek'
  | 'gemini'
  | 'glm'
  | 'gpt'
  | 'gpt_oss'
  | 'grok'
  | 'llama'
  | 'mistral'
  | 'openagents_khala'
  | 'pylon_codex'
  | 'pylon_claude'
  | 'qwen'
  | 'unknown'

export type VerificationPanelMember = Readonly<{
  family?: string
  model?: string
  ref: string
}>

export type VerificationChannelDefense = Readonly<{
  agentChannel?: boolean
  crossModelReview?: boolean
  paraphrasing?: boolean
  steganalysis?: boolean
}>

export type VerificationIntegrityInput = Readonly<{
  channelDefense?: VerificationChannelDefense
  minimumEffectiveVotes?: number
  verifierFamily?: string
  verifierModel?: string
  verifierPanel?: ReadonlyArray<VerificationPanelMember>
  workerFamily?: string
  workerModel?: string
}>

export type VerificationIntegrityAssessment = Readonly<{
  blockers: ReadonlyArray<string>
  crossModelDefenseRequired: boolean
  effectiveIndependenceVotes: number
  familyCounts: ReadonlyArray<Readonly<{ family: VerificationModelFamily; count: number }>>
  minimumEffectiveVotes: number
  ok: boolean
  panelSize: number
  paraphrasingDefenseRequired: boolean
  verifierFamilies: ReadonlyArray<VerificationModelFamily>
  workerFamily: VerificationModelFamily
}>

const familyPatterns: ReadonlyArray<
  readonly [VerificationModelFamily, ReadonlyArray<RegExp>]
> = [
  ['pylon_codex', [/pylon[-_/ ]?codex/i, /openagents\/pylon-codex/i]],
  ['pylon_claude', [/pylon[-_/ ]?claude/i, /openagents\/pylon-claude/i]],
  ['openagents_khala', [/openagents\/khala/i, /\bkhala\b/i]],
  ['gpt_oss', [/gpt[-_ ]?oss/i]],
  ['gpt', [/\bgpt[-_ ]?4/i, /\bgpt[-_ ]?5/i, /openai/i, /\bo[1345]\b/i]],
  ['anthropic_claude', [/claude/i, /anthropic/i]],
  ['gemini', [/gemini/i, /vertex/i, /google/i]],
  ['glm', [/\bglm\b/i, /z\.?ai/i]],
  ['deepseek', [/deepseek/i]],
  ['qwen', [/qwen/i, /dashscope/i]],
  ['llama', [/llama/i, /meta[-_ ]?ai/i]],
  ['mistral', [/mistral/i, /mixtral/i]],
  ['grok', [/grok/i, /xai/i]],
]

export const normalizeVerificationModelFamily = (
  familyOrModel: string | undefined,
): VerificationModelFamily => {
  const value = familyOrModel?.trim()

  if (value === undefined || value === '') {
    return 'unknown'
  }

  const exact = familyPatterns.find(([family]) => family === value)

  if (exact !== undefined) {
    return exact[0]
  }

  return familyPatterns.find(([, patterns]) =>
    patterns.some(pattern => pattern.test(value)),
  )?.[0] ?? 'unknown'
}

const uniqueSortedFamilies = (
  families: ReadonlyArray<VerificationModelFamily>,
): ReadonlyArray<VerificationModelFamily> => [...new Set(families)].sort()

const roundedVotes = (votes: number): number => Math.round(votes * 100) / 100

const effectiveIndependenceVotes = (
  families: ReadonlyArray<VerificationModelFamily>,
): number => {
  if (families.length === 0) {
    return 0
  }

  const counts = familyCounts(families)
  const denominator = counts.reduce((sum, row) => {
    const share = row.count / families.length

    return sum + share * share
  }, 0)

  return denominator === 0 ? 0 : roundedVotes(1 / denominator)
}

const familyCounts = (
  families: ReadonlyArray<VerificationModelFamily>,
): ReadonlyArray<Readonly<{ family: VerificationModelFamily; count: number }>> =>
  uniqueSortedFamilies(families)
    .map(family => ({
      family,
      count: families.filter(candidate => candidate === family).length,
    }))
    .sort((left, right) => right.count - left.count || left.family.localeCompare(right.family))

const verifierFamiliesFor = (
  input: VerificationIntegrityInput,
): ReadonlyArray<VerificationModelFamily> => {
  const panel = input.verifierPanel ?? []

  if (panel.length > 0) {
    return panel.map(member =>
      normalizeVerificationModelFamily(member.family ?? member.model),
    )
  }

  return [
    normalizeVerificationModelFamily(input.verifierFamily ?? input.verifierModel),
  ]
}

export const assessVerificationIntegrity = (
  input: VerificationIntegrityInput,
): VerificationIntegrityAssessment => {
  const panelSupplied = (input.verifierPanel?.length ?? 0) > 0
  const workerFamily = normalizeVerificationModelFamily(
    input.workerFamily ?? input.workerModel,
  )
  const verifierFamilies = verifierFamiliesFor(input)
  const uniqueVerifierFamilies = uniqueSortedFamilies(verifierFamilies)
  const effectiveVotes = effectiveIndependenceVotes(verifierFamilies)
  const minimumEffectiveVotes = input.minimumEffectiveVotes ?? (panelSupplied ? 2 : 1)
  const agentChannel = input.channelDefense?.agentChannel === true
  const paraphrasingDefenseRequired =
    agentChannel && input.channelDefense?.paraphrasing !== true
  const crossModelDefenseRequired =
    agentChannel && input.channelDefense?.crossModelReview !== true
  const blockers = [
    ...(workerFamily === 'unknown'
      ? ['blocker.verification_integrity.worker_model_family_unknown']
      : []),
    ...(verifierFamilies.some(family => family === 'unknown')
      ? ['blocker.verification_integrity.verifier_model_family_unknown']
      : []),
    ...(verifierFamilies.some(family => family === workerFamily)
      ? ['blocker.verification_integrity.same_family_worker_verifier']
      : []),
    ...(panelSupplied && uniqueVerifierFamilies.length < 2
      ? ['blocker.verification_integrity.verifier_panel_family_diversity_low']
      : []),
    ...(panelSupplied && effectiveVotes < minimumEffectiveVotes
      ? ['blocker.verification_integrity.effective_independence_below_floor']
      : []),
    ...(paraphrasingDefenseRequired
      ? ['blocker.verification_integrity.agent_channel_paraphrasing_missing']
      : []),
    ...(crossModelDefenseRequired
      ? ['blocker.verification_integrity.agent_channel_cross_model_review_missing']
      : []),
  ].sort()

  return {
    blockers,
    crossModelDefenseRequired,
    effectiveIndependenceVotes: effectiveVotes,
    familyCounts: familyCounts(verifierFamilies),
    minimumEffectiveVotes,
    ok: blockers.length === 0,
    panelSize: verifierFamilies.length,
    paraphrasingDefenseRequired,
    verifierFamilies: uniqueVerifierFamilies,
    workerFamily,
  }
}

export const verificationIntegrityInputFromPayload = (
  payload: Record<string, unknown>,
): VerificationIntegrityInput | undefined => {
  const workerFamily =
    typeof payload.workerModelFamily === 'string'
      ? payload.workerModelFamily
      : undefined
  const workerModel =
    typeof payload.workerModel === 'string' ? payload.workerModel : undefined
  const verifierFamily =
    typeof payload.verifierModelFamily === 'string'
      ? payload.verifierModelFamily
      : undefined
  const verifierModel =
    typeof payload.verifierModel === 'string' ? payload.verifierModel : undefined
  const rawPanel = Array.isArray(payload.verifierPanel)
    ? payload.verifierPanel
    : []
  const verifierPanel = rawPanel
    .map((item, index): VerificationPanelMember | undefined => {
      if (typeof item !== 'object' || item === null) {
        return undefined
      }

      const record = item as Record<string, unknown>
      const family = typeof record.family === 'string' ? record.family : undefined
      const model = typeof record.model === 'string' ? record.model : undefined
      const ref =
        typeof record.ref === 'string' && record.ref.trim() !== ''
          ? record.ref
          : `verifier.panel.${index}`

      return {
        ...(family === undefined ? {} : { family }),
        ...(model === undefined ? {} : { model }),
        ref,
      }
    })
    .filter((item): item is VerificationPanelMember => item !== undefined)
  const channelDefense =
    typeof payload.channelDefense === 'object' && payload.channelDefense !== null
      ? (payload.channelDefense as Record<string, unknown>)
      : undefined
  const minimumEffectiveVotes =
    typeof payload.minimumEffectiveVotes === 'number' &&
    Number.isFinite(payload.minimumEffectiveVotes)
      ? payload.minimumEffectiveVotes
      : undefined

  if (
    workerFamily === undefined &&
    workerModel === undefined &&
    verifierFamily === undefined &&
    verifierModel === undefined &&
    verifierPanel.length === 0 &&
    channelDefense === undefined
  ) {
    return undefined
  }

  return {
    ...(channelDefense === undefined
      ? {}
      : {
          channelDefense: {
            agentChannel: channelDefense.agentChannel === true,
            crossModelReview: channelDefense.crossModelReview === true,
            paraphrasing: channelDefense.paraphrasing === true,
            steganalysis: channelDefense.steganalysis === true,
          },
        }),
    ...(minimumEffectiveVotes === undefined ? {} : { minimumEffectiveVotes }),
    ...(verifierFamily === undefined ? {} : { verifierFamily }),
    ...(verifierModel === undefined ? {} : { verifierModel }),
    verifierPanel,
    ...(workerFamily === undefined ? {} : { workerFamily }),
    ...(workerModel === undefined ? {} : { workerModel }),
  }
}
