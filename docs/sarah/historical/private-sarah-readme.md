# Sarah

Sarah is OpenAgents' AI sales employee — the realtime voice + chat sales
agent for **sarah.openagents.com**. She is the first customer-facing
instance of the OpenAgents AI-employee product: she qualifies prospects,
explains what OpenAgents offers with claims bound to the public promise
registry, assembles packages from typed deal rules (never improvising a
price), and closes with a real checkout link — every consequential act
receipted.

The full product spec, audit, and rollout plan live in the main repo:
`OpenAgentsInc/openagents` →
`docs/fable/2026-07-07-sarah-sales-agent-spec.md`, sequenced as Phase P1
of `docs/fable/MASTER_ROADMAP.md`.

## Why a separate repo

This repo deliberately stands alone (private, `OpenAgentsInc/sarah`) so we
can use the standard Vercel + AI SDK realtime stack unmodified — Next.js,
`@ai-sdk/gateway` canary realtime, `useRealtime` — deploy straight to
Vercel at `sarah.openagents.com`, and know the newest voice-agent surface
area works before wiring it into the monorepo's substrate (CRM, credits,
receipts) over HTTP. It may be merged into `OpenAgentsInc/openagents`
later; until then, treat the monorepo as the system of record Sarah talks
to, not code she shares.

## Current state

An AI SDK browser realtime app with Eve-backed Sarah instructions and
durable transcript intake:

- `src/app/api/realtime/token/route.ts` — guarded server-side
  `gateway.experimental_realtime.getToken` for `openai/gpt-realtime-2`.
  The route requires same-origin POSTs, mints an opaque prospect cookie,
  applies per-IP/per-prospect rate limits and active-session caps, enforces
  a daily token-mint cap, and emits spend alerts by file or webhook. The AI
  Gateway key never reaches the browser. (Realtime models are swappable at
  this one line — `xai/grok-voice-think-fast-1.0` is the speech-to-speech
  alternative.)
- `agent/instructions.md` — Sarah's Eve-owned persona and authority rules;
  `src/app/api/realtime/session-config/route.ts` injects this file into the
  realtime session config so the browser loop and Eve share the same
  first-contact disclosure and one-question-at-a-time sales posture.
- `agent/channels/realtime-transcript.ts` — authored Eve channel for the
  realtime bridge. Text and voice transcript turns are serialized by
  `prospect_ref`-derived thread id and held until Eve reaches
  `session.waiting`, so follow-up transcript turns land in the same durable
  Eve session.
- `agent/channels/email.ts` — authored Eve email ingress channel. It accepts a
  signed inbound webhook payload, resumes by email/thread continuation token,
  treats the body as untrusted user content, and queues Sarah's completed reply
  as an operator approval draft instead of sending mail automatically. When the
  sender address matches a CRM-projected web prospect, the email continuation
  reuses that prospect ref and thread id.
- `src/app/api/eve/turn/route.ts` — Next-side bridge from the realtime UI to
  the Eve transcript channel. It reads or mints the first-party
  `sarah_prospect_ref` cookie, maps it to the Eve continuation token, and
  records a server-side operator projection with transcript text and audio
  event metadata only (no raw audio by default). Local dev discovers the Eve
  sidecar from `.eve/next-dev-server.json`; production should set `EVE_HOST`
  if the custom channel is not exposed through the Next service origin.
- `src/app/api/prospect/session/route.ts` — returning-browser probe for the
  prospect-backed thread id, so closing and reopening the tab resumes the
  same Eve session when the cookie is present.
- `src/app/api/operator/prospects/route.ts` — local operator session list
  keyed by prospect ref, backed by `.sarah/session-index.json` unless
  `SARAH_SESSION_INDEX_PATH` is set. Production should set
  `SARAH_OPERATOR_TOKEN`.
- `src/app/api/operator/email-drafts/route.ts` — local operator approval queue
  for email drafts. Approval marks a draft `approved_pending_send` unless
  `SARAH_EMAIL_SEND_LIVE=1` arms Resend REST delivery. Opt-out suppression is
  durable via `/unsubscribe` and suppressed drafts cannot be approved into a
  send. The Eve Chat SDK Resend adapter remains gated until it supports this AI
  SDK 7 canary stack cleanly.
- `src/lib/promise-registry.ts` — server-only live promise-registry fetcher
  for `https://openagents.com/api/public/product-promises`, with TTL cache
  and fail-closed degraded instructions. Realtime sessions receive registry
  version/state counts plus green, yellow, and non-live promise boundaries.
