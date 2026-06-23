// Onboarding program system prompt (EPIC #6123, issue #6126).
//
// The Khala onboarding program drives the productized OpenAgents Business intake
// interview from docs/business/2026-06-20-openagents-business-intake-spec.md over
// the OpenAI-compatible inference gateway. The system prompt is assembled
// DETERMINISTICALLY every turn from three parts so it never drifts from the live
// product reality:
//
//   1. The intake interview script (7 areas asked one at a time, branch logic,
//      land on one quick win + a relationship picture, fill the 10-section
//      Output Spec).
//   2. An HONESTY CONTRACT bound to the LIVE product-promise registry
//      (product-promises.ts): only sell green / operator-assisted surfaces, mark
//      roadmap as roadmap, never promise beyond what the registry supports. This
//      block is regenerated from `publicProductPromisesDocument()` so a promise
//      flip in source changes what the onboarding agent may sell — no second copy
//      of the truth.
//   3. A server-owned VERTICAL GUIDANCE slot — extra vertical guidance selected
//      from the bounded Autopilot Concierge vertical enum. Callers never supply
//      raw prompt text for this slot.
//
// Pure + transport-agnostic: this returns a string. Voice (STT -> route -> TTS)
// can layer on later without touching the prompt.

import {
  type AutopilotConciergeVertical,
  buildAutopilotConciergeVerticalGuidance,
} from './inference/autopilot-concierge-model'
import { publicProductPromisesDocument } from './product-promises'

// The promise areas the offerings menu maps onto. We surface the live state of a
// curated, stable set of registry promiseIds so the honesty contract reflects
// exactly what is sellable today rather than a hand-maintained restatement.
// Each entry ties an offerings-menu item to the promiseIds that gate it.
type OfferingPromiseBinding = Readonly<{
  offering: string
  promiseIds: ReadonlyArray<string>
}>

const OFFERING_PROMISE_BINDINGS: ReadonlyArray<OfferingPromiseBinding> = [
  {
    offering: '1. Coding & agent work',
    promiseIds: [
      'labor.forum_work_requests.v1',
      'autopilot.codex_probe_pylon_successor.v1',
      'autopilot.desktop_gui_client.v1',
    ],
  },
  {
    offering: '2. Inference / AI',
    promiseIds: [
      'inference.fireworks_open_model_provider.v1',
      'inference.gateway_credits_business.v1',
      'api.hosted_gemini.v1',
    ],
  },
  {
    offering: '3. Sites + commerce',
    promiseIds: [
      'autopilot_sites.native_email_sequences.v1',
      'autopilot_sites.custom_tenant_hostnames.v1',
      'sites.referral_bitcoin_stream.v1',
    ],
  },
  {
    offering: '4. Autopilot business automation',
    promiseIds: [
      'autopilot.all_in_one_business_system.v1',
      'workrooms.omni_client_delivery_workrooms.v1',
      'autopilot.repo_study_packets.v1',
    ],
  },
  {
    offering: '5. Distributed compute / training',
    promiseIds: [
      'training.decentralized_training_launch.v1',
      'cloud.fine_tuning_service.v1',
      'cloud.sandbox_compute_service.v1',
      'training.device_capability_dataset.v1',
    ],
  },
  {
    offering: '6. Forum / community',
    promiseIds: [
      'labor.forum_work_requests.v1',
      'labor.nostr_negotiation_market.v1',
    ],
  },
  {
    offering: '7. Payments rails',
    promiseIds: [
      'payments.accepted_outcome_economics.v1',
      'sites.referral_bitcoin_stream.v1',
    ],
  },
]

// Map a registry state to the customer-facing availability label the intake spec
// uses (Available now / Operator-assisted / Roadmap). `green` is the only state
// that sells as "available now"; `yellow`/`degraded` are operator-assisted (live
// but caveated/gated); `planned`/`red`/`withdrawn` are roadmap or unavailable.
const availabilityLabel = (state: string): string => {
  if (state === 'green') {
    return 'Available now'
  }
  if (state === 'yellow' || state === 'degraded') {
    return 'Operator-assisted'
  }
  return 'Roadmap'
}

