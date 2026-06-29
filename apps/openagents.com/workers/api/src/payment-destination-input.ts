import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const OpenAgentsPaymentDestinationKind = S.Literals([
  'ambiguous',
  'bitcoin_uri',
  'bolt11',
  'bolt12',
  'cashu',
  'human_readable_name',
  'lightning_address',
  'lnurl',
  'malformed',
  'onchain_address',
  'unsupported',
])
export type OpenAgentsPaymentDestinationKind =
  typeof OpenAgentsPaymentDestinationKind.Type

export const OpenAgentsPaymentDestinationSource = S.Literals([
  'bitcoin_uri',
  'lightning_uri',
  'qr_payload',
  'raw_text',
  'uri_handler',
])
export type OpenAgentsPaymentDestinationSource =
  typeof OpenAgentsPaymentDestinationSource.Type

export const OpenAgentsPaymentDestinationNetworkHint = S.Literals([
  'bitcoin',
  'regtest',
  'signet',
  'testnet',
  'unknown',
])
export type OpenAgentsPaymentDestinationNetworkHint =
  typeof OpenAgentsPaymentDestinationNetworkHint.Type

export const OpenAgentsPaymentDestinationClassificationStatus = S.Literals([
  'ambiguous',
  'malformed',
  'requires_resolution',
  'supported_parse_only',
  'unsupported',
])
export type OpenAgentsPaymentDestinationClassificationStatus =
  typeof OpenAgentsPaymentDestinationClassificationStatus.Type

export const OpenAgentsPaymentDestinationRuntimeDecision = S.Literals([
  'rust_wasm_or_sidecar_required_for_resolution',
  'unsupported_without_resolver',
  'worker_lexical_parser',
])
export type OpenAgentsPaymentDestinationRuntimeDecision =
  typeof OpenAgentsPaymentDestinationRuntimeDecision.Type

export class OpenAgentsPaymentDestinationInput extends S.Class<OpenAgentsPaymentDestinationInput>(
  'OpenAgentsPaymentDestinationInput',
)({
  allowCashu: S.Boolean,
  allowNetworkResolution: S.Boolean,
  allowOnchain: S.Boolean,
  inputRef: S.String,
  rawInput: S.String,
  source: OpenAgentsPaymentDestinationSource,
}) {}

export class OpenAgentsPaymentDestinationProjection extends S.Class<OpenAgentsPaymentDestinationProjection>(
  'OpenAgentsPaymentDestinationProjection',
)({
  approvalRequired: S.Boolean,
  classificationStatus: OpenAgentsPaymentDestinationClassificationStatus,
  dispatchAllowed: S.Boolean,
  docsRefs: S.Array(S.String),
  inputRef: S.String,
  kind: OpenAgentsPaymentDestinationKind,
  methodRefs: S.Array(S.String),
  networkHint: OpenAgentsPaymentDestinationNetworkHint,
  payoutAuthorityCreated: S.Boolean,
  rawDestinationProjected: S.Boolean,
  reasonRefs: S.Array(S.String),
  redactedDestinationRef: S.String,
  requiresResolution: S.Boolean,
  runtimeDecision: OpenAgentsPaymentDestinationRuntimeDecision,
  safeSummary: S.String,
  source: OpenAgentsPaymentDestinationSource,
  sourceRefs: S.Array(S.String),
}) {}

export class OpenAgentsPaymentDestinationUnsafe extends S.TaggedErrorClass<OpenAgentsPaymentDestinationUnsafe>()(
  'OpenAgentsPaymentDestinationUnsafe',
  {
    reason: S.String,
  },
) {}

type ParsedPaymentCandidate = {
  readonly kind: OpenAgentsPaymentDestinationKind
  readonly methodRefs: ReadonlyArray<string>
  readonly networkHint: OpenAgentsPaymentDestinationNetworkHint
  readonly reasonRefs: ReadonlyArray<string>
  readonly requiresResolution: boolean
  readonly source: OpenAgentsPaymentDestinationSource
}

const docsRefs = [
  'docs.mdk.bitcoin_payment_instructions_source_audit',
  'docs.sites.payment_destination_input_parser',
]

