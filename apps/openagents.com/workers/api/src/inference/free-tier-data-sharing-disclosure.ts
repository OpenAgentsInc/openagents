// Free-API data-sharing terms / consent disclosure (openagents #6296, child of
// #6293, epic #6206).
//
// WHY THIS EXISTS. Default-on free-tier trace capture is going live (#6293):
// free-tier `/api/v1/chat/completions` traffic is captured by default as
// REDACTED, PRIVATE (`owner_only`) ATIF traces, and MAY be used to improve /
// train OpenAgents models. Capturing free traffic for training without a clear,
// discoverable, honest disclosure is not acceptable (Episode 243 data-sharing
// thesis, docs/transcripts/243.md:74). This module is the SINGLE SOURCE OF TRUTH
// for that disclosure so the same accurate terms render at every surface — the
// free-key mint response (POST /api/keys/free), the public agent-readable
// endpoint (GET /api/public/free-tier-data-sharing), the public AGENTS.md, and
// the product-promise registry — instead of scattering copy that can drift.
//
// HONEST AND ACCURATE TO THE CODE. Every clause below maps to a real seam:
//   - "captured by default, redacted, private" -> khala-chat-trace-emitter.ts
//     (auto-capture stored `owner_only`, redactTraceValue scrub + tripwire
//     backstop, KHALA_AUTO_CAPTURE_VISIBILITY).
//   - "pay for privacy to opt out (fail-closed)" -> inference-privacy-
//     entitlement.ts (`captureDefault = freeTier.free && !paidPrivacy`,
//     fail-closed-to-private).
//   - "public sharing is opt-in only" -> auto-capture is never public; only an
//     explicit owner opt-in moves a trace toward `public`.
//   - "may be used to improve/train; no payout is granted" -> the data-market
//     reward marker stays INERT / owner-gated (#6221); capture grants no payout
//     or settlement.
//
// This module ships NO capture behavior and NO authority. It is disclosure text
// + bounded policy facts only.

// The stable promise id this disclosure is tracked under in docs/promises/ and
// the product-promise registry (apps/openagents.com/workers/api/src/
// product-promises.ts). Reports about the disclosure route through that id.
export const FREE_TIER_DATA_SHARING_PROMISE_ID =
  'data.free_tier_capture_disclosure.v1' as const

// Disclosure version. Bump when the TERMS text changes (not on unrelated copy
// edits). Surfaces echo this so a caller/agent can pin which terms they saw.
export const FREE_TIER_DATA_SHARING_DISCLOSURE_VERSION = '2026-06-29.1' as const

// Public, human-readable summary line. Intentionally short and quotable; the
// full clause list carries the precise terms.
export const FREE_TIER_DATA_SHARING_SUMMARY: string =
  'Free API usage is designed to be captured by default, when owner-armed, as ' +
  'redacted, private-by-default traces that may be used to improve and train ' +
  'OpenAgents models. Pay for privacy (or use confidential compute) to opt out ' +
  'of capture. Public sharing of a captured trace is opt-in only.'

// The precise, code-accurate terms, one bounded clause each. Public-safe: no
// secrets, no account material, no prompts.
export const FREE_TIER_DATA_SHARING_TERMS: ReadonlyArray<string> = [
  'Free tier: when the deployment owner arms default capture for the free Khala API (openagents/khala), free usage without paid privacy is captured by default; until then, the owner-gated blocker remains explicit.',
  'Redacted: captured traffic is scrubbed for secrets, credentials, wallet/payment material, and personal data before storage, with a public-safety backstop that drops anything residual.',
  'Private by default: an auto-captured trace is stored owner_only — it is not in any public feed and is not reachable by link until you explicitly choose to share it.',
  'May be used to improve/train: captured free-tier data may be used to improve and train the next generation of OpenAgents models.',
  'Pay for privacy to opt out: callers paying for privacy, or running confidential compute, are excluded from capture (fail-closed — when in doubt, not captured).',
  'Public sharing is opt-in only: a captured trace becomes public only if its owner explicitly opts it into public visibility.',
  'No payout is granted by capture: being captured grants no payment, payout, or settlement; the data-market reward marker is inert and owner-gated.',
] as const