// A registry snapshot keyed for fast lookup, taken once per prompt build so the
// honesty contract is internally consistent within a turn.
type PromiseSnapshot = Readonly<{
  promiseId: string
  state: string
  productArea: string
  unsafeCopy: string
}>

const snapshotRegistry = (): ReadonlyMap<string, PromiseSnapshot> => {
  const document = publicProductPromisesDocument()
  const entries = document.promises.map(
    (promise): readonly [string, PromiseSnapshot] => [
      promise.promiseId,
      {
        promiseId: promise.promiseId,
        productArea: promise.productArea,
        state: promise.state,
        unsafeCopy: promise.unsafeCopy,
      },
    ],
  )

  return new Map(entries)
}

// Render the live honesty contract block from the current registry snapshot.
// Each offering lists its gating promises with their live availability label, so
// the agent can read exactly what it may sell and what it must mark as roadmap.
export const buildHonestyContractBlock = (): string => {
  const registry = snapshotRegistry()
  const document = publicProductPromisesDocument()

  const offeringLines = OFFERING_PROMISE_BINDINGS.flatMap(binding => {
    const promiseLines = binding.promiseIds.flatMap(promiseId => {
      const snapshot = registry.get(promiseId)
      if (snapshot === undefined) {
        return []
      }
      return [
        `    - ${promiseId} (${snapshot.productArea}): ${availabilityLabel(snapshot.state)} [registry state: ${snapshot.state}]`,
      ]
    })

    return [`  ${binding.offering}:`, ...promiseLines]
  })

  return [
    'HONESTY CONTRACT (bound to the live product-promise registry).',
    `Registry version: ${document.version}. This is the single source of truth for what you may sell.`,
    'Rules:',
    '  - Only present a surface as "Available now" when its gating promise is green in the registry below.',
    '  - Present yellow/degraded surfaces only as "Operator-assisted": live but caveated, behind a flag, or needing a human/operator path. Say so in writing.',
    '  - Present red/planned/withdrawn surfaces only as "Roadmap": not shipped. Never imply they are live.',
    '  - If the human asks for something the registry does not support as live, say so plainly and capture it as an open question in section 10. Do NOT promise it.',
    '  - Never invent capabilities, settlement guarantees, payout authority, or scale claims beyond what the registry states.',
    '',
    'Live availability of the offerings (from the registry):',
    ...offeringLines,
    '',
    `The full agent-readable registry is at ${document.canonicalDocsUrl ? 'https://openagents.com/api/public/product-promises' : 'https://openagents.com/api/public/product-promises'} — when in doubt, defer to it and to a human/operator scope, never to optimism.`,
  ].join('\n')
}

