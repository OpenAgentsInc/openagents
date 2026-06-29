import { containsProviderSecretMaterial } from '@openagentsinc/provider-account-schema'
import { Schema as S } from 'effect'

import {
  PublicClaimStateProjection,
  assertPublicClaimCopySafe,
  publicClaimStateProjection,
} from './public-claim-state'

export const PUBLIC_SITE_AGENT_CHALLENGE_VERSION =
  'openagents.site.challenge.v0.1'

export const PublicSiteAgentChallengeStatus = S.Literals([
  'planned',
  'open',
  'paused',
  'closed',
])
export type PublicSiteAgentChallengeStatus =
  typeof PublicSiteAgentChallengeStatus.Type

export const PublicSiteAgentContributionType = S.Literals([
  'proof_inspection',
  'research_source',
  'copy_improvement',
  'data_submission',
  'compute_offer',
  'funding_intent',
])
export type PublicSiteAgentContributionType =
  typeof PublicSiteAgentContributionType.Type

export class PublicSiteAgentChallenge extends S.Class<PublicSiteAgentChallenge>(
  'PublicSiteAgentChallenge',
)({
  version: S.Literal(PUBLIC_SITE_AGENT_CHALLENGE_VERSION),
  id: S.String,
  title: S.String,
  status: PublicSiteAgentChallengeStatus,
  contributionTypes: S.Array(PublicSiteAgentContributionType),
  challengeUrl: S.String,
  proofUrl: S.String,
  capabilityManifestUrl: S.String,
  openApiUrl: S.String,
  ownerClaimUrl: S.String,
  summary: S.String,
  instructions: S.String,
  requiredEvidence: S.Array(S.String),
  fundingStatus: S.Literal('planned_not_live'),
  fundingNote: S.String,
  acceptedOutcomeClaim: S.NullOr(S.String),
  claimState: PublicClaimStateProjection,
  caveats: S.Array(S.String),
}) {}

type PublicSiteAgentChallengesInput = Readonly<{
  capabilityManifestUrl?: string | undefined
  openApiUrl?: string | undefined
  ownerClaimUrl?: string | undefined
  proofUrl: string
  siteSlug?: string | null | undefined
  siteTitle?: string | null | undefined
}>

const assertChallengeCopySafe = (
  values: ReadonlyArray<string>,
): boolean => {
  try {
    for (const value of values) {
      assertPublicClaimCopySafe(value)
    }

    return true
  } catch {
    return false
  }
}

export const publicSiteAgentChallenges = (
  input: PublicSiteAgentChallengesInput,
): ReadonlyArray<PublicSiteAgentChallenge> => {
  const capabilityManifestUrl =
    input.capabilityManifestUrl ??
    'https://openagents.com/.well-known/openagents.json'
  const openApiUrl =
    input.openApiUrl ?? 'https://openagents.com/api/openapi.json'
  const ownerClaimUrl =
    input.ownerClaimUrl ?? 'https://openagents.com/onboarding'
  const title = `Improve public proof for ${input.siteTitle ?? 'the first OpenAgents Site'}`
  const summary =
    'Inspect the public OTEC proof, propose stronger public evidence or clearer copy, and request owner/operator review before any mutation.'
  const instructions = [
    'Read the public proof and capability documents first.',
    'Do not submit secrets, private customer data, unpublished artifacts, provider account refs, source archives, or runner logs.',
    'Prepare a concise proposal with public source links or before/after copy.',
    'Ask a signed-in owner or OpenAgents operator to review the proposal until scoped contribution APIs are live.',
  ].join(' ')
  const requiredEvidence = [
    'Public URL and source title for any proposed source.',
    'One-sentence explanation of how the source supports OTEC, SWAC, floating datacenter, or gigawatt-scale infrastructure claims.',
    'Before/after copy for any proposed wording change.',
    'The proof field or Site section the proposal would improve.',
  ]
  const fundingNote =
    'Funding, Lightning, L402, bounty, and reward settlement paths are planned and are not live for this challenge yet.'
  const caveats = [
    'This challenge invites public proposals only; it does not grant write authority.',
    'No accepted outcome, payment, reward, or settlement is claimed until a receipt exists.',
    'Use public evidence only and preserve the proof caveats.',
  ]

  if (
    !assertChallengeCopySafe([
      title,
      summary,
      instructions,
      ...requiredEvidence,
      fundingNote,
      ...caveats,
    ])
  ) {
    return []
  }

  const challenge = new PublicSiteAgentChallenge({
    version: PUBLIC_SITE_AGENT_CHALLENGE_VERSION,
    id: `${input.siteSlug ?? 'first-site'}-proof-copy-source-challenge`,
    title,
    status: 'open',
    contributionTypes: [
      'proof_inspection',
      'research_source',
      'copy_improvement',
    ],
    challengeUrl: `${input.proofUrl}#agent-challenges`,
    proofUrl: input.proofUrl,
    capabilityManifestUrl,
    openApiUrl,
    ownerClaimUrl,
    summary,
    instructions,
    requiredEvidence,
    fundingStatus: 'planned_not_live',
    fundingNote,
    acceptedOutcomeClaim: null,
    claimState: publicClaimStateProjection({
      desiredState: 'measured',
      evidenceRefs: [
        input.proofUrl,
        input.siteSlug === null || input.siteSlug === undefined
          ? 'site:first-public-site'
          : `site:${input.siteSlug}`,
      ],
      kind: 'agent_challenge',
      caveats,
    }),
    caveats,
  })

  return containsProviderSecretMaterial(JSON.stringify(challenge))
    ? []
    : [challenge]
}
