export type ForgeProductPromiseEvidenceFreshness = 'fresh' | 'stale' | 'unknown'

export type ForgeProductPromiseEvidenceGateStatus =
  | 'blocked'
  | 'ready'
  | 'stale'

export type ForgeProductPromiseEvidenceGateInput = Readonly<{
  blockerRefs?: ReadonlyArray<string>
  claimRefs?: ReadonlyArray<string>
  deployRefs?: ReadonlyArray<string>
  freshness?: ForgeProductPromiseEvidenceFreshness
  generatedAt: string
  gateRef: string
  liveSmokeRefs?: ReadonlyArray<string>
  productPromiseRefs?: ReadonlyArray<string>
  publicSafetyRefs?: ReadonlyArray<string>
  signatureRefs?: ReadonlyArray<string>
}>

export type ForgeProductPromiseEvidenceGateAuthority = Readonly<{
  deploymentAuthority: false
  productPromiseWriteAuthority: false
  publicClaimAuthority: false
  registryMutationAuthority: false
  settlementAuthority: false
}>

export type ForgeProductPromiseEvidenceGateCounts = Readonly<{
  claimRefs: number
  deployRefs: number
  liveSmokeRefs: number
  productPromiseRefs: number
  publicSafetyRefs: number
  signatureRefs: number
}>

export type ForgeProductPromiseEvidenceGateView = Readonly<{
  authority: ForgeProductPromiseEvidenceGateAuthority
  blockerRefs: ReadonlyArray<string>
  claimRefs: ReadonlyArray<string>
  counts: ForgeProductPromiseEvidenceGateCounts
  deployRefs: ReadonlyArray<string>
  freshness: ForgeProductPromiseEvidenceFreshness
  gateRef: string
  generatedAt: string
  liveSmokeRefs: ReadonlyArray<string>
  omittedUnsafeRefCount: number
  productPromiseRefs: ReadonlyArray<string>
  publicSafe: true
  publicSafetyRefs: ReadonlyArray<string>
  signatureRefs: ReadonlyArray<string>
  status: ForgeProductPromiseEvidenceGateStatus
}>

type RefBundle = Readonly<{
  omittedUnsafeRefCount: number
  refs: ReadonlyArray<string>
}>

type OptionalRefBundle = Readonly<{
  omittedUnsafeRefCount: number
  ref: string | null
}>