const sourceRefs = [
  'source.moneydevkit.bitcoin_payment_instructions.d53d244',
  'source.moneydevkit.bitcoin_payment_instructions.version_0_7_0',
]

const paymentDestinationPrivateInputPattern =
  /(access[_-]?token|api[_-]?key|bearer\s+|checkout[_-]?secret|config\.json|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|mnemonic|payment[_-]?preimage|preimage=|provider[_-]?(credential|secret|token)|raw[_-]?(preimage|secret|wallet)|recovery[_-]?phrase|secret|seed[_-]?phrase|sk-[a-z0-9]|wallet[_/.-]?(config|key|material|mnemonic|preimage|secret|seed)|webhook[_-]?secret|\/\.mdk-wallet)/i

const projectionUnsafeValuePattern =
  /(@|bitcoin:|checkout[_-]?id=|creq[0-9a-z]+|lnbc[0-9a-z]+|lnbcrt[0-9a-z]+|lno1[0-9a-z]+|lntb[0-9a-z]+|lnurl1[0-9a-z]+|mdk[_-]?(access[_-]?token|mnemonic|webhook[_-]?secret)|payment[_-]?(hash|preimage)|preimage=|provider[_-]?token|raw[_-]?(invoice|payment|payload)|secret|wallet)/i

const bitcoinUriMethodParamKeys = new Set([
  'bc',
  'creq',
  'lightning',
  'lno',
  'tb',
])

const bitcoinUriAllowedParamKeys = new Set([
  'amount',
  'bc',
  'creq',
  'label',
  'lightning',
  'lno',
  'message',
  'pop',
  'tb',
])

const safeInputRefPattern = /^[A-Za-z0-9][A-Za-z0-9_.:/-]{0,240}$/

const uniqueSorted = (
  values: ReadonlyArray<string>,
): ReadonlyArray<string> =>
  [...new Set(values.map(value => value.trim()).filter(value => value !== ''))]
    .sort()

const stringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === 'string') {
    return [value]
  }

  if (Array.isArray(value)) {
    return value.flatMap(stringValues)
  }

  if (value !== null && typeof value === 'object') {
    return Object.values(value as Record<string, unknown>).flatMap(stringValues)
  }

  return []
}

export const openAgentsPaymentDestinationHasPrivateMaterial = (
  value: unknown,
): boolean =>
  stringValues(value).some(text =>
    containsProviderSecretMaterial(text) ||
    projectionUnsafeValuePattern.test(text)
  )

const assertSafeProjection = (
  projection: OpenAgentsPaymentDestinationProjection,
): void => {
  if (!safeInputRefPattern.test(projection.inputRef)) {
    throw new OpenAgentsPaymentDestinationUnsafe({
      reason: 'Payment destination inputRef must be a stable public-safe ref.',
    })
  }

  if (projection.rawDestinationProjected) {
    throw new OpenAgentsPaymentDestinationUnsafe({
      reason: 'Payment destination projections must never expose raw input.',
    })
  }

  if (openAgentsPaymentDestinationHasPrivateMaterial(projection)) {
    throw new OpenAgentsPaymentDestinationUnsafe({
      reason:
        'Payment destination projection contains a raw invoice, address, preimage, wallet material, provider credential, or private destination.',
    })
  }
}

const assertSafeInput = (input: OpenAgentsPaymentDestinationInput): void => {
  if (!safeInputRefPattern.test(input.inputRef)) {
    throw new OpenAgentsPaymentDestinationUnsafe({
      reason: 'Payment destination inputRef must be a stable public-safe ref.',
    })
  }

  if (input.rawInput.trim().length === 0 || input.rawInput.length > 4096) {
    throw new OpenAgentsPaymentDestinationUnsafe({
      reason:
        'Payment destination input must be non-empty and shorter than 4096 bytes.',
    })
  }

  if (
    paymentDestinationPrivateInputPattern.test(input.rawInput) ||
    containsProviderSecretMaterial(input.rawInput)
  ) {
    throw new OpenAgentsPaymentDestinationUnsafe({
      reason:
        'Payment destination input contains wallet material, preimage, provider credential, or private secret material.',
    })
  }
}