- `agent/tools/{intake_capture,human_handoff,checkout_link_create}.ts` —
  first real Sarah sales tools. They execute through the Eve realtime bridge,
  return receipt-shaped results, and stay in dry-run mode unless
  `SARAH_OPENAGENTS_LIVE_WRITES=1` arms OpenAgents API writes.
- `agent/tools/{crm_contact_upsert,crm_activity_append}.ts` — OpenAgents CRM
  sync tools. Live mode calls the monorepo-owned `/api/mcp` CRM surface with a
  scoped MCP bearer token; dry-run mode records only local Sarah receipts.
- `agent/tools/deal_rules_evaluate.ts` and `src/lib/deal-rules.ts` — typed
  `sarah.deal_rules.v1` evaluator. Credit-volume tiers, Bitcoin discount, the
  default transaction cap, and the 3+ large-module bundle rule are compiled as
  config refs; module prices and tactics stay owner-gated placeholders until
  signed. `checkout_link_create` now requires a `quoteRef` and deal-rule refs.
- `src/lib/gateway-realtime-browser.ts` — the browser-side realtime model
  shim (WebSocket config via subprotocol auth, passthrough event codec).
- `src/app/page.tsx` — connect/disconnect, mic capture with server VAD,
  playback, text-message send, live connection/event/error state, branded
  Sarah disclosure UI, receipt cards, and background transcript mirroring
  into Eve.
- `evals/sarah-fixtures.json` and `scripts/sarah-eval-suite.mjs` — S-12
  fixture-first eval suite with CONFIRMED/REFUTED verdicts for
  qualification, honesty, discount pressure, inbound-email injection, fake
  checkout close path, and public disclosure checks against the configured
  deployment.
- `agent/schedules/follow-up.ts` and `src/lib/follow-up-scheduler.ts` —
  quiet-after-quote loop-back queue. Due follow-ups create email drafts in
  the approval queue; closed, opted-out, or suppressed jobs do not send.
- `src/app/api/operator/{follow-ups,ops}/route.ts` — protected operator JSON
  views/actions for follow-ups, sessions, approval queue, receipts, and
  realtime spend guard status.

S-1 evidence: `docs/evidence/2026-07-08-s1-eve-transcript-evidence.json`
records a text transcript and a voice transcript accepted under the same
Eve `sessionId` and thread id, both reaching `session.waiting`.

S-2 evidence: `docs/evidence/2026-07-08-s2-promise-registry-grounding.json`
records the live registry grounding snapshot and a non-green capability
probe where Sarah labels the agentic-npm paid marketplace as roadmap/not
live.

S-3 evidence: `docs/evidence/2026-07-08-s3-token-hardening.json` records
same-origin denial, per-prospect/session throttling, daily cap denial, and
a file-backed spend-alert smoke using `SARAH_REALTIME_TOKEN_TEST_MODE=1`.

S-4 evidence: `docs/evidence/2026-07-08-s4-durable-sessions.json` records a
two-turn close-tab/return simulation where the same `sarah_prospect_ref`
cookie resumes the same Eve `sessionId` and the operator endpoint exposes
the text + voice transcript metadata server-side.

S-5 evidence: `docs/evidence/2026-07-08-s5-tool-bridge-smoke.json` records
a server-side realtime tool bridge smoke where `/api/eve/tool-call` executes
`demo_sales_context`, Eve returns a waiting-session receipt, and operator
ops exposes the same tool call. It also records a browser text conversation
that invoked `demo_sales_context` and rendered the result.
`docs/evidence/2026-07-08-s5-browser-voice-smoke.json` records the browser
voice proof: system Chrome receives a generated Samantha-voice fake mic
fixture, Sarah transcribes the spoken prompt, invokes `demo_sales_context`,
speaks the tool result, and renders the tool receipt in the conversation.

S-6 evidence: `docs/evidence/2026-07-08-s6-live-intake-gate.json` records
a configured deal-rules quote plus a live OpenAgents business-signup intake
row created from Sarah. The remaining gates are the operator handoff token,
a configured OpenAgents checkout creation endpoint, and the OpenAgents Worker
deploy blocked by Cloudflare D1 account storage quota.
`docs/evidence/2026-07-08-s6-openagents-endpoint-gates.json` records a
no-secret production endpoint preflight: the OpenAgents handoff route is
deployed and auth-protected, while the Sarah checkout route still returns
404 on `https://openagents.com`.

