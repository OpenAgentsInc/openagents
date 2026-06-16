export type ForgeSkillDescriptorState =
  | 'available'
  | 'disabled'
  | 'failed'
  | 'needs_review'
  | 'pending'

export type ForgeSkillDescriptorFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeSkillDescriptorCatalogStatus =
  | 'blocked'
  | 'empty'
  | 'needs_attention'
  | 'ready'
  | 'stale'

export type ForgeSkillDescriptorInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  bodyRequestRefs?: ReadonlyArray<string>
  descriptorRef: string
  freshness?: ForgeSkillDescriptorFreshness
  policyRefs?: ReadonlyArray<string>
  skillRef: string
  sourceRefs?: ReadonlyArray<string>
  state?: ForgeSkillDescriptorState
  summaryRefs?: ReadonlyArray<string>
  triggerRefs?: ReadonlyArray<string>
}>

export type ForgeSkillDescriptorCatalogInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  catalogRef: string
  entries?: ReadonlyArray<ForgeSkillDescriptorInput>
  freshness?: ForgeSkillDescriptorFreshness
  generatedAt: string
  workOrderRef: string
}>

export type ForgeSkillDescriptorEntry = Readonly<{
  blockerRefs: ReadonlyArray<string>
  bodyIncludedByDefault: false
  bodyRequestRefs: ReadonlyArray<string>
  descriptorRef: string
  freshness: ForgeSkillDescriptorFreshness
  policyRefs: ReadonlyArray<string>
  skillRef: string
  sourceRefs: ReadonlyArray<string>
  state: ForgeSkillDescriptorState
  summaryRefs: ReadonlyArray<string>
  triggerRefs: ReadonlyArray<string>
}>

export type ForgeSkillDescriptorCatalogCounts = Readonly<{
  available: number
  disabled: number
  failed: number
  needsReview: number
  pending: number
  total: number
}>

export type ForgeSkillDescriptorCatalogAuthority = Readonly<{
  contextInjectionAuthority: false
  providerAccountAuthority: false
  settlementAuthority: false
  toolCallAuthority: false
  workspaceWriteAuthority: false
}>

export type ForgeSkillDescriptorCatalogDisclosure = Readonly<{
  bodyIncludedByDefault: false
  defaultContextIncludesFullSkillBody: false
  explicitBodyRequestRequired: true
}>

export type ForgeSkillDescriptorCatalogView = Readonly<{
  authority: ForgeSkillDescriptorCatalogAuthority
  blockerRefs: ReadonlyArray<string>
  catalogRef: string
  counts: ForgeSkillDescriptorCatalogCounts
  disclosure: ForgeSkillDescriptorCatalogDisclosure
  entries: ReadonlyArray<ForgeSkillDescriptorEntry>
  freshness: ForgeSkillDescriptorFreshness
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  status: ForgeSkillDescriptorCatalogStatus
  workOrderRef: string
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_SKILL_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_SKILL_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:skill|body|file|source|shell|command|prompt|log|transcript|test)/i,
  /full[-_ ]skill[-_ ]body/i,
  /skill[-_ ]body[-_ ](?:content|full|raw|text)/i,
  /private[-_ ](?:repo|content|source|transcript|instructions?|skill|body)/i,
  /provider[-_ ]payload/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:api[-_ ]?key|bearer|token|secret|mnemonic|password)\b/i,
]

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_SKILL_REF_PATTERN.test(trimmed) &&
    !PRIVATE_SKILL_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeRefs = (refs: ReadonlyArray<string> | undefined): RefBundle => {
  const sanitized = (refs ?? []).reduce<Readonly<{ omitted: number; refs: string[] }>>(
    (state, ref) => {
      const safe = safeRef(ref)

      return safe === null
        ? { omitted: state.omitted + 1, refs: state.refs }
        : { omitted: state.omitted, refs: [...state.refs, safe] }
    },
    { omitted: 0, refs: [] },
  )

  return {
    omittedUnsafeRefCount: sanitized.omitted,
    refs: Array.from(new Set(sanitized.refs)),
  }
}

