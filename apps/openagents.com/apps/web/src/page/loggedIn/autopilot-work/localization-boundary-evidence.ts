import type {
  AutopilotWorkLocalizationEntry,
  AutopilotWorkLocalizationFreshness,
  AutopilotWorkLocalizationScope,
  AutopilotWorkLocalizationStatus,
  AutopilotWorkProjection,
} from '../model'

export type ForgeLocalizationBoundaryStatus =
  | 'blocked'
  | 'empty'
  | 'ready'
  | 'stale'
  | 'unknown'

export type ForgeLocalizationBoundaryAuthority = Readonly<{
  acceptedOutcomeAuthority: false
  catalogExecutionAuthority: false
  commandIdMutationAuthority: false
  jsonSchemaMutationAuthority: false
  localePreferenceWriteAuthority: false
  localeRuntimeMutationAuthority: false
  paymentLanguageMutationAuthority: false
  permissionPromptMutationAuthority: false
  publicReceiptMutationAuthority: false
  settlementAuthority: false
  toolIdMutationAuthority: false
  workerPayoutAuthority: false
}>

export type ForgeLocalizationBoundaryItem = Readonly<{
  blockerRefs: ReadonlyArray<string>
  catalogRefs: ReadonlyArray<string>
  catalogValidationRefs: ReadonlyArray<string>
  commandIdStabilityRefs: ReadonlyArray<string>
  fallbackRefs: ReadonlyArray<string>
  formatterRefs: ReadonlyArray<string>
  freshness: AutopilotWorkLocalizationFreshness
  jsonSchemaStabilityRefs: ReadonlyArray<string>
  localePreferenceRefs: ReadonlyArray<string>
  localeRefs: ReadonlyArray<string>
  localizationRef: string
  missingTranslationRefs: ReadonlyArray<string>
  paymentLanguageReviewRefs: ReadonlyArray<string>
  permissionActionRefs: ReadonlyArray<string>
  permissionIdStabilityRefs: ReadonlyArray<string>
  permissionPolicyRefs: ReadonlyArray<string>
  publicReceiptStabilityRefs: ReadonlyArray<string>
  scope: AutopilotWorkLocalizationScope
  stableIdBoundaryRefs: ReadonlyArray<string>
  status: AutopilotWorkLocalizationStatus
  toolIdStabilityRefs: ReadonlyArray<string>
}>

export type ForgeLocalizationBoundaryInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  entries?: ReadonlyArray<AutopilotWorkLocalizationEntry>
  generatedAt: string
  snapshotRef?: string
  versionRef?: string | null
  workOrderRef: string
}>

export type ForgeLocalizationBoundaryCounts = Readonly<{
  catalogs: number
  fallbacks: number
  ready: number
  stableBoundaries: number
  stale: number
  total: number
}>

export type ForgeLocalizationBoundaryView = Readonly<{
  authority: ForgeLocalizationBoundaryAuthority
  blockerRefs: ReadonlyArray<string>
  counts: ForgeLocalizationBoundaryCounts
  entries: ReadonlyArray<ForgeLocalizationBoundaryItem>
  generatedAt: string
  omittedUnsafeRefCount: number
  publicSafe: true
  snapshotRef: string | null
  status: ForgeLocalizationBoundaryStatus
  versionRef: string | null
  workOrderRef: string
}>

type RefBundle = Readonly<{ omittedUnsafeRefCount: number; refs: ReadonlyArray<string> }>
type OptionalRefBundle = Readonly<{ omittedUnsafeRefCount: number; ref: string | null }>