// The intake interview script, lifted faithfully from the intake spec. Kept as a
// constant so the interview behavior is auditable and stable across turns.
const INTAKE_INTERVIEW_SCRIPT = `You are the OpenAgents Business onboarding agent. You run a short, friendly intake interview to help a potential customer pick ONE fast quick win plus a picture of the ongoing relationship — not a giant project. Keep the human's time short; aim for a quick win they could see in days.

What OpenAgents sells: machine work with receipts. AI agents and compute that do real, useful work, where every accepted outcome is tied to verifiable evidence. Start with a fast quick win, then put parts of the business on Autopilot as trust builds, scoping payment up front and only as work is accepted.

INTERVIEW METHOD (critical):
  - Ask ONE area at a time, in a natural conversation. Do NOT dump all questions at once.
  - After each area, briefly summarize back what you heard before moving on.
  - Skip questions that obviously do not apply, and use the branch guidance below.
  - Land on one or two offerings that fit, with their honest availability.

THE SEVEN AREAS (ask in order, one area per turn):
  A. Business & goals: what the business does; customers / main product; the single most important outcome they want from OpenAgents in the next month. Branch: if they cannot name an outcome, ask what took too much of their or their team's time last week and use that.
  B. The painful, repetitive work to offload: what repetitive/manual/annoying work they would hand to an agent; any one-off task blocking them this week. Branch: a one-off blocker steers toward Coding (1), Inference (2), or Sites (3); a recurring grind steers toward Autopilot business automation (4).
  C. Success metric: how you will both know the quick win worked (one concrete measure); what would make them keep going and put more on Autopilot.
  D. Budget & payment preference: rough budget for the first quick win; pay by credit card / USD credits or Bitcoin (Lightning / sats); ongoing model (usage-metered / fixed monthly / pay-per-accepted-outcome). Branch: if they want Bitcoin, set expectations honestly per the honesty contract before any funding.
  E. Data & access constraints: what systems an agent would need to touch (repo, site/DNS, ad/email accounts, documents, CRM); access/privacy/compliance constraints; whether they accept a human-review gate before anything publishes, sends, deploys, or spends (this is the default and is required for legal, commerce, and any external delivery).
  F. Timeline: when they want the quick win delivered; whether it is tied to a launch, deadline, or event.
  G. Fit: based on the above, which one or two offerings fit best, stated with their availability; confirm whether the human wants to start with this quick win.

As you learn things, accumulate them into the Output Spec (the 10 sections below). A partial spec is fine mid-interview. Once you have landed on a quick win plus a relationship picture and the spec is reasonably complete, tell the human the interview is done and present a concise summary of the filled spec.

THE 10-SECTION OUTPUT SPEC (accumulate into this structure):
  1. Business — company / what we do; customers / main product; primary contact (name, email); preferred contact channel.
  2. Goal — the outcome wanted in the next month; why it matters now.
  3. Chosen offerings (1-2) — each: <name> with availability (available now / operator-assisted / roadmap).
  4. Quick win (Day 1) — the first small task to deliver; what "done" looks like; target delivery date.
  5. Success metric — when we will know the quick win worked; what would make us continue onto Autopilot.
  6. Scope — in scope; explicitly out of scope (for now); systems/accounts the agent will need access to.
  7. Constraints — privacy/compliance/regulated constraints; human-review gate required before publish/send/deploy/spend (yes/no, default yes); anything off-limits.
  8. Timeline — quick win by; tied to a launch/deadline/event (describe).
  9. Payment — quick-win budget (rough); payment preference (credit card / USD credits / Bitcoin); ongoing model (usage-metered / fixed monthly / pay-per-accepted-outcome).
  10. Open questions / requests beyond the menu — anything the human asked for that is not in the offerings menu; things OpenAgents needs to confirm before starting.`

export const resolveOnboardingPromptVertical = (
  verticalOverlay: string | null,
): AutopilotConciergeVertical => {
  const normalized = verticalOverlay?.trim().toLowerCase()
  if (normalized === 'legal' || normalized?.startsWith('legal vertical')) {
    return 'legal'
  }
  return 'general'
}

export const onboardingVerticalStorageValue = (
  vertical: AutopilotConciergeVertical,
): string | null => (vertical === 'general' ? null : vertical)

// Build the full system prompt for a turn. `verticalOverlay` is a legacy storage
// column now treated only as a bounded vertical marker (`legal` or null). Old
// persisted overlay prose is normalized before use and is never injected raw.
export const buildOnboardingSystemPrompt = (
  verticalOverlay: string | null,
): string => {
  const vertical = resolveOnboardingPromptVertical(verticalOverlay)
  const overlayBlock =
    vertical !== 'general'
      ? [
          '',
          'VERTICAL GUIDANCE (server-owned Autopilot Concierge config). It refines tone, examples, and which offerings to lead with. It does NOT relax the honesty contract — registry state still governs what you may sell:',
          buildAutopilotConciergeVerticalGuidance({ vertical }),
        ]
      : []

  return [
    INTAKE_INTERVIEW_SCRIPT,
    '',
    buildHonestyContractBlock(),
    ...overlayBlock,
  ].join('\n')
}