const SAFE_GATE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,260}$/
const PRIVATE_GATE_MARKERS: ReadonlyArray<RegExp> = [
  /diff --git/i,
  /^@@/m,
  /^[-+](?![-+])/m,
  /raw[-_ ](?:trace|transcript|file|source|shell|command|prompt|log|test|provider|payload)/i,
  /private[-_ ](?:repo|content|source|trace|transcript|instructions?|customer|workspace)/i,
  /provider[-_ ]payload/i,
  /customer[-_ ]private/i,
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

  return SAFE_GATE_REF_PATTERN.test(trimmed) &&
    !PRIVATE_GATE_MARKERS.some(marker => marker.test(trimmed))
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

const blockerRef = (gateRef: string, suffix: string): string =>
  `forge-product-promise-evidence-gate-blocker:${gateRef}:${suffix}`

const missingBlockers = (
  gateRef: string,
  refs: Readonly<{
    claimRefs: ReadonlyArray<string>
    deployRefs: ReadonlyArray<string>
    liveSmokeRefs: ReadonlyArray<string>
    productPromiseRefs: ReadonlyArray<string>
    publicSafetyRefs: ReadonlyArray<string>
    signatureRefs: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => [
  ...(refs.claimRefs.length === 0 ? [blockerRef(gateRef, 'missing-claim-ref')] : []),
  ...(refs.productPromiseRefs.length === 0
    ? [blockerRef(gateRef, 'missing-product-promise-ref')]
    : []),
  ...(refs.deployRefs.length === 0 ? [blockerRef(gateRef, 'missing-deploy-ref')] : []),
  ...(refs.liveSmokeRefs.length === 0
    ? [blockerRef(gateRef, 'missing-live-smoke-ref')]
    : []),
  ...(refs.signatureRefs.length === 0
    ? [blockerRef(gateRef, 'missing-signature-ref')]
    : []),
  ...(refs.publicSafetyRefs.length === 0
    ? [blockerRef(gateRef, 'missing-public-safety-ref')]
    : []),
]

const gateStatus = (
  blockerRefs: ReadonlyArray<string>,
  freshness: ForgeProductPromiseEvidenceFreshness,
): ForgeProductPromiseEvidenceGateStatus => {
  if (blockerRefs.length > 0) {
    return 'blocked'
  }

  return freshness === 'stale' ? 'stale' : 'ready'
}

export const projectForgeProductPromiseEvidenceGate = (
  input: ForgeProductPromiseEvidenceGateInput,
): ForgeProductPromiseEvidenceGateView => {
  const gateRef = safeOptionalRef(input.gateRef)
  const inputBlockerRefs = safeRefs(input.blockerRefs)
  const claimRefs = safeRefs(input.claimRefs)
  const productPromiseRefs = safeRefs(input.productPromiseRefs)
  const deployRefs = safeRefs(input.deployRefs)
  const liveSmokeRefs = safeRefs(input.liveSmokeRefs)
  const signatureRefs = safeRefs(input.signatureRefs)
  const publicSafetyRefs = safeRefs(input.publicSafetyRefs)
  const freshness = input.freshness ?? 'unknown'
  const omittedUnsafeRefCount =
    gateRef.omittedUnsafeRefCount +
    inputBlockerRefs.omittedUnsafeRefCount +
    claimRefs.omittedUnsafeRefCount +
    productPromiseRefs.omittedUnsafeRefCount +
    deployRefs.omittedUnsafeRefCount +
    liveSmokeRefs.omittedUnsafeRefCount +
    signatureRefs.omittedUnsafeRefCount +
    publicSafetyRefs.omittedUnsafeRefCount
  const safeGateRef = gateRef.ref ?? 'unsafe-product-promise-evidence-gate'
  const refs = {
    claimRefs: claimRefs.refs,
    deployRefs: deployRefs.refs,
    liveSmokeRefs: liveSmokeRefs.refs,
    productPromiseRefs: productPromiseRefs.refs,
    publicSafetyRefs: publicSafetyRefs.refs,
    signatureRefs: signatureRefs.refs,
  }
  const blockerRefs = Array.from(
    new Set([
      ...inputBlockerRefs.refs,
      ...(gateRef.ref === null ? [blockerRef(safeGateRef, 'missing-gate-ref')] : []),
      ...missingBlockers(safeGateRef, refs),
      ...(omittedUnsafeRefCount === 0
        ? []
        : [blockerRef(safeGateRef, 'unsafe-product-promise-evidence-omitted')]),
    ]),
  )

  return {
    authority: {
      deploymentAuthority: false,
      productPromiseWriteAuthority: false,
      publicClaimAuthority: false,
      registryMutationAuthority: false,
      settlementAuthority: false,
    },
    blockerRefs,
    claimRefs: refs.claimRefs,
    counts: {
      claimRefs: refs.claimRefs.length,
      deployRefs: refs.deployRefs.length,
      liveSmokeRefs: refs.liveSmokeRefs.length,
      productPromiseRefs: refs.productPromiseRefs.length,
      publicSafetyRefs: refs.publicSafetyRefs.length,
      signatureRefs: refs.signatureRefs.length,
    },
    deployRefs: refs.deployRefs,
    freshness,
    gateRef: gateRef.ref ?? 'unsafe-gate-ref-omitted',
    generatedAt: input.generatedAt,
    liveSmokeRefs: refs.liveSmokeRefs,
    omittedUnsafeRefCount,
    productPromiseRefs: refs.productPromiseRefs,
    publicSafe: true,
    publicSafetyRefs: refs.publicSafetyRefs,
    signatureRefs: refs.signatureRefs,
    status: gateStatus(blockerRefs, freshness),
  }
}
