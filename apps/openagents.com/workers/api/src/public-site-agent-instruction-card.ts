import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import type { OpenAgentsSiteAgentSurfacePreset } from './site-source-metadata'
import {
  PublicSiteReferralCta,
  publicSiteReferralCta,
} from './public-site-referral-cta'

export const PUBLIC_SITE_AGENT_INSTRUCTION_CARD_VERSION =
  'openagents.site.agent_card.v0.1'

export const OpenAgentsPublicAgentAction = S.Literals([
  'inspect_public_proof',
  'inspect_capability_manifest',
  'inspect_openapi',
  'summarize_public_status',
  'propose_site_improvement',
  'request_owner_review',
])
export type OpenAgentsPublicAgentAction =
  typeof OpenAgentsPublicAgentAction.Type

export class PublicSiteAgentInstructionCard extends S.Class<PublicSiteAgentInstructionCard>(
  'PublicSiteAgentInstructionCard',
)({
  version: S.Literal(PUBLIC_SITE_AGENT_INSTRUCTION_CARD_VERSION),
  title: S.String,
  preset: S.String,
  siteSlug: S.NullOr(S.String),
  siteTitle: S.NullOr(S.String),
  siteUrl: S.NullOr(S.String),
  proofUrl: S.NullOr(S.String),
  capabilityManifestUrl: S.String,
  openApiUrl: S.String,
  instructionDocUrl: S.String,
  allowedActions: S.Array(OpenAgentsPublicAgentAction),
  requiresOwnerClaimForMutation: S.Boolean,
  referralCta: S.NullOr(PublicSiteReferralCta),
  copyableInstruction: S.String,
  caveats: S.Array(S.String),
}) {}

type PublicSiteAgentInstructionCardInput = Readonly<{
  capabilityManifestUrl?: string | undefined
  openApiUrl?: string | undefined
  instructionDocUrl?: string | undefined
  proofUrl?: string | null | undefined
  publicSourceRef?: string | null | undefined
  preset?: OpenAgentsSiteAgentSurfacePreset | undefined
  siteSlug?: string | null | undefined
  siteTitle?: string | null | undefined
  siteUrl?: string | null | undefined
}>

const publicAgentActionsForPreset = (
  preset: string,
): ReadonlyArray<OpenAgentsPublicAgentAction> =>
  preset === 'none'
    ? ['inspect_capability_manifest', 'inspect_openapi']
    : [
        'inspect_public_proof',
        'inspect_capability_manifest',
        'inspect_openapi',
        'summarize_public_status',
        'propose_site_improvement',
        'request_owner_review',
      ]

const cardCaveats = (
  input: PublicSiteAgentInstructionCardInput,
): ReadonlyArray<string> => [
  'This card is a public discovery aid, not an authorization grant.',
  'Do not send secrets, private data, tokens, account material, unpublished artifacts, provider account refs, source archives, or private runner logs.',
  'Mutating actions require signed-in owner authority, a scoped API key, owner claim, or a future credits/L402 path.',
  ...(input.proofUrl === null || input.proofUrl === undefined
    ? ['No public proof URL is available yet; inspect public discovery documents only.']
    : []),
  ...(input.siteUrl === null || input.siteUrl === undefined
    ? ['No public Site URL is available yet; inspect the proof URL and wait for a deployment receipt before claiming a live Site.']
    : []),
]

const instructionText = (input: {
  allowedActions: ReadonlyArray<OpenAgentsPublicAgentAction>
  capabilityManifestUrl: string
  openApiUrl: string
  instructionDocUrl: string
  referralCta: PublicSiteReferralCta | null
  proofUrl: string | null
  siteTitle: string | null
  siteUrl: string | null
}): string =>
  [
    `You are being sent to inspect an OpenAgents public Site${input.siteTitle === null ? '' : ` for ${input.siteTitle}`}.`,
    'Read this instruction before taking action.',
    'Do not send secrets, private data, tokens, account material, unpublished artifacts, provider account refs, source archives, or private runner logs.',
    `First inspect ${input.capabilityManifestUrl} and ${input.openApiUrl}.`,
    `Then read ${input.instructionDocUrl}.`,
    input.proofUrl === null
      ? 'No public proof URL is available yet; limit yourself to public dry-run discovery.'
      : `Inspect the public Site proof at ${input.proofUrl}.`,
    input.siteUrl === null
      ? 'Do not claim a live deployment exists until a public deployment URL appears in proof.'
      : `Inspect the deployed public Site at ${input.siteUrl}.`,
    input.referralCta === null
      ? 'No OpenAgents Site referral join URL is attached to this card.'
      : `To request your own OpenAgents Site from this source, use ${input.referralCta.agentReferralJoinUrl}.`,
    `Allowed public actions: ${input.allowedActions.join(', ')}.`,
    'For non-public actions, request signed-in browser session authority, a scoped API key, owner claim, or a future credits/L402 path before mutating anything.',
  ].join('\n')

export const publicSiteAgentInstructionCard = (
  input: PublicSiteAgentInstructionCardInput,
): PublicSiteAgentInstructionCard | null => {
  const preset = input.preset ?? 'basic'

  if (preset === 'none') {
    return null
  }

  const capabilityManifestUrl =
    input.capabilityManifestUrl ??
    'https://openagents.com/.well-known/openagents.json'
  const openApiUrl =
    input.openApiUrl ?? 'https://openagents.com/api/openapi.json'
  const instructionDocUrl =
    input.instructionDocUrl ?? 'https://openagents.com/AGENTS.md'
  const proofUrl = input.proofUrl ?? null
  const siteUrl = input.siteUrl ?? null
  const allowedActions = publicAgentActionsForPreset(preset)
  const referralCta =
    input.publicSourceRef === null || input.publicSourceRef === undefined
      ? null
      : publicSiteReferralCta({
          publicSourceRef: input.publicSourceRef,
          siteSlug: input.siteSlug,
          siteTitle: input.siteTitle,
        })
  const card = new PublicSiteAgentInstructionCard({
    version: PUBLIC_SITE_AGENT_INSTRUCTION_CARD_VERSION,
    title: 'Send your agent to this Site',
    preset,
    siteSlug: input.siteSlug ?? null,
    siteTitle: input.siteTitle ?? null,
    siteUrl,
    proofUrl,
    capabilityManifestUrl,
    openApiUrl,
    instructionDocUrl,
    allowedActions: [...allowedActions],
    requiresOwnerClaimForMutation: true,
    referralCta,
    copyableInstruction: instructionText({
      allowedActions,
      capabilityManifestUrl,
      openApiUrl,
      instructionDocUrl,
      referralCta,
      proofUrl,
      siteTitle: input.siteTitle ?? null,
      siteUrl,
    }),
    caveats: [...cardCaveats(input)],
  })

  return containsProviderSecretMaterial(JSON.stringify(card)) ? null : card
}