// Bounded, machine-checkable policy facts. These mirror the runtime seams so an
// agent can reason over the policy without parsing prose.
export type FreeTierDataSharingPolicy = Readonly<{
  // Free-tier traffic is captured by default (subject to the deployment's
  // capture flag being armed; the POLICY default is capture-on for free tier).
  capturedByDefault: true
  // Default capture is still owner-gated until the deployment flag is armed.
  captureDefaultOwnerGated: true
  // Public-safe name of the deployment flag that arms default capture.
  captureDefaultGate: 'KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT'
  // Captured traffic is redacted before storage.
  redacted: true
  // Default stored visibility of an auto-captured trace.
  defaultVisibility: 'owner_only'
  // Captured free-tier data may be used to improve/train models.
  mayTrain: true
  // Paying for privacy / confidential compute opts the caller OUT of capture.
  paidPrivacyOptOut: true
  // A captured trace is public only on explicit owner opt-in.
  publicSharingOptIn: true
  // Capture grants no payout/settlement (reward marker inert, owner-gated).
  rewardInert: true
  // Public-safe blocker refs that explain why this disclosure is yellow.
  blockerRefs: ReadonlyArray<string>
}>

export const FREE_TIER_DATA_SHARING_POLICY: FreeTierDataSharingPolicy = {
  capturedByDefault: true,
  captureDefaultOwnerGated: true,
  captureDefaultGate: 'KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT',
  redacted: true,
  defaultVisibility: 'owner_only',
  mayTrain: true,
  paidPrivacyOptOut: true,
  publicSharingOptIn: true,
  rewardInert: true,
  blockerRefs: [
    'blocker.product_promises.free_tier_capture_default_owner_gated',
    'blocker.product_promises.disclosure_copy_owner_signoff_pending',
  ],
}

// The canonical disclosure object surfaced to humans and agents. Stable shape:
// version + summary + ordered terms + bounded policy facts + reference links.
export type FreeTierDataSharingDisclosure = Readonly<{
  promiseId: string
  version: string
  summary: string
  terms: ReadonlyArray<string>
  policy: FreeTierDataSharingPolicy
  // How to opt out of capture (pay for privacy / confidential compute).
  optOut: string
  // How to make a captured trace public (owner opt-in only).
  publicSharing: string
  // Where to report a disclosure mismatch (Forum-first, per repo policy).
  reportPath: string
  // Public references for the policy and its source.
  references: ReadonlyArray<string>
}>

export const FREE_TIER_DATA_SHARING_REPORT_PATH =
  'https://openagents.com/forum/f/product-promises' as const

// Build the canonical disclosure object. Pure; no IO, no env, no clock.
export const freeTierDataSharingDisclosure =
  (): FreeTierDataSharingDisclosure => ({
    promiseId: FREE_TIER_DATA_SHARING_PROMISE_ID,
    version: FREE_TIER_DATA_SHARING_DISCLOSURE_VERSION,
    summary: FREE_TIER_DATA_SHARING_SUMMARY,
    terms: FREE_TIER_DATA_SHARING_TERMS,
    policy: FREE_TIER_DATA_SHARING_POLICY,
    optOut:
      'Pay for privacy or run confidential compute to be excluded from capture entirely (fail-closed to not-captured).',
    publicSharing:
      'Auto-captured traces are private (owner_only). A trace is shared publicly only when its owner explicitly opts it into public visibility.',
    reportPath: FREE_TIER_DATA_SHARING_REPORT_PATH,
    references: [
      'https://openagents.com/docs/product-promises',
      'https://openagents.com/api/public/product-promises',
      'https://github.com/OpenAgentsInc/openagents/blob/main/docs/promises/2026-06-25-free-tier-data-sharing-disclosure.md',
      'https://github.com/OpenAgentsInc/openagents/blob/main/docs/traces/2026-06-25-default-on-trace-capture-audit.md',
    ],
  })