S-9 evidence: `docs/evidence/2026-07-08-s9-deal-rules-v0.json` records
property-style deal-rule tests, a realtime `deal_rules_evaluate` quote, a
checkout refusal without quote trace, and a valid traced dry-run checkout.

S-10 evidence: `docs/evidence/2026-07-08-s10-ui-verification.json` records
the branded public Sarah UI, pre-connect AI disclosure, hidden diagnostics,
and desktop/mobile browser checks.

S-12 evidence: `docs/evidence/2026-07-08-s12-eval-suite.json` records a
green fixture-first eval run. The generated transcript artifact is written
to `.sarah/evals/sarah-eval-suite.latest.json`.

S-13 evidence: `docs/evidence/2026-07-08-s13-followups-ops.json` records a
green follow-up/receipt smoke: due follow-up to approval draft, suppression,
and complete session receipt export.

Remaining gate rollup:
`docs/evidence/2026-07-08-sarah-remaining-gates.json` records the current
gate audit for the original open lanes: S-6 checkout endpoint still 404s,
S-7 cannot currently mint/use a scoped CRM MCP grant with the local ignored
operator credential candidate, S-8's public Resend Chat SDK adapter remains on
the AI SDK 6 peer line, and S-11 production hosting/DNS is not yet serving
Sarah.
`docs/evidence/2026-07-08-openagents-d1-deploy-blocker.json` records the
current shared OpenAgents deploy blocker behind S-6/S-7: both staging and
production still list the pending D1 migration
`0309_provider_account_token_custody_auth_deleted.sql`, while the account D1
footprint remains about 5.12 GB. The exit gate is to raise/free the Cloudflare
D1 account storage quota, then rerun the sanctioned OpenAgents deploy path so
the migration applies before the Worker upload.

S-8 continuity evidence:
`docs/evidence/2026-07-08-s8-web-email-continuity-binding.json` records the
web-prospect email binding smoke. The live `sarah@` Resend adapter remains
owner/compatibility gated.

S-8 send/suppression evidence:
`docs/evidence/2026-07-08-s8-email-suppression-and-send-gate.json` records
approval-gated outbound behavior, the durable opt-out suppression list, and the
current Chat SDK Resend adapter compatibility blocker.

S-8 adapter compatibility evidence:
`docs/evidence/2026-07-08-s8-resend-adapter-compat.json` records a live npm
metadata probe showing the latest public `@resend/chat-sdk-adapter` still
depends on the public Chat SDK line whose `ai` peer is incompatible with Sarah's
AI SDK 7 realtime canary stack.

S-7 live CRM gate evidence:
`docs/evidence/2026-07-08-s7-live-crm-gate.json` records the opt-in live CRM
smoke command and a prior production catalog blocker: with a temporary scoped
grant, production's deployed CRM MCP catalog did not expose
`crm.contact.upsert` or `crm.activity.append`. Current catalog preflight now
fails earlier because the local ignored production operator credential candidate
returns 401 while minting a temporary CRM MCP grant. Provide a current scoped
Sarah CRM MCP token or production admin bearer, then rerun the catalog preflight
before the live smoke.
`docs/evidence/2026-07-08-s7-mcp-catalog-gate.json` records the no-write
catalog preflight and is rewritten on every run; its current state is blocked on
`Could not mint temporary CRM MCP grant: 401 unauthorized`.

Stack: Next.js 16 / React 19 / Tailwind 4 / `ai@canary`,
`@ai-sdk/react@canary`, `@ai-sdk/gateway@canary` (realtime requires the
canary line).

## Development

```sh
pnpm install
cp .env.example .env.local   # set AI_GATEWAY_API_KEY (Vercel AI Gateway)
pnpm dev
pnpm audit:openagents-deploy-blocker # read-only OpenAgents D1/migration probe
pnpm audit:remaining-gates # current S-6/S-7/S-8/S-11 non-secret gate rollup
pnpm test:s5-tool-bridge  # requires local app at localhost:3000
pnpm test:s5-browser-voice # requires local app, system Chrome, macOS say, ffmpeg
pnpm test:s6-sales-flow   # requires live-write arming for monorepo effects
pnpm test:s6-openagents-gates # no-secret OpenAgents endpoint deploy preflight
pnpm test:s7-live-crm     # requires live-write arming + scoped CRM MCP token
pnpm test:s7-mcp-catalog  # catalog preflight; needs scoped MCP token or admin bearer
pnpm test:s12-evals          # with SARAH_EVAL_BASE_URL, defaults localhost:3000
pnpm test:s8-continuity      # isolated web-prospect -> email binding smoke
pnpm test:s8-email-gates     # isolated email approval/suppression smoke
pnpm test:s8-resend-adapter-compat # live npm metadata probe, installs nothing
pnpm test:s13-followups      # isolated follow-up + receipt smoke
```

