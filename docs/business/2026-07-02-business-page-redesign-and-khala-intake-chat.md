# /business Redesign, Khala Intake Chat, and the Landing Preview

Date: 2026-07-02
Scope: owner-directed (2026-07-02): rebrand `openagents.com/business` to
"Agents that work." on the site's dark operational theme, replace the
static-form-first intake with a dynamic conversational intake ("talk with
Khala, which intuits their needs" — the interview from
`docs/business/2026-06-20-openagents-business-intake-spec.md`), and add a
review-only landing-page candidate that forks BUILDERS → Khala Code and
BUSINESSES → `/business`. The live homepage is untouched.

## What shipped

### 1. `/business` — "Agents that work." (dark ops redesign)

`apps/openagents.com/apps/web/src/page/business.ts`:

- **Dark-only.** The page previously forced the light landing theme on a
  dark site; it now renders the DESIGN.md operational surface (pure black,
  mono-first, subtle borders) with no theme selector. Page title:
  "Agents that work - OpenAgents".
- **Hero:** eyebrow "OpenAgents Business", H1 **"Agents that work."**,
  registry-honest body copy (quick win in days → Autopilot as trust
  builds; receipts; human-review gate; receipt-planned payment). Primary
  CTA "Talk to Khala" → the intake console; secondary "Use the form".
- **The Khala intake console** (new): a command-surface panel
  (`KHALA · INTAKE` strip, role-prefixed mono transcript, single-line
  composer) rendered server-side as a static shell with an honest empty
  state and a `<noscript>` pointer to the form.
- The offering menu, quick-win ladder, project-invite, and signup form
  keep their shared `@openagentsinc/ui` components, now in dark mode; the
  form is the no-JS fallback **and the single submit authority** — the
  chat drafts, the form submits.

### 2. The intake chat (dynamic, bounded, honest)

- **Client:** `page/business-intake-chat.ts` (pure state core — bounds
  mirroring the server: ≤24 messages, ≤2,000 chars each; phases
  ready/waiting/done/rate_limited/unavailable/error; strict reply
  decoding where done-without-spec is not done) +
  `page/business-intake-chat-controller.ts` (install-once DOM controller
  in `entry.ts`, decoupled from the Foldkit loop like the tokens-served
  count-up). Server calls are strictly user-initiated — no completion is
  spent on page load. On completion, the drafted Output Spec is written
  into the form's `helpWith` textarea and the visitor is walked to the
  form; failure states render honestly (rate-limited / offline → "the
  form below works"). Reduced-motion-safe.
- **Worker:** `POST /api/public/business-intake-chat` — a bounded,
  stateless, server-side interview. The system prompt encodes the intake
  spec (offerings menu with honest availability labels, interview areas
  A–G, quick-win ladder, Output Spec Template) and instructs the model to
  emit the completed spec inside a sentinel the route extracts into
  `{ done: true, spec }`. Serving rides the existing gateway provider
  seam (same enablement + provider-key gating as the free tier; 503
  fail-closed when unavailable), fixed model and bounded params, per-IP
  rate limits, and **exact token accounting** on every completion
  (`usage_truth='exact'`, internal demand attribution with
  `demand_source='business_intake_chat'` — acquisition-surface usage is
  never mislabeled as external demand).

### 3. `/preview/landing` — the review-only landing candidate

`apps/openagents.com/apps/web/src/page/landingPreview.ts`, route
`LandingPreview` (`/preview/landing`, statelessShell, open): a candidate
front door for owner review, with a banner stating it is not the live
homepage. One thesis line ("Software, built by agents."), then the fork:

- **FOR BUILDERS — "Build it myself"** → `/khala` (Khala Code: open
  source, OpenAI-compatible free API, wraps your own Codex, exact token
  accounting).
- **FOR BUSINESSES — "Build it for me"** → `/business` ("Agents that
  work": quick win in days, human-review gate, receipts, dollars or
  Bitcoin).

Proof footer links only to dereferenceable surfaces (repo, promise
registry, /stats). No marketing numbers, no green claims the registry
does not carry. Cutover to `/` is an explicit owner decision, not part
of this change.

## Honesty boundary

The chat interviews and drafts; it grants nothing. The intake receipt
remains the only stored artifact (`business_signup_requests` via the
existing signup endpoint); the interview endpoint stores no transcript
server-side. Availability language in both the page copy and the system
prompt stays pinned to the registry's shipped / operator-assisted /
roadmap labels; the spec's honesty note ("we'll say so in writing and
scope the smallest honest version") is part of the prompt. No pricing is
published (rate-card publication is AW-0/A0.2 in
`docs/fable/ROADMAP_AFTER.md`, an owner decision).

## Verification

- Web: `bun run typecheck:web`; suites: route/route-coverage/
  navigation-policy/client-server-route-agreement/business-route/
  business-intake-chat/main/update/startup/view.scene (453 tests at time
  of writing) — green.
- Worker: `business-intake-chat.test.ts` (bounds, gating, sentinel
  extraction, rate limiting, exact-usage recording) + typecheck +
  `check:deploy` — see the PR/commit for outputs.
- Visual: desktop + mobile screenshots of both pages reviewed against
  DESIGN.md (dark-only, mono, no gradients/cards-by-default).
