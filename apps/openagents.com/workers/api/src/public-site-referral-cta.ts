import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

export const PUBLIC_SITE_REFERRAL_CTA_VERSION =
  'openagents.site.referral_cta.v0.1'

export class PublicSiteReferralCta extends S.Class<PublicSiteReferralCta>(
  'PublicSiteReferralCta',
)({
  version: S.Literal(PUBLIC_SITE_REFERRAL_CTA_VERSION),
  title: S.String,
  label: S.String,
  description: S.String,
  siteSlug: S.NullOr(S.String),
  siteTitle: S.NullOr(S.String),
  openAgentsJoinUrl: S.String,
  referralJoinUrl: S.String,
  agentReferralJoinUrl: S.String,
  copyableAgentInstruction: S.String,
  caveats: S.Array(S.String),
}) {}

type PublicSiteReferralCtaInput = Readonly<{
  baseUrl?: string | undefined
  publicSourceRef: string
  siteSlug?: string | null | undefined
  siteTitle?: string | null | undefined
}>

const SAFE_SOURCE_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9_.:-]{0,190}$/
const SAFE_SLUG_PATTERN = /^[a-z0-9][a-z0-9-]{0,120}$/
const PROHIBITED_REF_PATTERN =
  /\b(lnbc|lntb|lnbcrt|lno1|mnemonic|xprv|payment_preimage|mdk_access_token|wallet_secret|private_key|webhook_secret|token_hash|bearer|gho_)/i

const safePublicValue = (value: string): boolean =>
  !containsProviderSecretMaterial(value) && !PROHIBITED_REF_PATTERN.test(value)

const safeSourceRef = (value: string): boolean =>
  SAFE_SOURCE_REF_PATTERN.test(value) && safePublicValue(value)

const safeOptionalSlug = (value: string | null | undefined): boolean =>
  value === null ||
  value === undefined ||
  (SAFE_SLUG_PATTERN.test(value) && safePublicValue(value))

const captureUrl = (
  baseUrl: string,
  publicSourceRef: string,
  target: 'agent' | 'order',
): string =>
  `${baseUrl}/r/site/${encodeURIComponent(publicSourceRef)}?target=${target}${
    target === 'agent' ? '&path=agent' : ''
  }`

const copyableInstruction = (
  siteTitle: string | null,
  agentReferralJoinUrl: string,
): string =>
  [
    `You are inspecting an OpenAgents public Site${siteTitle === null ? '' : ` for ${siteTitle}`}.`,
    `To request your own OpenAgents Site while preserving this source attribution, open ${agentReferralJoinUrl}.`,
    'Use the OpenAgents-hosted capture URL only. Do not copy referral state into public product URLs, generated source, manifests, screenshots, logs, emails, or payment payloads.',
    'This referral path may record attribution under OpenAgents policy. It is not a payout promise or an authorization grant.',
  ].join('\n')

export const publicSiteReferralCta = (
  input: PublicSiteReferralCtaInput,
): PublicSiteReferralCta | null => {
  const baseUrl = input.baseUrl ?? 'https://openagents.com'
  const siteSlug = input.siteSlug ?? null
  const siteTitle = input.siteTitle ?? null

  if (
    !safeSourceRef(input.publicSourceRef) ||
    !safePublicValue(baseUrl) ||
    !safeOptionalSlug(siteSlug) ||
    (siteTitle !== null && !safePublicValue(siteTitle))
  ) {
    return null
  }

  const openAgentsJoinUrl = captureUrl(baseUrl, input.publicSourceRef, 'order')
  const referralJoinUrl = openAgentsJoinUrl
  const agentReferralJoinUrl = captureUrl(
    baseUrl,
    input.publicSourceRef,
    'agent',
  )
  const projection = new PublicSiteReferralCta({
    version: PUBLIC_SITE_REFERRAL_CTA_VERSION,
    title: 'Get your own OpenAgents Site',
    label: 'Start your Site request',
    description:
      'Request a website, web app, tool, game, or public page through OpenAgents while preserving this public Site as the source.',
    siteSlug,
    siteTitle,
    openAgentsJoinUrl,
    referralJoinUrl,
    agentReferralJoinUrl,
    copyableAgentInstruction: copyableInstruction(
      siteTitle,
      agentReferralJoinUrl,
    ),
    caveats: [
      'This is a public referral path, not a payout promise.',
      'OpenAgents records attribution only through hosted capture routes.',
      'Do not put referral secrets, payment material, wallet material, provider grants, or private user data into generated Site source.',
    ],
  })

  return containsProviderSecretMaterial(JSON.stringify(projection)) ||
    PROHIBITED_REF_PATTERN.test(JSON.stringify(projection))
    ? null
    : projection
}