const networkHintFromText = (
  text: string,
): OpenAgentsPaymentDestinationNetworkHint => {
  const lower = text.toLowerCase()

  if (lower.startsWith('lntbs') || lower.includes('lntbs')) {
    return 'signet'
  }

  if (
    lower.startsWith('lntb') ||
    lower.startsWith('tb1') ||
    lower.includes('lntb') ||
    lower.includes('tb1')
  ) {
    return 'testnet'
  }

  if (
    lower.startsWith('lnbcrt') ||
    lower.startsWith('bcrt1') ||
    lower.includes('lnbcrt') ||
    lower.includes('bcrt1')
  ) {
    return 'regtest'
  }

  if (
    lower.startsWith('lnbc') ||
    lower.startsWith('bc1') ||
    lower.includes('lnbc') ||
    lower.includes('bc1') ||
    lower.startsWith('bitcoin:')
  ) {
    return 'bitcoin'
  }

  return 'unknown'
}

const candidateFromKind = (
  input: {
    kind: OpenAgentsPaymentDestinationKind
    methodRef: string
    reasonRef: string
    requiresResolution?: boolean
    source: OpenAgentsPaymentDestinationSource
    text: string
  },
): ParsedPaymentCandidate => ({
  kind: input.kind,
  methodRefs: [input.methodRef],
  networkHint: networkHintFromText(input.text),
  reasonRefs: [input.reasonRef],
  requiresResolution: input.requiresResolution ?? false,
  source: input.source,
})

const isBolt11 = (text: string): boolean =>
  /^(lnbc|lntb|lnbcrt)[0-9a-z]{40,}$/u.test(text.toLowerCase())

const isBolt12 = (text: string): boolean =>
  /^lno1[0-9a-z]{40,}$/u.test(text.toLowerCase())

const isLnurl = (text: string): boolean =>
  /^lnurl1[0-9a-z]{30,}$/u.test(text.toLowerCase())

const isCashu = (text: string): boolean =>
  /^creq[0-9a-z]{20,}$/u.test(text.toLowerCase())

const isOnchainAddress = (text: string): boolean =>
  /^(bc1|tb1|bcrt1)[0-9a-z]{20,}$/u.test(text.toLowerCase()) ||
  /^[13][A-Za-z0-9]{25,40}$/u.test(text)

const isLightningAddress = (text: string): boolean =>
  /^[a-z0-9._+-]{1,64}@[a-z0-9.-]{1,190}\.[a-z]{2,63}$/u.test(
    text.toLowerCase(),
  )

const isHumanReadableName = (text: string): boolean =>
  /^[a-z0-9._+-]{1,64}@[a-z0-9.-]{1,190}$/u.test(text.toLowerCase()) &&
  !isLightningAddress(text)

