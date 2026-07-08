# Provider-Peer ToS-Compliance Review — OpenAI/ChatGPT-Codex, Anthropic, Google Gemini

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11. Rung: M13 (#4771) of
`docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`.
This document is the gating deliverable for the M13 connect-flow work:
no provider connect flow ships unless this review concludes the flow
is compliant with the provider's current published terms.

Peer reviews: the CX1 Codex credential-policy review
(`apps/pylon/docs/codex-bridge.md`, "Credential policy") and the
Claude Agent bridge BYOK policy (`apps/pylon/docs/claude-agent-bridge.md`,
epic #4717) cover Lane B — the contributor's own credentials on the
contributor's own Pylon. This review covers Stack A — hosted
provider-account connect in the `openagents.com` web product, where
the platform stores a credential reference and dispatches the user's
own runs against it.

## The lane law this review binds to

The standing law (issue #4771, wedge essay router law): **the user's
identity, the user's inference, BYOK only — no credential brokering,
no resale metering, no pooled accounts across users.** A connected
provider account is used solely for runs the owning user requested,
under that user's identity with the provider. If a provider's terms
foreclose even that, the product narrows honestly for that provider.

This review evaluates three connection shapes per provider:

1. **API-key BYOK** — the user pastes their own provider API key; the
   platform stores it as a secret ref and uses it only for that
   user's runs.
2. **Subscription-account connect** — the user signs into their
   consumer subscription (ChatGPT Plus/Pro, Claude Pro/Max, Google AI
   Pro/Ultra) and the platform routes inference through the
   subscription credential.
3. **Multi-account routing / delegated background usage** — the lease
   policy rotating among the user's own connected accounts, and runs
   executing unattended in the background.

## Findings per provider

### 1. OpenAI / ChatGPT-Codex (the existing lane)

**Connection shape shipped today:** the first-party Codex device-login
flow (`/api/provider-accounts/chatgpt-codex/device-login/*`), which is
OpenAI's own published auth flow for Codex
(`https://auth.openai.com/codex/device`). The user authenticates their
own ChatGPT/Codex account through OpenAI's own device-code ceremony;
the platform never captures the user's password and the resulting
credential drives Codex — the product OpenAI built that credential
for. OpenAI also documents API-key auth (`OPENAI_API_KEY` /
`CODEX_API_KEY`) for programmatic Codex use; the CX1 review
(2026-06-11, `apps/pylon/docs/codex-bridge.md`) already cleared both
key sources and the owner's own `codex login` for owner-jobs.

**Constraints the terms impose, bound into the design:**

- Account credentials stay single-user: the lease policy selects only
  among accounts connected by (and scoped to) the requesting user or
  their team's own connected pool — never a cross-customer pool.
- The CX1 scope boundary stands: serving *other people's* jobs on
  subscription-login auth raises resale questions that remain
  uncleared; provider-mode (Lane C / P6) work on subscription auth is
  blocked pending its own review. API-key sources are the safe
  default for provider-mode work.
- No resale metering: the platform charges for agentic work and
  accepted outcomes, not for marked-up raw inference on a connected
  subscription (INVARIANTS.md, "Provider Capacity Marketplace Gate").

**Verdict:** device-login connect for the user's own runs — compliant
as shipped. Multi-account rotation within the user's own accounts —
compliant (the accounts are all the user's own identity). Background
usage — Codex is built for delegated background work; compliant.

### 2. Anthropic

**What the current terms say (reviewed 2026-06-11 via web research):**

- Anthropic's Claude Code legal-and-compliance documentation
  (`https://code.claude.com/docs/en/legal-and-compliance`,
  "Authentication and credential use", added ~2026-02-19) states
  verbatim: *"OAuth authentication is intended exclusively for
  purchasers of Claude Free, Pro, Max, Team, and Enterprise
  subscription plans and is designed to support ordinary use of
  Claude Code and other native Anthropic applications"* and
  *"Developers building products or services that interact with
  Claude's capabilities, including those using the Agent SDK, should
  use API key authentication through Claude Console or a supported
  cloud provider. Anthropic does not permit third-party developers to
  offer Claude.ai login or to route requests through Free, Pro, or
  Max plan credentials on behalf of their users."*
- This is actively enforced: in January–February 2026 Anthropic
  blocked third-party tools (OpenClaw, OpenCode, Roo Code, Goose and
  others) that used OAuth tokens extracted from Claude subscriptions,
  and updated the Consumer Terms posture to make the prohibition
  explicit (The Register, 2026-02-20,
  `https://www.theregister.com/2026/02/20/anthropic_clarifies_ban_third_party_claude_access/`).
- The Commercial Terms (`https://www.anthropic.com/legal/commercial-terms`)
  prohibit sharing account credentials/API keys with others and
  prohibit redistribution patterns where one party's key serves
  external customers as a conduit. A user's own key used by a tool
  the user chose, for the user's own requests, is the pattern
  Anthropic's own documentation directs developers toward.

**Per-shape conclusions:**

- **API-key BYOK: COMPLIANT.** The user creates their own Claude
  Console API key and connects it; the platform uses it only for that
  user's runs (Claude API / Claude Agent SDK executor lanes). This is
  the exact shape Anthropic's documentation prescribes for products
  built on the Agent SDK. Design constraints bound: the key is
  user-scoped (no cross-user pooling), usage is the user's own
  inference billed to the user's own Anthropic account, and the
  platform charges for work, not marked-up tokens.
- **Subscription-account connect (Claude.ai / Pro / Max login or
  OAuth-token capture): NOT COMPLIANT — do not build.** Anthropic
  explicitly forbids third parties offering Claude.ai login or
  routing requests through Free/Pro/Max credentials on behalf of
  users, and enforces without notice. There is no "user consents"
  carve-out; the prohibition runs against the third-party developer.
- **Multi-account routing:** compliant only across the user's own
  connected API keys (e.g., personal + team workspace keys). Pooling
  keys across customers, or platform-held keys resold as user
  capacity, stays blocked.
- **Delegated/background usage:** compliant on API keys — the
  Commercial Terms and Agent SDK exist for programmatic, unattended
  use. (Note: Anthropic's June 2026 Agent SDK subscription-credit
  change concerns *subscription* plans, which this lane does not
  touch.)

### 3. Google Gemini

**What the current terms say (reviewed 2026-06-11 via web research):**

- The Gemini API Additional Terms of Service
  (`https://ai.google.dev/gemini-api/terms`, last updated 2026-03-23)
  govern API-key access for "developers building with Google AI
  models for professional or business purposes." They contain no
  prohibition on a user authorizing a third-party platform to call
  the API with the user's own key; the standard developer obligations
  (key confidentiality, Prohibited Use Policy, age limits) apply.
- Google explicitly distinguishes the OAuth/subscription path: using
  third-party software to access the services powering Gemini CLI /
  Gemini Code Assist / Antigravity OAuth (Google account login,
  including Google AI Pro/Ultra subscriptions) "is a violation of
  applicable terms and policies" (google-gemini/gemini-cli service
  update, discussion #22970). Google mass-suspended accounts for
  OpenClaw-style OAuth piggybacking in February 2026 and directs
  third-party agents to use a Gemini API key or Vertex AI key
  instead.
- Operational note: Google is migrating Gemini API "standard" keys to
  service-account-bound "auth" keys, with standard keys rejected from
  September 2026. Key format and validation must therefore stay
  opaque on our side — store, redact, and probe; never pattern-gate
  beyond redaction-safety bounds.

**Per-shape conclusions:**

- **API-key BYOK: COMPLIANT.** The user's own Gemini API key (AI
  Studio / Cloud project) connected for the user's own runs is the
  path Google itself recommends for third-party coding agents. Same
  bound constraints as Anthropic: user-scoped, user-billed, no
  resale metering.
- **Subscription/OAuth-account connect (Google account login, Code
  Assist individuals, AI Pro/Ultra): NOT COMPLIANT — do not build.**
  Google treats third-party access through those credentials as a
  terms violation and suspends accounts, including paying
  subscribers.
- **Multi-account routing / background usage:** compliant across the
  user's own keys; unattended programmatic use is the API's intended
  shape. The existing platform-key Gemini broker
  (`GEMINI_API_KEY` worker secret, token-usage-metered) is the
  platform acting as its own Gemini API customer for platform-paid
  usage — a separate, already-bounded lane; it is not account
  leasing and is unaffected by this review.

## Summary table

| Provider | API-key BYOK connect | Subscription-account connect | Multi-account routing (own accounts) | Delegated/background |
| --- | --- | --- | --- | --- |
| OpenAI / ChatGPT-Codex | Compliant (CX1) | Compliant via OpenAI's own Codex device-login only; owner-jobs scope | Compliant | Compliant |
| Anthropic | **Compliant — build** | **Forbidden — do not build** | Compliant (own keys only) | Compliant (API keys) |
| Google Gemini | **Compliant — build** | **Forbidden — do not build** | Compliant (own keys only) | Compliant (API keys) |

## Design constraints bound by this review

1. Anthropic and Gemini provider peers connect by **API key only**
   (`authMode: 'api_key'`). No Claude.ai/Pro/Max login, no Google
   account OAuth, no subscription token capture — those connect
   shapes are not built, and adding one requires a new dated review
   superseding this one.
2. Connected keys are **user-scoped lease candidates**: the lease
   policy may only select provider-tagged candidates belonging to the
   requesting user/team's own connected accounts
   (`provider-account-lease-policy.ts`, provider-tagged candidates).
3. **Redaction law unchanged:** raw keys live only in the auth KV
   under `provider-auth:<providerAccountRef>`; D1 rows, projections,
   events, grants, logs, tests, and fixtures carry secret refs only
   (`@openagentsinc/provider-account-schema` markers cover `sk-ant-*`
   via the `sk-` marker, `AIza*`, `ANTHROPIC_API_KEY=`,
   `GEMINI_API_KEY=`).
4. **Honest copy:** a provider is described as connectable, not
   "supported", until a real run consumes a leased account
   end-to-end; executor copy says "Claude Agent" (never
   "Claude Code") for the Anthropic lane.
5. **No resale metering:** the platform bills for agentic work and
   accepted outcomes; connected-key inference is billed by the
   provider to the user's own provider account
   (INVARIANTS.md, Provider Capacity Marketplace Gate, unchanged).

## Sources

- https://code.claude.com/docs/en/legal-and-compliance (fetched
  2026-06-11; "Authentication and credential use" quoted above)
- https://www.anthropic.com/legal/commercial-terms /
  https://www.anthropic.com/legal/consumer-terms (linked from the
  page above as the governing agreements)
- https://www.theregister.com/2026/02/20/anthropic_clarifies_ban_third_party_claude_access/
- https://ai.google.dev/gemini-api/terms (fetched 2026-06-11; last
  updated 2026-03-23)
- https://google-gemini.github.io/gemini-cli/docs/tos-privacy.html
  (per-auth-method governing terms)
- https://github.com/google-gemini/gemini-cli/discussions/22970
  (service update: third-party OAuth access is a violation)
- `apps/pylon/docs/codex-bridge.md` (CX1 credential-policy review,
  2026-06-11)