Open `http://localhost:3000`, Connect, Start mic, talk.

Realtime token guard environment:

- `SARAH_REALTIME_VOICE`: optional realtime voice preset. Defaults to
  `shimmer`.
- `SARAH_OPENAGENTS_BASE_URL`: monorepo system-of-record API origin.
  Defaults to `https://openagents.com`.
- `SARAH_OPENAGENTS_LIVE_WRITES`: set to `1` only when Sarah should write to
  OpenAgents APIs. Local dev defaults to dry-run tool receipts.
- `SARAH_OPENAGENTS_CRM_MCP_TOKEN`: scoped OpenAgents CRM MCP bearer token.
  Required for live `crm_contact_upsert`, `crm_activity_append`, and returning
  prospect CRM context reads.
- `SARAH_OPENAGENTS_OPERATOR_TOKEN`: bearer token for operator-only
  OpenAgents endpoints such as business pipeline handoff.
- `SARAH_OPENAGENTS_CHECKOUT_ENDPOINT`: explicit checkout creation endpoint.
  Defaults to `/api/operator/business/sarah-checkout-links`, the operator-gated
  OpenAgents route that can return a no-money test-mode checkout receipt or,
  when owner-armed, delegate to the Stripe credit checkout catalog.
- `SARAH_OPENAGENTS_CHECKOUT_BUYER_USER_ID`: optional existing OpenAgents
  `users.id` used only for owner-armed live Stripe checkout creation. Leave
  empty for monorepo test-mode checkout receipts.
- `SARAH_ALLOWED_ORIGINS`: optional comma-separated origin allowlist. Empty
  defaults to the request origin, which is the normal Vercel same-origin
  deployment shape.
- `SARAH_REALTIME_RATE_WINDOW_MS`, `SARAH_REALTIME_MAX_TOKENS_PER_IP`,
  `SARAH_REALTIME_MAX_TOKENS_PER_PROSPECT`: request throttles.
- `SARAH_REALTIME_MAX_ACTIVE_SESSIONS_PER_IP`,
  `SARAH_REALTIME_MAX_ACTIVE_SESSIONS_PER_PROSPECT`,
  `SARAH_REALTIME_SESSION_TTL_MS`: active-session caps. The current AI
  Gateway canary does not expose a Gateway-side realtime token TTL; the app
  enforces this local session-slot TTL before minting.
- `SARAH_REALTIME_DAILY_TOKEN_CAP`,
  `SARAH_REALTIME_SPEND_ALERT_THRESHOLD`,
  `SARAH_REALTIME_SPEND_ALERT_FILE`,
  `SARAH_REALTIME_SPEND_ALERT_WEBHOOK_URL`: daily token-mint cap and alert
  outputs. File alerts are for local smoke tests; production should use a
  webhook plus Vercel/Gateway dashboard spend caps.
- `SARAH_SESSION_INDEX_PATH`: optional filename under `.sarah/` for the
  local operator projection. Empty defaults to `.sarah/session-index.json`.
- `SARAH_OPERATOR_TOKEN`: bearer token required by the operator endpoint in
  production.
- `SARAH_EMAIL_WEBHOOK_TOKEN`: optional bearer token for
  `POST /eve/email/inbound`. Required in production.
- `SARAH_EMAIL_APPROVAL_QUEUE_PATH`: optional filename under `.sarah/` for
  the local email approval queue. Empty defaults to
  `.sarah/email-approval-queue.json`.
- `SARAH_EMAIL_SUPPRESSION_LIST_PATH`: optional filename under `.sarah/` for
  opt-out suppression state. Empty defaults to
  `.sarah/email-suppression-list.json`.
- `SARAH_EMAIL_SEND_LIVE`: set to `1` only after the Sarah mailbox/domain is
  armed. Approved drafts then send through Resend; otherwise approvals remain
  `approved_pending_send`.