const classifyRawPayload = (
  text: string,
  source: OpenAgentsPaymentDestinationSource,
  input: OpenAgentsPaymentDestinationInput,
): ParsedPaymentCandidate => {
  const normalized = text.trim()

  if (isBolt11(normalized)) {
    return candidateFromKind({
      kind: 'bolt11',
      methodRef: 'method.lightning.bolt11',
      reasonRef: 'reason.payment_destination.bolt11_parse_only',
      source,
      text: normalized,
    })
  }

  if (isBolt12(normalized)) {
    return candidateFromKind({
      kind: 'bolt12',
      methodRef: 'method.lightning.bolt12_offer',
      reasonRef: 'reason.payment_destination.bolt12_parse_only',
      source,
      text: normalized,
    })
  }

  if (isLnurl(normalized)) {
    return candidateFromKind({
      kind: 'lnurl',
      methodRef: 'method.lightning.lnurl',
      reasonRef: 'reason.payment_destination.lnurl_requires_resolution',
      requiresResolution: true,
      source,
      text: normalized,
    })
  }

  if (isLightningAddress(normalized)) {
    return candidateFromKind({
      kind: 'lightning_address',
      methodRef: 'method.lightning.address',
      reasonRef:
        'reason.payment_destination.lightning_address_requires_resolution',
      requiresResolution: true,
      source,
      text: normalized,
    })
  }

  if (isHumanReadableName(normalized)) {
    return candidateFromKind({
      kind: 'human_readable_name',
      methodRef: 'method.bitcoin.bip353',
      reasonRef: 'reason.payment_destination.hrn_requires_dns_resolution',
      requiresResolution: true,
      source,
      text: normalized,
    })
  }

  if (isOnchainAddress(normalized)) {
    return input.allowOnchain
      ? candidateFromKind({
        kind: 'onchain_address',
        methodRef: 'method.bitcoin.onchain',
        reasonRef: 'reason.payment_destination.onchain_parse_only',
        source,
        text: normalized,
      })
      : {
        kind: 'unsupported',
        methodRefs: [],
        networkHint: networkHintFromText(normalized),
        reasonRefs: ['reason.payment_destination.onchain_disabled'],
        requiresResolution: false,
        source,
      }
  }

  if (isCashu(normalized)) {
    return input.allowCashu
      ? candidateFromKind({
        kind: 'cashu',
        methodRef: 'method.cashu.payment_request',
        reasonRef: 'reason.payment_destination.cashu_parse_only',
        source,
        text: normalized,
      })
      : {
        kind: 'unsupported',
        methodRefs: [],
        networkHint: 'unknown',
        reasonRefs: ['reason.payment_destination.cashu_disabled'],
        requiresResolution: false,
        source,
      }
  }

  return {
    kind: normalized.toLowerCase().startsWith('ln') ? 'malformed' : 'unsupported',
    methodRefs: [],
    networkHint: networkHintFromText(normalized),
    reasonRefs: normalized.toLowerCase().startsWith('ln')
      ? ['reason.payment_destination.lightning_payload_malformed']
      : ['reason.payment_destination.unsupported_format'],
    requiresResolution: false,
    source,
  }
}

const queryPairs = (query: string): ReadonlyArray<readonly [string, string]> =>
  Array.from(new URLSearchParams(query).entries()).map(([key, value]) => [
    key.toLowerCase(),
    value,
  ] as const)

const parseBitcoinUri = (
  raw: string,
  input: OpenAgentsPaymentDestinationInput,
): ParsedPaymentCandidate => {
  const afterScheme = raw.trim().slice('bitcoin:'.length)
  const [addressPart = '', query = ''] = afterScheme.split('?', 2)
  const pairs = queryPairs(query)
  const unsupportedRequired = pairs.find(([key]) =>
    key.startsWith('req-') &&
    !bitcoinUriAllowedParamKeys.has(key.slice('req-'.length))
  )

  if (unsupportedRequired !== undefined) {
    return {
      kind: 'malformed',
      methodRefs: [],
      networkHint: networkHintFromText(raw),
      reasonRefs: [
        'reason.payment_destination.bitcoin_uri_unknown_required_parameter',
      ],
      requiresResolution: false,
      source: 'bitcoin_uri',
    }
  }

  const methodPairs = pairs.filter(([key]) =>
    bitcoinUriMethodParamKeys.has(
      key.startsWith('req-') ? key.slice('req-'.length) : key,
    )
  )
  const methodRefs = [
    addressPart !== '' && isOnchainAddress(addressPart)
      ? 'method.bitcoin.onchain'
      : undefined,
    ...methodPairs.map(([key, value]) => {
      const methodKey = key.startsWith('req-') ? key.slice('req-'.length) : key
      const candidate = value.toLowerCase()

      if (methodKey === 'lightning' && isLnurl(candidate)) {
        return 'method.lightning.lnurl'
      }

      if (methodKey === 'lightning') {
        return 'method.lightning.bolt11'
      }

      if (methodKey === 'lno') {
        return 'method.lightning.bolt12_offer'
      }

      if (methodKey === 'creq') {
        return 'method.cashu.payment_request'
      }

      return methodKey === 'bc' || methodKey === 'tb'
        ? 'method.bitcoin.onchain'
        : undefined
    }),
  ].filter((value): value is string => value !== undefined)
  const distinctMethodRefs = uniqueSorted(methodRefs)

  if (distinctMethodRefs.length === 0) {
    return {
      kind: 'malformed',
      methodRefs: [],
      networkHint: networkHintFromText(raw),
      reasonRefs: ['reason.payment_destination.bitcoin_uri_no_method'],
      requiresResolution: false,
      source: 'bitcoin_uri',
    }
  }

  if (!input.allowOnchain && distinctMethodRefs.includes('method.bitcoin.onchain')) {
    return {
      kind: 'unsupported',
      methodRefs: distinctMethodRefs.filter(ref => ref !== 'method.bitcoin.onchain'),
      networkHint: networkHintFromText(raw),
      reasonRefs: ['reason.payment_destination.bitcoin_uri_onchain_disabled'],
      requiresResolution: false,
      source: 'bitcoin_uri',
    }
  }

  if (
    !input.allowCashu &&
    distinctMethodRefs.includes('method.cashu.payment_request')
  ) {
    return {
      kind: 'unsupported',
      methodRefs: distinctMethodRefs.filter(ref => ref !== 'method.cashu.payment_request'),
      networkHint: networkHintFromText(raw),
      reasonRefs: ['reason.payment_destination.bitcoin_uri_cashu_disabled'],
      requiresResolution: false,
      source: 'bitcoin_uri',
    }
  }

  return {
    kind: 'bitcoin_uri',
    methodRefs: distinctMethodRefs,
    networkHint: networkHintFromText(raw),
    reasonRefs: ['reason.payment_destination.bitcoin_uri_structured_methods'],
    requiresResolution: distinctMethodRefs.includes('method.lightning.lnurl'),
    source: 'bitcoin_uri',
  }
}