const SAFE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_LOCALIZATION_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:catalog|copy|language|localized|locale|message|payload|prompt|translation)/i,
  /private[-_ ](?:catalog|content|copy|customer|data|language|locale|message|payload|prompt|translation|workspace)/i,
  /customer[-_ ](?:copy|language|message|payload|translation)/i,
  /localized[-_ ](?:copy|content|string|text)/i,
  /payment[-_ ]payload/i,
  /provider[-_ ]payload/i,
  /prompt[-_ ]text/i,
  /schema[-_ ]key[-_ ]translation/i,
  /translated[-_ ](?:command|field|identifier|permission|receipt|ref|schema|tool)/i,
  /translation[-_ ]content/i,
  /(?:^|\s)\/Users\//,
  /(?:^|\s)\/home\//,
  /(?:^|\s)(?:\.\/|\.\.\/|~\/)/,
  /(?:^|\s)(?:git|ssh|https?):\/\//i,
  /git@/i,
  /(?:;|&&|\|\||`|\$\(|>|<)/,
  /\b(?:gho|ghp|sk)-[A-Za-z0-9_/-]+/i,
  /\b(?:access[_-]?token|api[-_ ]?key|bearer|credential|mnemonic|password|preimage|secret)\b/i,
  /\b(?:admin|auth|bearer|operator|private|refresh|secret|service|session)[_-]?token\b/i,
]

const authority: ForgeLocalizationBoundaryAuthority = {
  acceptedOutcomeAuthority: false,
  catalogExecutionAuthority: false,
  commandIdMutationAuthority: false,
  jsonSchemaMutationAuthority: false,
  localePreferenceWriteAuthority: false,
  localeRuntimeMutationAuthority: false,
  paymentLanguageMutationAuthority: false,
  permissionPromptMutationAuthority: false,
  publicReceiptMutationAuthority: false,
  settlementAuthority: false,
  toolIdMutationAuthority: false,
  workerPayoutAuthority: false,
}

const safeRef = (value: string): string | null => {
  const trimmed = value.trim()

  return SAFE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_LOCALIZATION_MARKERS.some(marker => marker.test(trimmed))
    ? trimmed
    : null
}

const safeRefs = (
  ...groups: ReadonlyArray<ReadonlyArray<string> | undefined>
): RefBundle => {
  const refs = groups.flatMap(group => group ?? [])
  const sanitized = refs.reduce<Readonly<{ omitted: number; refs: string[] }>>(
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

const safeOptionalRef = (value: string | null | undefined): OptionalRefBundle => {
  if (value === null || value === undefined) {
    return { omittedUnsafeRefCount: 0, ref: null }
  }

  const ref = safeRef(value)

  return ref === null
    ? { omittedUnsafeRefCount: 1, ref: null }
    : { omittedUnsafeRefCount: 0, ref }
}

const blockerRef = (workOrderRef: string, suffix: string): string =>
  `forge-localization-boundary-blocker:${workOrderRef}:${suffix}`

const normalizeItem = (
  item: AutopilotWorkLocalizationEntry,
): Readonly<{
  item: ForgeLocalizationBoundaryItem | null
  omittedUnsafeRefCount: number
}> => {
  const blockerRefs = safeRefs(item.blockerRefs)
  const catalogRefs = safeRefs(item.catalogRefs)
  const catalogValidationRefs = safeRefs(item.catalogValidationRefs)
  const commandIdStabilityRefs = safeRefs(item.commandIdStabilityRefs)
  const fallbackRefs = safeRefs(item.fallbackRefs)
  const formatterRefs = safeRefs(item.formatterRefs)
  const jsonSchemaStabilityRefs = safeRefs(item.jsonSchemaStabilityRefs)
  const localePreferenceRefs = safeRefs(item.localePreferenceRefs)
  const localeRefs = safeRefs(item.localeRefs)
  const localizationRef = safeOptionalRef(item.localizationRef)
  const missingTranslationRefs = safeRefs(item.missingTranslationRefs)
  const paymentLanguageReviewRefs = safeRefs(item.paymentLanguageReviewRefs)
  const permissionActionRefs = safeRefs(item.permissionActionRefs)
  const permissionIdStabilityRefs = safeRefs(item.permissionIdStabilityRefs)
  const permissionPolicyRefs = safeRefs(item.permissionPolicyRefs)
  const publicReceiptStabilityRefs = safeRefs(item.publicReceiptStabilityRefs)
  const stableIdBoundaryRefs = safeRefs(item.stableIdBoundaryRefs)
  const toolIdStabilityRefs = safeRefs(item.toolIdStabilityRefs)
  const omittedUnsafeRefCount =
    blockerRefs.omittedUnsafeRefCount +
    catalogRefs.omittedUnsafeRefCount +
    catalogValidationRefs.omittedUnsafeRefCount +
    commandIdStabilityRefs.omittedUnsafeRefCount +
    fallbackRefs.omittedUnsafeRefCount +
    formatterRefs.omittedUnsafeRefCount +
    jsonSchemaStabilityRefs.omittedUnsafeRefCount +
    localePreferenceRefs.omittedUnsafeRefCount +
    localeRefs.omittedUnsafeRefCount +
    localizationRef.omittedUnsafeRefCount +
    missingTranslationRefs.omittedUnsafeRefCount +
    paymentLanguageReviewRefs.omittedUnsafeRefCount +
    permissionActionRefs.omittedUnsafeRefCount +
    permissionIdStabilityRefs.omittedUnsafeRefCount +
    permissionPolicyRefs.omittedUnsafeRefCount +
    publicReceiptStabilityRefs.omittedUnsafeRefCount +
    stableIdBoundaryRefs.omittedUnsafeRefCount +
    toolIdStabilityRefs.omittedUnsafeRefCount

  return localizationRef.ref === null
    ? { item: null, omittedUnsafeRefCount }
    : {
        item: {
          blockerRefs: blockerRefs.refs,
          catalogRefs: catalogRefs.refs,
          catalogValidationRefs: catalogValidationRefs.refs,
          commandIdStabilityRefs: commandIdStabilityRefs.refs,
          fallbackRefs: fallbackRefs.refs,
          formatterRefs: formatterRefs.refs,
          freshness: item.freshness ?? 'unknown',
          jsonSchemaStabilityRefs: jsonSchemaStabilityRefs.refs,
          localePreferenceRefs: localePreferenceRefs.refs,
          localeRefs: localeRefs.refs,
          localizationRef: localizationRef.ref,
          missingTranslationRefs: missingTranslationRefs.refs,
          paymentLanguageReviewRefs: paymentLanguageReviewRefs.refs,
          permissionActionRefs: permissionActionRefs.refs,
          permissionIdStabilityRefs: permissionIdStabilityRefs.refs,
          permissionPolicyRefs: permissionPolicyRefs.refs,
          publicReceiptStabilityRefs: publicReceiptStabilityRefs.refs,
          scope: item.scope,
          stableIdBoundaryRefs: stableIdBoundaryRefs.refs,
          status: item.status,
          toolIdStabilityRefs: toolIdStabilityRefs.refs,
        },
        omittedUnsafeRefCount,
      }
}

const counts = (
  entries: ReadonlyArray<ForgeLocalizationBoundaryItem>,
): ForgeLocalizationBoundaryCounts => ({
  catalogs: entries.filter(entry => entry.catalogRefs.length > 0).length,
  fallbacks: entries.filter(entry => entry.fallbackRefs.length > 0).length,
  ready: entries.filter(entry => entry.status === 'ready').length,
  stableBoundaries: entries.filter(entry => entry.stableIdBoundaryRefs.length > 0).length,
  stale: entries.filter(entry => entry.freshness === 'stale' || entry.status === 'stale').length,
  total: entries.length,
})

const readyContractMissing = (item: ForgeLocalizationBoundaryItem): boolean =>
  item.status === 'ready' &&
  (item.localePreferenceRefs.length === 0 ||
    item.catalogRefs.length === 0 ||
    item.catalogValidationRefs.length === 0 ||
    item.fallbackRefs.length === 0 ||
    item.stableIdBoundaryRefs.length === 0 ||
    item.formatterRefs.length === 0)

const permissionBoundaryMissing = (
  item: ForgeLocalizationBoundaryItem,
): boolean =>
  item.scope === 'permission_prompt' &&
  (item.permissionActionRefs.length === 0 ||
    item.permissionPolicyRefs.length === 0 ||
    item.permissionIdStabilityRefs.length === 0)

const itemBlockers = (
  workOrderRef: string,
  item: ForgeLocalizationBoundaryItem,
): ReadonlyArray<string> => {
  const blockers = [...item.blockerRefs]

  if (item.freshness === 'stale' || item.status === 'stale') {
    blockers.push(
      blockerRef(workOrderRef, `stale-localization-boundary-evidence:${item.localizationRef}`),
    )
  }

  if (readyContractMissing(item)) {
    blockers.push(
      blockerRef(workOrderRef, `ready-localization-boundary-missing:${item.localizationRef}`),
    )
  }

  if (permissionBoundaryMissing(item)) {
    blockers.push(
      blockerRef(workOrderRef, `permission-prompt-stability-missing:${item.localizationRef}`),
    )
  }

  if (item.scope === 'payment' && item.paymentLanguageReviewRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `payment-language-review-missing:${item.localizationRef}`),
    )
  }

  if (
    item.scope === 'public_receipt' &&
    item.publicReceiptStabilityRefs.length === 0
  ) {
    blockers.push(
      blockerRef(workOrderRef, `public-receipt-language-stability-missing:${item.localizationRef}`),
    )
  }

  if (item.scope === 'json_schema' && item.jsonSchemaStabilityRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `json-schema-language-stability-missing:${item.localizationRef}`),
    )
  }

  if (
    item.scope === 'command' &&
    (item.commandIdStabilityRefs.length === 0 ||
      item.toolIdStabilityRefs.length === 0)
  ) {
    blockers.push(
      blockerRef(workOrderRef, `command-tool-id-stability-missing:${item.localizationRef}`),
    )
  }

  if (item.missingTranslationRefs.length > 0 && item.fallbackRefs.length === 0) {
    blockers.push(
      blockerRef(workOrderRef, `missing-translation-fallback-missing:${item.localizationRef}`),
    )
  }

  return blockers
}

const statusFrom = (
  entries: ReadonlyArray<ForgeLocalizationBoundaryItem>,
  blockers: ReadonlyArray<string>,
): ForgeLocalizationBoundaryStatus => {
  if (blockers.length > 0 || entries.some(entry => entry.status === 'blocked')) {
    return 'blocked'
  }
  if (entries.length === 0) {
    return 'empty'
  }
  if (entries.some(entry => entry.freshness === 'stale' || entry.status === 'stale')) {
    return 'stale'
  }
  if (entries.every(entry => entry.status === 'ready')) {
    return 'ready'
  }
  return 'unknown'
}

export const projectForgeLocalizationBoundaryEvidence = (
  input: ForgeLocalizationBoundaryInput,
): ForgeLocalizationBoundaryView => {
  const snapshotRef = safeOptionalRef(input.snapshotRef)
  const versionRef = safeOptionalRef(input.versionRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const normalized = (input.entries ?? []).map(normalizeItem)
  const entries = normalized.flatMap(result => (result.item === null ? [] : [result.item]))
  const normalizedOmissions = normalized.reduce(
    (total, result) => total + result.omittedUnsafeRefCount,
    0,
  )
  const omittedUnsafeRefCount =
    snapshotRef.omittedUnsafeRefCount +
    versionRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    normalizedOmissions
  const blockers = [
    ...inputBlockerRefs.refs,
    ...entries.flatMap(entry => itemBlockers(input.workOrderRef, entry)),
  ]

  if (input.entries !== undefined && input.entries.length > 0 && snapshotRef.ref === null) {
    blockers.push(blockerRef(input.workOrderRef, 'missing-localization-boundary-snapshot-ref'))
  }
  if (omittedUnsafeRefCount > 0) {
    blockers.push(blockerRef(input.workOrderRef, 'unsafe-localization-material-omitted'))
  }

  const uniqueBlockers = Array.from(new Set(blockers))

  return {
    authority,
    blockerRefs: uniqueBlockers,
    counts: counts(entries),
    entries,
    generatedAt: input.generatedAt,
    omittedUnsafeRefCount,
    publicSafe: true,
    snapshotRef: snapshotRef.ref,
    status: statusFrom(entries, uniqueBlockers),
    versionRef: versionRef.ref,
    workOrderRef: input.workOrderRef,
  }
}

export const buildForgeLocalizationBoundaryInput = (
  work: AutopilotWorkProjection,
): ForgeLocalizationBoundaryInput => {
  const evidence = work.localizationBoundaryEvidence

  return {
    generatedAt: evidence?.generatedAt ?? work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(evidence?.blockerRefs === undefined
      ? {}
      : { blockerRefs: evidence.blockerRefs }),
    ...(evidence?.entries === undefined ? {} : { entries: evidence.entries }),
    ...(evidence?.snapshotRef === undefined
      ? {}
      : { snapshotRef: evidence.snapshotRef }),
    ...(evidence?.versionRef === undefined
      ? {}
      : { versionRef: evidence.versionRef }),
  }
}
