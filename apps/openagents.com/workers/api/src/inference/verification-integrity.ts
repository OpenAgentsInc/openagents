// Verification-panel integrity helpers (#6423).
//
// These checks protect the replay/acceptance floor from correlated model-family
// failures. A verifier panel may have many nominal judges, but same-family judges
// count as one effective vote, and any model verifier from the worker's own family
// blocks certification.

export type VerificationChannelDefenses = Readonly<{
  paraphraseBeforeReview: boolean
  crossModelReview: boolean
  steganographyScan: boolean
}>

export type VerificationPanelMember = Readonly<{
  verifierRef: string
  model: string
}>

export type VerificationIntegrityInput = Readonly<{
  workerModel: string
  panel?: ReadonlyArray<VerificationPanelMember> | undefined
  channelDefenses?: VerificationChannelDefenses | undefined
  minimumEffectiveIndependentVotes?: number | undefined
}>

export type VerificationIntegrityReport = Readonly<{
  workerModelFamily: string
  verifierModelFamilies: ReadonlyArray<string>
  nominalVotes: number
  effectiveIndependentVotes: number
  minimumEffectiveIndependentVotes: number
  panelDiversityLabel: string
  channelDefenses: VerificationChannelDefenses
  blockerRefs: ReadonlyArray<string>
  passed: boolean
}>

const DEFAULT_DETERMINISTIC_VERIFIER: VerificationPanelMember = {
  model: 'deterministic/browser-acceptance-runner',
  verifierRef: 'verifier.khala_code.executed_acceptance_suite.v1',
}

const DEFAULT_CHANNEL_DEFENSES: VerificationChannelDefenses = {
  crossModelReview: true,
  paraphraseBeforeReview: true,
  steganographyScan: true,
}

const normalize = (value: string): string => value.trim().toLowerCase()

export const inferVerificationModelFamily = (model: string): string => {
  const value = normalize(model)
  if (value === '') return 'model_family.unknown'
  if (value.includes('deterministic') || value.includes('browser-acceptance')) {
    return 'model_family.deterministic_acceptance_runner'
  }
  if (
    value.includes('gpt') ||
    value.includes('openai') ||
    /\bo[134]\b/u.test(value)
  ) {
    return 'model_family.openai'
  }
  if (value.includes('claude') || value.includes('anthropic')) {
    return 'model_family.anthropic'
  }
  if (value.includes('gemini') || value.includes('google')) {
    return 'model_family.gemini'
  }
  if (value.includes('glm') || value.includes('z-ai') || value.includes('zai')) {
    return 'model_family.glm'
  }
  if (value.includes('deepseek')) return 'model_family.deepseek'
  if (value.includes('qwen')) return 'model_family.qwen'
  if (value.includes('kimi') || value.includes('moonshot')) {
    return 'model_family.kimi'
  }
  if (value.includes('llama') || value.includes('meta')) {
    return 'model_family.llama'
  }
  if (value.includes('mistral') || value.includes('mixtral')) {
    return 'model_family.mistral'
  }
  return `model_family.${value.replace(/[^a-z0-9]+/gu, '_').replace(/^_+|_+$/gu, '') || 'unknown'}`
}

export const assessVerificationIntegrity = (
  input: VerificationIntegrityInput,
): VerificationIntegrityReport => {
  const panel =
    input.panel === undefined || input.panel.length === 0
      ? [DEFAULT_DETERMINISTIC_VERIFIER]
      : input.panel
  const channelDefenses = input.channelDefenses ?? DEFAULT_CHANNEL_DEFENSES
  const minimumEffectiveIndependentVotes =
    input.minimumEffectiveIndependentVotes ?? 1
  const workerModelFamily = inferVerificationModelFamily(input.workerModel)
  const verifierModelFamilies = panel.map(member =>
    inferVerificationModelFamily(member.model),
  )
  const uniqueVerifierFamilies = new Set(verifierModelFamilies)
  const effectiveIndependentVotes = Array.from(uniqueVerifierFamilies).filter(
    family => family !== workerModelFamily,
  ).length
  const blockerRefs: string[] = []

  if (verifierModelFamilies.includes(workerModelFamily)) {
    blockerRefs.push(
      'blocker.public.verification_integrity.same_model_family_verifier',
    )
  }
  if (effectiveIndependentVotes < minimumEffectiveIndependentVotes) {
    blockerRefs.push(
      'blocker.public.verification_integrity.insufficient_effective_independent_votes',
    )
  }
  if (!channelDefenses.paraphraseBeforeReview) {
    blockerRefs.push(
      'blocker.public.verification_integrity.paraphrase_defense_missing',
    )
  }
  if (!channelDefenses.crossModelReview) {
    blockerRefs.push(
      'blocker.public.verification_integrity.cross_model_review_missing',
    )
  }
  if (!channelDefenses.steganographyScan) {
    blockerRefs.push(
      'blocker.public.verification_integrity.steganography_scan_missing',
    )
  }

  return {
    blockerRefs,
    channelDefenses,
    effectiveIndependentVotes,
    minimumEffectiveIndependentVotes,
    nominalVotes: panel.length,
    panelDiversityLabel: `${panel.length} nominal judge(s), ${effectiveIndependentVotes} effective independent vote(s)`,
    passed: blockerRefs.length === 0,
    verifierModelFamilies,
    workerModelFamily,
  }
}