const safeOptionalRef = (value: string): OptionalRefBundle => {
  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (catalogRef: string, suffix: string): string =>
  `forge-skill-descriptor-catalog-blocker:${catalogRef}:${suffix}`

const STATE_PRIORITY: Readonly<Record<ForgeSkillDescriptorState, number>> = {
  needs_review: 0,
  failed: 1,
  pending: 2,
  available: 3,
  disabled: 4,
}

const entrySort = (
  left: ForgeSkillDescriptorEntry,
  right: ForgeSkillDescriptorEntry,
): number =>
  STATE_PRIORITY[left.state] - STATE_PRIORITY[right.state] ||
  left.skillRef.localeCompare(right.skillRef) ||
  left.descriptorRef.localeCompare(right.descriptorRef)

const normalizeEntry = (
  entry: ForgeSkillDescriptorInput,
): Readonly<{ entry: ForgeSkillDescriptorEntry | null; omitted: number }> => {
  const descriptorRef = safeOptionalRef(entry.descriptorRef)
  const skillRef = safeOptionalRef(entry.skillRef)
  const summaryRefs = safeRefs(entry.summaryRefs)
  const triggerRefs = safeRefs(entry.triggerRefs)
  const sourceRefs = safeRefs(entry.sourceRefs)
  const policyRefs = safeRefs(entry.policyRefs)
  const bodyRequestRefs = safeRefs(entry.bodyRequestRefs)
  const blockerRefs = safeRefs(entry.blockerRefs)
  const omitted =
    descriptorRef.omittedUnsafeRefCount +
    skillRef.omittedUnsafeRefCount +
    summaryRefs.omittedUnsafeRefCount +
    triggerRefs.omittedUnsafeRefCount +
    sourceRefs.omittedUnsafeRefCount +
    policyRefs.omittedUnsafeRefCount +
    bodyRequestRefs.omittedUnsafeRefCount +
    blockerRefs.omittedUnsafeRefCount

  return {
    entry:
      descriptorRef.ref === null || skillRef.ref === null
        ? null
        : {
            blockerRefs: blockerRefs.refs,
            bodyIncludedByDefault: false,
            bodyRequestRefs: bodyRequestRefs.refs,
            descriptorRef: descriptorRef.ref,
            freshness: entry.freshness ?? 'unknown',
            policyRefs: policyRefs.refs,
            skillRef: skillRef.ref,
            sourceRefs: sourceRefs.refs,
            state: entry.state ?? 'pending',
            summaryRefs: summaryRefs.refs,
            triggerRefs: triggerRefs.refs,
          },
    omitted,
  }
}

const catalogCounts = (
  entries: ReadonlyArray<ForgeSkillDescriptorEntry>,
): ForgeSkillDescriptorCatalogCounts => ({
  available: entries.filter(entry => entry.state === 'available').length,
  disabled: entries.filter(entry => entry.state === 'disabled').length,
  failed: entries.filter(entry => entry.state === 'failed').length,
  needsReview: entries.filter(entry => entry.state === 'needs_review').length,
  pending: entries.filter(entry => entry.state === 'pending').length,
  total: entries.length,
})

const catalogStatus = (
  counts: ForgeSkillDescriptorCatalogCounts,
  blockerRefs: ReadonlyArray<string>,
  freshness: ForgeSkillDescriptorFreshness,
): ForgeSkillDescriptorCatalogStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  if (freshness === 'stale') {
    return 'stale'
  }

  if (counts.total === 0) {
    return 'empty'
  }

  return counts.failed > 0 || counts.needsReview > 0 || counts.pending > 0
    ? 'needs_attention'
    : 'ready'
}

export const projectForgeSkillDescriptorCatalog = (
  input: ForgeSkillDescriptorCatalogInput,
): ForgeSkillDescriptorCatalogView => {
  const catalogRef = safeOptionalRef(input.catalogRef)
  const workOrderRef = safeOptionalRef(input.workOrderRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalizedEntries = (input.entries ?? []).map(normalizeEntry)
  const entries = normalizedEntries
    .flatMap(result => (result.entry === null ? [] : [result.entry]))
    .sort(entrySort)
  const freshness = input.freshness ?? 'unknown'
  const omittedUnsafeRefCount =
    catalogRef.omittedUnsafeRefCount +
    workOrderRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedEntries.reduce((sum, result) => sum + result.omitted, 0)
  const safeCatalogRef = catalogRef.ref ?? 'unsafe-skill-descriptor-catalog'
  const blockerRefs = Array.from(
    new Set([
      ...inputBlockerRefs.refs,
      ...entries.flatMap(entry => entry.blockerRefs),
      ...(catalogRef.ref === null
        ? [blockerRef(safeCatalogRef, 'missing-catalog-ref')]
        : []),
      ...(workOrderRef.ref === null
        ? [blockerRef(safeCatalogRef, 'missing-work-order-ref')]
        : []),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(safeCatalogRef, 'unsafe-skill-material-omitted')]),
    ]),
  )
  const counts = catalogCounts(entries)

  return {
    authority: {
      contextInjectionAuthority: false,
      providerAccountAuthority: false,
      settlementAuthority: false,
      toolCallAuthority: false,
      workspaceWriteAuthority: false,
    },
    blockerRefs,
    catalogRef: catalogRef.ref ?? 'unsafe-catalog-ref-omitted',
    counts,
    disclosure: {
      bodyIncludedByDefault: false,
      defaultContextIncludesFullSkillBody: false,
      explicitBodyRequestRequired: true,
    },
    entries,
    freshness,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    status: catalogStatus(counts, blockerRefs, freshness),
    workOrderRef: workOrderRef.ref ?? 'unsafe-work-order-ref-omitted',
  }
}