- `SARAH_RESEND_API_KEY`, `SARAH_EMAIL_FROM_ADDRESS`,
  `SARAH_EMAIL_FROM_NAME`, `SARAH_EMAIL_REPLY_TO`: live outbound Resend
  configuration. The API key stays in `.env.local` / deployment env only.
- `SARAH_FOLLOW_UP_QUEUE_PATH`: optional filename under `.sarah/` for the
  local follow-up queue. Empty defaults to `.sarah/follow-ups.json`.
- `SARAH_PUBLIC_BASE_URL`: public Sarah origin used in email opt-out footers.
  Defaults to `https://sarah.openagents.com`.
- `SARAH_EVAL_BASE_URL`, `SARAH_EVAL_TIMEOUT_MS`: deployment target and
  timeout for the S-12 eval gate.
- `SARAH_PRODUCTION_BASE_URL`, `SARAH_PRODUCTION_SMOKE_MINT_TOKEN`,
  `SARAH_PRODUCTION_SMOKE_TIMEOUT_MS`: production smoke target. Token minting
  is opt-in so routine disclosure/session-config checks do not create Gateway
  realtime credentials.

## Path to deploy (sarah.openagents.com)

Tracked in the main-repo roadmap (MASTER_ROADMAP P1 / lane SR-0) and as
GitHub issues on this repo; the short version, in order:

1. **Persona + honesty grounding** — Sarah's instructions (AI disclosure,
   one-question-at-a-time qualification, sales posture from the spec)
   injected via session config, with promise-registry state fetched
   server-side so capability claims stay registry-bound.
2. **Token-route hardening** — the token endpoint currently mints gateway
   client secrets unauthenticated; before public deploy it needs
   origin/rate limits, session caps + TTLs, and gateway spend alerts.
3. **Durable sessions + prospect ref** — persist transcripts server-side
   and mint an opaque prospect ref (cookie) so conversations resume; sync
   summaries to the OpenAgents CRM over its API (CRM stays the system of
   record).
4. **First tools** — wire the realtime session's tool channel (the token
   route already returns `tools: []`): `human_handoff`, intake capture →
   the openagents business-pipeline API, and a pack-priced checkout link.
5. **Sarah UI** — replace the diagnostic quickstart surface with the
   branded Sarah page (Protoss blue, disclosure line, mic states, text
   fallback chat).
6. **Vercel wiring** — project + `sarah.openagents.com` domain (DNS),
   `AI_GATEWAY_API_KEY` env, model pin, cost caps. See
   `docs/deployment/vercel-production-runbook.md`.
7. **Evals** — the Sarah fixture suite (qualification flows,
   discount-pressure probes, honesty probes) authored under the main
   repo's QAM-7 lane points at this deployment.

## Agent framework: eve (decided 2026-07-07)

Sarah's brain is **vercel/eve** (filesystem-first durable-agent
framework). Owner decision — not a spike. The division of labor:

- **The Next.js realtime loop stays the voice/text I/O** exactly as
  built (eve does not do realtime audio).
- **eve owns everything behind it**: durable sessions and state,
  `agent/instructions.md` as the persona file, typed `agent/tools/`
  (handoff, intake capture, checkout link, deal-rules evaluation),
  `agent/schedules/` for follow-ups, and `agent/channels/` — the Chat
  SDK **Resend email adapter** for Sarah's inbound/outbound email
  continuity now, Twilio as a future phone lane.
- **The bridge**: the realtime session's tool calls execute against eve;
  the transcript of each voice/text turn lands in the eve session so
  every channel shares one relationship thread.
- eve ships its full docs in `node_modules/eve/docs` — read them there
  before extending the agent directory.

Target layout once integrated:

```text
agent/
├── instructions.md      # Sarah persona + honesty rules (registry-bound)
├── tools/               # handoff, intake, checkout_link, deal_rules
├── skills/              # qualification playbook, objection handling
├── channels/            # resend email (now), twilio (later)
└── schedules/           # follow-up loop-backs
```

Setup and integration work is tracked in this repo's GitHub issues.

## Rules that bind this repo

- Sarah discloses she's an AI on first contact, in every channel.
- No improvised pricing — every price/discount traces to a deal-rules
  config ref; absent a rule, escalate to a human.
- Capability claims are capped by the public promise registry.
- Money-in only: checkout links, never spend/refund authority.
- The monorepo owns money, CRM, credits, and receipts; this repo calls
  its APIs and never re-implements them.
