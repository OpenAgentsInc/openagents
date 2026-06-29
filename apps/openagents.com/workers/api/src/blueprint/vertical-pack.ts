import { Schema as S } from 'effect'

import {
  BlueprintContextPack,
  type BlueprintContextPack as BlueprintContextPackType,
} from './schemas/source-context'

// Vertical Packs are reusable, customer-agnostic Context Pack templates for a
// given line of business. They scope the source authorities, data
// classification, consent posture, and public/customer-safe projection a
// Blueprint Program should assume for that vertical, plus an explicit
// ethical-marketing policy block that constrains any outbound marketing or
// communication work the Program performs on that vertical's behalf.
//
// A Vertical Pack must NOT name a specific customer, brand, or campaign. It is
// a generic template; concrete customer Context Packs are derived from it at
// runtime by binding real source authorities.

export const VerticalPackEthicalMarketingPolicy = S.Struct({
  // Stable id for this policy block so receipts/evidence can reference it.
  policyRef: S.String,
  // No fabricated testimonials, reviews, case studies, or social proof.
  noFabricatedTestimonials: S.Boolean,
  // No invented credentials, certifications, awards, or affiliations.
  noFabricatedCredentials: S.Boolean,
  // No fake scarcity, countdowns, or manufactured urgency.
  noFakeUrgency: S.Boolean,
  // Prefer clear, accurate description over hype or superlatives.
  clarityOverHype: S.Boolean,
  // Sensitive sends (claims, pricing, legal, outreach at scale) require a
  // human-in-the-loop approval before dispatch.
  humanInLoopOnSensitiveSends: S.Boolean,
  // Free-form, human-readable rules retained for review surfaces. These are
  // descriptive guidance, not authority; enforcement remains with the booleans
  // above and the Action Submission boundary.
  ruleRefs: S.Array(S.String),
})
export type VerticalPackEthicalMarketingPolicy =
  typeof VerticalPackEthicalMarketingPolicy.Type

export const VerticalPack = S.Struct({
  // Stable lookup id, e.g. "vertical_pack.services_business".
  id: S.String,
  // Generic vertical label, e.g. "services_business". Never a customer name.
  vertical: S.String,
  // Human-readable, customer-agnostic description of the vertical's posture.
  description: S.String,
  // The underlying Context Pack template (source authorities, classification,
  // consent, public/customer-safe projection flags).
  contextPack: BlueprintContextPack,
  // The ethical-marketing policy this vertical must operate under.
  ethicalMarketingPolicy: VerticalPackEthicalMarketingPolicy,
})
export type VerticalPack = typeof VerticalPack.Type

