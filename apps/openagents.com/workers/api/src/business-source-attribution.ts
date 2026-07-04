export type BusinessSourceKind =
  | 'content'
  | 'outbound'
  | 'ai_search'
  | 'referral'
  | 'direct'
  | 'unknown'

export const BUSINESS_SOURCE_REF_DIRECT = 'direct' as const
export const BUSINESS_SOURCE_REF_UNKNOWN = 'unknown' as const

export type BusinessSourceRefDecode =
  | Readonly<{ sourceRef: string }>
  | Readonly<{ reason: string }>

const SOURCE_REF_MAX_LENGTH = 80
const SOURCE_REF_TOKEN_PATTERN =
  /^(direct|unknown|ai_search|own_your_ai|apollo_model_custody|apollo_agent_readiness_[a-z0-9][a-z0-9_-]{0,63}|affiliate_[a-z0-9][a-z0-9_-]{0,63}|partner_[a-z0-9][a-z0-9_-]{0,63}|content_[a-z0-9][a-z0-9_-]{0,63}|vertical_[a-z0-9][a-z0-9_-]{0,63})$/
const UNSAFE_SOURCE_REF_PATTERN =
  /@|https?:\/\/|www\.|[/?#&=]|(?:^|[_-])(utm|email|phone|contact|customer|client|raw|payload|token|secret|key|mnemonic|xprv)(?:$|[_-])/i

const aliasSourceRef = (value: string): string => {
  const normalized = value.trim().toLowerCase()
  if (
    normalized === 'ai-search' ||
    normalized === 'aisearch' ||
    normalized === 'ai search'
  ) {
    return 'ai_search'
  }
  if (normalized === 'partner-expansion' || normalized === 'partner expansion') {
    return 'partner_expansion'
  }
  if (normalized === 'own-your-ai' || normalized === 'own your ai') {
    return 'own_your_ai'
  }
  if (
    normalized === 'apollo-model-custody' ||
    normalized === 'apollo model custody' ||
    normalized === 'model-custody' ||
    normalized === 'model custody'
  ) {
    return 'apollo_model_custody'
  }
  return normalized
}

export const isBusinessSourceRef = (value: string): boolean =>
  value.length <= SOURCE_REF_MAX_LENGTH &&
  SOURCE_REF_TOKEN_PATTERN.test(value) &&
  !UNSAFE_SOURCE_REF_PATTERN.test(value)

export const decodeBusinessSourceRef = (
  value: unknown,
  fallback: string = BUSINESS_SOURCE_REF_DIRECT,
): BusinessSourceRefDecode => {
  if (value === undefined || value === null) {
    return { sourceRef: fallback }
  }
  if (typeof value !== 'string') {
    return { reason: 'sourceRef must be a bounded public-safe token' }
  }

  const sourceRef = aliasSourceRef(value)
  if (sourceRef === '') {
    return { sourceRef: fallback }
  }
  if (!isBusinessSourceRef(sourceRef)) {
    return { reason: 'sourceRef must be a bounded public-safe token' }
  }

  return { sourceRef }
}

export const coerceStoredBusinessSourceRef = (
  value: string | null | undefined,
): string => {
  const decoded = decodeBusinessSourceRef(value, BUSINESS_SOURCE_REF_DIRECT)
  return 'sourceRef' in decoded ? decoded.sourceRef : BUSINESS_SOURCE_REF_UNKNOWN
}

export const businessSourceKindForSourceRef = (
  sourceRef: string,
): BusinessSourceKind => {
  if (sourceRef === 'direct') {
    return 'direct'
  }
  if (sourceRef === 'ai_search') {
    return 'ai_search'
  }
  if (sourceRef === 'unknown') {
    return 'unknown'
  }
  if (sourceRef.startsWith('content_') || sourceRef.startsWith('vertical_')) {
    return 'content'
  }
  if (sourceRef.startsWith('affiliate_') || sourceRef.startsWith('partner_')) {
    return 'referral'
  }
  if (
    sourceRef.startsWith('apollo_agent_readiness_') ||
    sourceRef === 'own_your_ai' ||
    sourceRef === 'apollo_model_custody'
  ) {
    return 'outbound'
  }
  return 'unknown'
}

const safeRefPart = (value: string): string =>
  value
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/gu, '_')
    .replace(/^_+|_+$/gu, '')

export const businessSourceRefForReferralCode = (
  referralCode: string,
): string => {
  const part = safeRefPart(referralCode).slice(0, 63)
  const sourceRef = part === '' ? 'affiliate_unknown' : `affiliate_${part}`
  return isBusinessSourceRef(sourceRef) ? sourceRef : 'affiliate_unknown'
}

export const businessSourceRefForVertical = (slug: string): string => {
  const part = safeRefPart(slug).slice(0, 63)
  const sourceRef = part === '' ? 'vertical_unknown' : `vertical_${part}`
  return isBusinessSourceRef(sourceRef) ? sourceRef : 'vertical_unknown'
}