const extractLnurlFallback = (raw: string): string | undefined => {
  const lower = raw.toLowerCase()
  const index = lower.indexOf('lnurl1')

  if (index < 0) {
    return undefined
  }

  return raw
    .slice(index)
    .split(/[&#\s]/u, 1)[0]
}

const likelyCandidateKindsInRawText = (
  raw: string,
): ReadonlyArray<OpenAgentsPaymentDestinationKind> => {
  const lower = raw.toLowerCase()

  return uniqueSorted([
    /(lnbc|lntb|lnbcrt)[0-9a-z]{40,}/u.test(lower) ? 'bolt11' : '',
    /lno1[0-9a-z]{40,}/u.test(lower) ? 'bolt12' : '',
    /lnurl1[0-9a-z]{30,}/u.test(lower) ? 'lnurl' : '',
    /bitcoin:/u.test(lower) ? 'bitcoin_uri' : '',
  ]).filter((value): value is OpenAgentsPaymentDestinationKind =>
    value !== '',
  )
}

const sourceForInput = (
  raw: string,
  configuredSource: OpenAgentsPaymentDestinationSource,
): OpenAgentsPaymentDestinationSource =>
  raw.trim().toLowerCase().startsWith('lightning:')
    ? 'lightning_uri'
    : raw.trim().toLowerCase().startsWith('bitcoin:')
      ? 'bitcoin_uri'
      : configuredSource

const parseCandidate = (
  input: OpenAgentsPaymentDestinationInput,
): ParsedPaymentCandidate => {
  const trimmed = input.rawInput.trim()
  const source = sourceForInput(trimmed, input.source)
  const lower = trimmed.toLowerCase()
  const likelyKinds = source === 'raw_text' || source === 'qr_payload'
    ? likelyCandidateKindsInRawText(trimmed)
    : []

  if (likelyKinds.length > 1 && !lower.startsWith('bitcoin:')) {
    return {
      kind: 'ambiguous',
      methodRefs: likelyKinds.map(kind => `method.candidate.${kind}`),
      networkHint: networkHintFromText(trimmed),
      reasonRefs: ['reason.payment_destination.multiple_candidates'],
      requiresResolution: likelyKinds.includes('lnurl'),
      source,
    }
  }

  if (lower.startsWith('bitcoin:')) {
    return parseBitcoinUri(trimmed, input)
  }

  if (lower.startsWith('lightning:')) {
    return classifyRawPayload(trimmed.slice('lightning:'.length), source, input)
  }

  const lnurlFallback = extractLnurlFallback(trimmed)

  if (lnurlFallback !== undefined && isLnurl(lnurlFallback)) {
    return candidateFromKind({
      kind: 'lnurl',
      methodRef: 'method.lightning.lnurl',
      reasonRef: 'reason.payment_destination.lnurl_embedded_qr_payload',
      requiresResolution: true,
      source: 'qr_payload',
      text: lnurlFallback,
    })
  }

  return classifyRawPayload(trimmed, source, input)
}

const statusForCandidate = (
  candidate: ParsedPaymentCandidate,
  allowNetworkResolution: boolean,
): OpenAgentsPaymentDestinationClassificationStatus => {
  if (candidate.kind === 'ambiguous') {
    return 'ambiguous'
  }

  if (candidate.kind === 'malformed') {
    return 'malformed'
  }

  if (candidate.kind === 'unsupported') {
    return 'unsupported'
  }

  return candidate.requiresResolution && !allowNetworkResolution
    ? 'requires_resolution'
    : 'supported_parse_only'
}

const runtimeDecisionForCandidate = (
  candidate: ParsedPaymentCandidate,
  allowNetworkResolution: boolean,
): OpenAgentsPaymentDestinationRuntimeDecision => {
  if (candidate.kind === 'unsupported' || candidate.kind === 'malformed') {
    return 'unsupported_without_resolver'
  }

  return candidate.requiresResolution && !allowNetworkResolution
    ? 'rust_wasm_or_sidecar_required_for_resolution'
    : 'worker_lexical_parser'
}

const redactedDestinationRefForCandidate = (
  inputRef: string,
  candidate: ParsedPaymentCandidate,
): string =>
  [
    'payment_destination',
    candidate.kind,
    candidate.source,
    candidate.networkHint,
    inputRef.replace(/[^A-Za-z0-9_.:/-]/gu, '_'),
  ].join('.')

const summaryForCandidate = (
  candidate: ParsedPaymentCandidate,
): string => {
  if (candidate.kind === 'ambiguous') {
    return 'Multiple possible payment destination formats were found.'
  }

  if (candidate.kind === 'malformed') {
    return 'The payment destination looked like a payment instruction but failed bounded parsing.'
  }

  if (candidate.kind === 'unsupported') {
    return 'The payment destination format is not accepted by this Omega boundary.'
  }

  if (candidate.requiresResolution) {
    return 'The payment destination parsed to a resolvable payment instruction; resolution remains outside the Worker parser.'
  }

  return 'The payment destination parsed to a supported payment instruction class.'
}

export const classifyOpenAgentsPaymentDestinationInput = (
  input: OpenAgentsPaymentDestinationInput,
): OpenAgentsPaymentDestinationProjection => {
  assertSafeInput(input)

  const candidate = parseCandidate(input)
  const projection: OpenAgentsPaymentDestinationProjection = {
    approvalRequired: true,
    classificationStatus: statusForCandidate(
      candidate,
      input.allowNetworkResolution,
    ),
    dispatchAllowed: false,
    docsRefs,
    inputRef: input.inputRef,
    kind: candidate.kind,
    methodRefs: uniqueSorted(candidate.methodRefs),
    networkHint: candidate.networkHint,
    payoutAuthorityCreated: false,
    rawDestinationProjected: false,
    reasonRefs: uniqueSorted(candidate.reasonRefs),
    redactedDestinationRef: redactedDestinationRefForCandidate(
      input.inputRef,
      candidate,
    ),
    requiresResolution: candidate.requiresResolution,
    runtimeDecision: runtimeDecisionForCandidate(
      candidate,
      input.allowNetworkResolution,
    ),
    safeSummary: summaryForCandidate(candidate),
    source: candidate.source,
    sourceRefs,
  }

  assertSafeProjection(projection)

  return projection
}