// Generic services-business / agency vertical pack. Source authorities below
// are template placeholders (sourceRef values describe the KIND of source a
// concrete pack would bind, not any real customer record).
export const servicesBusinessVerticalPack: VerticalPack = {
  id: 'vertical_pack.services_business',
  vertical: 'services_business',
  description:
    'Generic services / agency business vertical. Scopes the source ' +
    'authorities, data classification, consent posture, and public/customer-' +
    'safe projection a Blueprint Program should assume when operating for a ' +
    'professional-services or agency-style customer, with an ethical-' +
    'marketing policy applied to any outbound communication work.',
  contextPack: {
    createdAt: '2026-06-14T00:00:00.000Z',
    customerSafeProjection: true,
    dataClassification: 'customer',
    excludedContextRefs: [
      // Raw private customer correspondence is excluded from context by
      // default; only consented summaries flow through.
      'email.raw_customer_inbox',
    ],
    id: 'context_pack.services_business_template',
    includedContextRefs: [
      'order.services_engagement',
      'customer_asset.brand_kit',
      'exa_brief.market_positioning',
      'repo.public_marketing_site',
    ],
    publicSafeProjection: true,
    sourceAuthorities: [
      {
        // Public market research about the vertical (no customer specifics).
        classificationCaveatRef: 'classification.public_market_research',
        confidence: 'medium',
        consentState: 'public',
        customerSafe: true,
        dataClassification: 'public',
        excludedReasonRef: null,
        freshness: 'recent',
        includedInContext: true,
        publicSafe: true,
        publicSummaryRef: 'summary.market_positioning_public',
        sourceKind: 'exa_brief',
        sourceRef: 'exa_brief.market_positioning',
        trustTier: 'reviewed',
      },
      {
        // The signed services engagement / order, customer-consented.
        classificationCaveatRef: 'classification.services_engagement',
        confidence: 'high',
        consentState: 'customer_provided',
        customerSafe: true,
        dataClassification: 'customer',
        excludedReasonRef: null,
        freshness: 'current',
        includedInContext: true,
        publicSafe: false,
        publicSummaryRef: 'summary.services_engagement_public_safe',
        sourceKind: 'order',
        sourceRef: 'order.services_engagement',
        trustTier: 'verified',
      },
      {
        // Customer-provided brand assets (logos, voice, palette). Customer-safe
        // but not public until the customer publishes.
        classificationCaveatRef: 'classification.customer_brand_kit',
        confidence: 'high',
        consentState: 'customer_provided',
        customerSafe: true,
        dataClassification: 'customer',
        excludedReasonRef: null,
        freshness: 'current',
        includedInContext: true,
        publicSafe: false,
        publicSummaryRef: 'summary.brand_kit_customer_safe',
        sourceKind: 'customer_asset',
        sourceRef: 'customer_asset.brand_kit',
        trustTier: 'verified',
      },
      {
        // The customer's already-public marketing site is public-safe.
        classificationCaveatRef: 'classification.public_marketing_site',
        confidence: 'high',
        consentState: 'public',
        customerSafe: true,
        dataClassification: 'public',
        excludedReasonRef: null,
        freshness: 'current',
        includedInContext: true,
        publicSafe: true,
        publicSummaryRef: 'summary.public_marketing_site',
        sourceKind: 'repo',
        sourceRef: 'repo.public_marketing_site',
        trustTier: 'verified',
      },
      {
        // Raw private inbox correspondence is internal-only and excluded.
        classificationCaveatRef: 'classification.private_customer_inbox',
        confidence: 'medium',
        consentState: 'internal_only',
        customerSafe: false,
        dataClassification: 'private',
        excludedReasonRef: 'excluded.raw_customer_inbox',
        freshness: 'recent',
        includedInContext: false,
        publicSafe: false,
        publicSummaryRef: null,
        sourceKind: 'email',
        sourceRef: 'email.raw_customer_inbox',
        trustTier: 'reviewed',
      },
    ],
    trustTier: 'reviewed',
    updatedAt: '2026-06-14T00:00:00.000Z',
  },
  ethicalMarketingPolicy: {
    policyRef: 'policy.ethical_marketing.services_business',
    noFabricatedTestimonials: true,
    noFabricatedCredentials: true,
    noFakeUrgency: true,
    clarityOverHype: true,
    humanInLoopOnSensitiveSends: true,
    ruleRefs: [
      'rule.no_fabricated_testimonials_or_reviews',
      'rule.no_invented_credentials_or_affiliations',
      'rule.no_fake_scarcity_or_countdowns',
      'rule.describe_accurately_avoid_superlatives',
      'rule.human_approval_before_sensitive_send',
    ],
  },
}

// Registry of named vertical packs. Keyed by pack id for direct lookup.
export const verticalPackRegistry: Readonly<Record<string, VerticalPack>> = {
  [servicesBusinessVerticalPack.id]: servicesBusinessVerticalPack,
}

// Look up a vertical pack by its id. Returns undefined when unknown so callers
// can decide their own missing-pack policy.
export const getVerticalPack = (id: string): VerticalPack | undefined =>
  verticalPackRegistry[id]

// Re-export the underlying Context Pack type for convenience.
export type { BlueprintContextPackType }

/*
COORDINATOR WIRING: add the following export block to
workers/api/src/blueprint/index.ts (do NOT add it here):

export {
  getVerticalPack,
  servicesBusinessVerticalPack,
  VerticalPack,
  type VerticalPack as VerticalPackType,
  VerticalPackEthicalMarketingPolicy,
  type VerticalPackEthicalMarketingPolicy as VerticalPackEthicalMarketingPolicyType,
  verticalPackRegistry,
} from './vertical-pack'
*/
